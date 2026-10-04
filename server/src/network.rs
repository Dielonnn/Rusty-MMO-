//! Accepts connections, runs the world at a fixed tick rate, routes messages
//! and keeps the save file up to date.

use std::collections::HashMap;
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use shared::net::Connection;
use shared::protocol::*;
use shared::world::VIEW_DISTANCE;

use crate::TICK_RATE;
use crate::store::{Store, normalize_account};
use crate::world::{Audience, World};

/// Connections that haven't said hello by now are dropped.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// How often characters in the world are written to the save file.
const AUTOSAVE_INTERVAL: f32 = 5.0;

struct Client {
    conn: Connection,
    addr: SocketAddr,
    account: Option<String>,
    player: Option<EntityId>,
    connected_at: Instant,
}

pub struct Server {
    listener: TcpListener,
    clients: HashMap<u64, Client>,
    next_client: u64,
    pub world: World,
    pub store: Store,
    since_save: f32,
    /// Print connections and disconnections.
    pub verbose: bool,
}

impl Server {
    pub fn bind(addr: impl ToSocketAddrs, store: Store) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        Ok(Self {
            listener,
            clients: HashMap::new(),
            next_client: 0,
            world: World::new(seed),
            store,
            since_save: 0.0,
            verbose: true,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Runs the server until the process exits.
    pub fn run(mut self) {
        let dt = 1.0 / TICK_RATE as f32;
        let tick = Duration::from_secs_f32(dt);
        let mut next = Instant::now();
        loop {
            self.step(dt);
            next += tick;
            let now = Instant::now();
            if next > now {
                thread::sleep(next - now);
            } else {
                // Running behind; don't try to catch up with a burst of ticks.
                next = now;
            }
        }
    }

    /// One server tick: network in, simulate, network out, maybe save.
    pub fn step(&mut self, dt: f32) {
        self.accept();
        self.receive();
        self.world.tick(dt);
        self.deliver();
        self.since_save += dt;
        if self.since_save >= AUTOSAVE_INTERVAL {
            self.since_save = 0.0;
            self.save();
        }
    }

    /// Writes everyone in the world to the save file.
    pub fn save(&mut self) {
        for client in self.clients.values() {
            if let Some(c) = client.player.and_then(|id| self.world.character(id)) {
                self.store.update(c);
            }
        }
        if let Err(e) = self.store.save() {
            eprintln!("Couldn't save characters: {e}");
        }
    }

    fn accept(&mut self) {
        loop {
            match self.listener.accept() {
                Ok((stream, addr)) => match Connection::new(stream) {
                    Ok(conn) => {
                        self.next_client += 1;
                        self.clients.insert(
                            self.next_client,
                            Client {
                                conn,
                                addr,
                                account: None,
                                player: None,
                                connected_at: Instant::now(),
                            },
                        );
                    }
                    Err(e) => self.log(format!("{addr}: {e}")),
                },
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => {
                    self.log(format!("accept failed: {e}"));
                    break;
                }
            }
        }
    }

    fn receive(&mut self) {
        let mut dropped = Vec::new();
        let ids: Vec<u64> = self.clients.keys().copied().collect();
        for cid in ids {
            // Cap messages per tick so one client can't stall the server.
            for _ in 0..64 {
                let client = self.clients.get_mut(&cid).unwrap();
                let msg = match client.conn.poll::<ClientMsg>() {
                    Ok(Some(msg)) => msg,
                    Ok(None) => break,
                    Err(_) => {
                        dropped.push(cid);
                        break;
                    }
                };
                if !self.handle(cid, msg) {
                    dropped.push(cid);
                    break;
                }
            }
            let client = &self.clients[&cid];
            if client.account.is_none() && client.connected_at.elapsed() > HELLO_TIMEOUT {
                dropped.push(cid);
            }
        }
        for cid in dropped {
            self.disconnect(cid, "disconnected");
        }
    }

    fn send_characters(&mut self, cid: u64) {
        let client = self.clients.get_mut(&cid).unwrap();
        let list = client
            .account
            .as_ref()
            .map_or_else(Vec::new, |a| self.store.list(a));
        let _ = client.conn.send(&ServerMsg::Characters(list));
    }

    fn character_error(&mut self, cid: u64, msg: String) {
        let client = self.clients.get_mut(&cid).unwrap();
        let _ = client.conn.send(&ServerMsg::CharacterError(msg));
    }

    /// Handles one message. Returns false to drop the connection.
    fn handle(&mut self, cid: u64, msg: ClientMsg) -> bool {
        let client = self.clients.get_mut(&cid).unwrap();
        match (&client.account, client.player, msg) {
            (None, _, ClientMsg::Hello { version, account }) => {
                if version != PROTOCOL_VERSION {
                    let reason = format!(
                        "Version mismatch: the server speaks protocol {PROTOCOL_VERSION}, you have {version}. Update your game."
                    );
                    let _ = client.conn.send(&ServerMsg::Rejected(reason));
                    return false;
                }
                match normalize_account(&account) {
                    Ok(account) => {
                        client.account = Some(account);
                        self.send_characters(cid);
                    }
                    Err(e) => {
                        let _ = client.conn.send(&ServerMsg::Rejected(e.to_string()));
                        return false;
                    }
                }
            }
            (None, _, _) => {}
            (
                Some(account),
                None,
                ClientMsg::CreateCharacter {
                    name,
                    class,
                    appearance,
                },
            ) => {
                let account = account.clone();
                match self.store.create(&account, &name, class, appearance) {
                    Ok(()) => {
                        self.save_now();
                        self.send_characters(cid);
                    }
                    Err(e) => self.character_error(cid, e),
                }
            }
            (Some(account), None, ClientMsg::DeleteCharacter(name)) => {
                let account = account.clone();
                match self.store.delete(&account, &name) {
                    Ok(()) => {
                        self.save_now();
                        self.send_characters(cid);
                    }
                    Err(e) => self.character_error(cid, e),
                }
            }
            (Some(account), None, ClientMsg::EnterWorld(name)) => {
                let Some(c) = self.store.get(account, &name).cloned() else {
                    self.character_error(cid, format!("You have no character named {name}."));
                    return true;
                };
                let online = self.clients.values().any(|other| {
                    other.player.is_some_and(|id| {
                        self.world
                            .entities
                            .get(&id)
                            .is_some_and(|e| e.name == c.name)
                    })
                });
                if online {
                    self.character_error(cid, format!("{} is already in the world.", c.name));
                    return true;
                }
                let id = self.world.add_player(&c);
                let client = self.clients.get_mut(&cid).unwrap();
                client.player = Some(id);
                let _ = client.conn.send(&ServerMsg::Welcome { id });
                let msg = format!(
                    "{} entered the world as {} ({:?})",
                    client.addr, c.name, c.class
                );
                self.log(msg);
            }
            (Some(_), Some(id), ClientMsg::Logout) => {
                if let Some(c) = self.world.remove_player(id) {
                    self.store.update(c);
                    self.save_now();
                }
                self.clients.get_mut(&cid).unwrap().player = None;
                self.send_characters(cid);
            }
            (Some(_), Some(id), msg) => self.world.handle(id, msg),
            (Some(_), None, _) => {}
        }
        true
    }

    fn save_now(&mut self) {
        if let Err(e) = self.store.save() {
            eprintln!("Couldn't save characters: {e}");
        }
    }

    fn disconnect(&mut self, cid: u64, why: &str) {
        let Some(client) = self.clients.remove(&cid) else {
            return;
        };
        if let Some(c) = client.player.and_then(|id| self.world.remove_player(id)) {
            self.store.update(c);
            self.save_now();
        }
        self.log(format!("{} {why}", client.addr));
    }

    fn deliver(&mut self) {
        let outbox = self.world.drain_outbox();
        let mut dropped = Vec::new();
        for (cid, client) in &mut self.clients {
            let Some(id) = client.player else { continue };
            let Some(me) = self.world.entities.get(&id) else {
                continue;
            };
            let my_pos = me.pos;
            let mut ok = true;
            for (to, msg) in &outbox {
                let wanted = match *to {
                    Audience::Everyone => true,
                    Audience::Near(pos) => pos.distance(my_pos) <= VIEW_DISTANCE,
                    Audience::Only(who) => who == id,
                };
                if wanted {
                    ok &= client.conn.send(msg).is_ok();
                }
            }
            if let Some(snapshot) = self.world.snapshot_for(id) {
                ok &= client.conn.send(&ServerMsg::Snapshot(snapshot)).is_ok();
            }
            if !ok {
                dropped.push(*cid);
            }
        }
        for cid in dropped {
            self.disconnect(cid, "dropped");
        }
    }

    fn log(&self, msg: String) {
        if self.verbose {
            println!("{msg}");
        }
    }
}

/// Starts a server on a background thread, listening on localhost only and
/// saving to `save_path`. Used by the client's single-player mode.
pub fn spawn_local(save_path: PathBuf) -> io::Result<SocketAddr> {
    let store = Store::open(save_path)?;
    let mut server = Server::bind("127.0.0.1:0", store)?;
    server.verbose = false;
    let addr = server.local_addr()?;
    thread::Builder::new()
        .name("local-server".into())
        .spawn(move || server.run())?;
    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::data::{Appearance, Class};

    fn pump(
        server: &mut Server,
        conn: &mut Connection,
        until: impl Fn(&ServerMsg) -> bool,
    ) -> Vec<ServerMsg> {
        let mut got = Vec::new();
        for _ in 0..300 {
            server.step(0.05);
            while let Some(msg) = conn.poll::<ServerMsg>().unwrap() {
                let done = until(&msg);
                got.push(msg);
                if done {
                    return got;
                }
            }
            thread::sleep(Duration::from_millis(2));
        }
        panic!("timed out; got {got:?}");
    }

    fn connect(server: &Server, account: &str) -> Connection {
        let addr = server.local_addr().unwrap().to_string();
        let mut conn = Connection::connect(&addr, DEFAULT_PORT).unwrap();
        conn.send(&ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            account: account.into(),
        })
        .unwrap();
        conn
    }

    #[test]
    fn create_enter_logout_and_keep_progress() {
        let mut server = Server::bind("127.0.0.1:0", Store::in_memory()).unwrap();
        server.verbose = false;
        let mut conn = connect(&server, "Ann");
        let got = pump(&mut server, &mut conn, |m| {
            matches!(m, ServerMsg::Characters(_))
        });
        assert!(matches!(got.last(), Some(ServerMsg::Characters(list)) if list.is_empty()));

        conn.send(&ClientMsg::CreateCharacter {
            name: "aria".into(),
            class: Class::Cleric,
            appearance: Appearance::default(),
        })
        .unwrap();
        let got = pump(&mut server, &mut conn, |m| {
            matches!(m, ServerMsg::Characters(_))
        });
        assert!(
            matches!(got.last(), Some(ServerMsg::Characters(list)) if list.len() == 1 && list[0].name == "Aria")
        );

        conn.send(&ClientMsg::EnterWorld("Aria".into())).unwrap();
        let got = pump(&mut server, &mut conn, |m| {
            matches!(m, ServerMsg::Snapshot(_))
        });
        let id = got
            .iter()
            .find_map(|m| match m {
                ServerMsg::Welcome { id } => Some(*id),
                _ => None,
            })
            .expect("welcomed");
        server.world.give_xp(id, 150, "test");

        conn.send(&ClientMsg::Logout).unwrap();
        let got = pump(&mut server, &mut conn, |m| {
            matches!(m, ServerMsg::Characters(_))
        });
        assert!(matches!(got.last(), Some(ServerMsg::Characters(list)) if list[0].level == 2));
        assert!(server.world.entities.values().all(|e| e.player().is_none()));
    }

    #[test]
    fn names_are_unique_across_accounts() {
        let mut server = Server::bind("127.0.0.1:0", Store::in_memory()).unwrap();
        server.verbose = false;
        server
            .store
            .create("ann", "Aria", Class::Mage, Appearance::default())
            .unwrap();
        let mut conn = connect(&server, "bob");
        pump(&mut server, &mut conn, |m| {
            matches!(m, ServerMsg::Characters(_))
        });
        conn.send(&ClientMsg::CreateCharacter {
            name: "Aria".into(),
            class: Class::Rogue,
            appearance: Appearance::default(),
        })
        .unwrap();
        let got = pump(&mut server, &mut conn, |m| {
            matches!(m, ServerMsg::CharacterError(_))
        });
        assert!(matches!(got.last(), Some(ServerMsg::CharacterError(e)) if e.contains("taken")));
        // And bob can't play ann's character.
        conn.send(&ClientMsg::EnterWorld("Aria".into())).unwrap();
        pump(&mut server, &mut conn, |m| {
            matches!(m, ServerMsg::CharacterError(_))
        });
    }

    #[test]
    fn wrong_version_is_rejected() {
        let mut server = Server::bind("127.0.0.1:0", Store::in_memory()).unwrap();
        server.verbose = false;
        let addr = server.local_addr().unwrap().to_string();
        let mut conn = Connection::connect(&addr, DEFAULT_PORT).unwrap();
        conn.send(&ClientMsg::Hello {
            version: PROTOCOL_VERSION + 1,
            account: "old".into(),
        })
        .unwrap();
        let mut rejected = false;
        for _ in 0..200 {
            server.step(0.05);
            match conn.poll::<ServerMsg>() {
                Ok(Some(ServerMsg::Rejected(_))) => {
                    rejected = true;
                    break;
                }
                Ok(_) => thread::sleep(Duration::from_millis(2)),
                Err(_) => break,
            }
        }
        assert!(rejected);
    }
}
