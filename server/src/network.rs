//! Accepts connections, runs the world at a fixed tick rate and routes messages.

use std::collections::HashMap;
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use shared::net::Connection;
use shared::protocol::*;
use shared::world::VIEW_DISTANCE;

use crate::TICK_RATE;
use crate::world::{Audience, World};

/// Connections that haven't said hello by now are dropped.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

struct Client {
    conn: Connection,
    addr: SocketAddr,
    player: Option<EntityId>,
    connected_at: Instant,
}

pub struct Server {
    listener: TcpListener,
    clients: HashMap<u64, Client>,
    next_client: u64,
    pub world: World,
    /// Print connections and disconnections.
    pub verbose: bool,
}

impl Server {
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
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

    /// One server tick: network in, simulate, network out.
    pub fn step(&mut self, dt: f32) {
        self.accept();
        self.receive();
        self.world.tick(dt);
        self.deliver();
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
                match (client.player, msg) {
                    (
                        None,
                        ClientMsg::Hello {
                            version,
                            name,
                            class,
                        },
                    ) => {
                        if version != PROTOCOL_VERSION {
                            let reason = format!(
                                "Version mismatch: the server speaks protocol {PROTOCOL_VERSION}, you have {version}. Update your game."
                            );
                            let _ = client.conn.send(&ServerMsg::Rejected(reason));
                            dropped.push(cid);
                            break;
                        }
                        let id = self.world.add_player(&name, class);
                        let client = self.clients.get_mut(&cid).unwrap();
                        client.player = Some(id);
                        let _ = client.conn.send(&ServerMsg::Welcome { id });
                        let msg = format!(
                            "{} joined as {} ({:?})",
                            client.addr, self.world.entities[&id].name, class
                        );
                        self.log(msg);
                    }
                    (None, _) => {}
                    (Some(id), msg) => self.world.handle(id, msg),
                }
            }
            let client = &self.clients[&cid];
            if client.player.is_none() && client.connected_at.elapsed() > HELLO_TIMEOUT {
                dropped.push(cid);
            }
        }
        for cid in dropped {
            if let Some(client) = self.clients.remove(&cid) {
                if let Some(id) = client.player {
                    self.world.remove_player(id);
                }
                self.log(format!("{} disconnected", client.addr));
            }
        }
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
            if let Some(client) = self.clients.remove(&cid) {
                if let Some(id) = client.player {
                    self.world.remove_player(id);
                }
                self.log(format!("{} dropped", client.addr));
            }
        }
    }

    fn log(&self, msg: String) {
        if self.verbose {
            println!("{msg}");
        }
    }
}

/// Starts a server on a background thread, listening on localhost only.
/// Used by the client's single-player mode.
pub fn spawn_local() -> io::Result<SocketAddr> {
    let mut server = Server::bind("127.0.0.1:0")?;
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
    use shared::data::Class;

    #[test]
    fn client_joins_and_gets_snapshots() {
        let mut server = Server::bind("127.0.0.1:0").unwrap();
        server.verbose = false;
        let addr = server.local_addr().unwrap().to_string();
        let mut conn = Connection::connect(&addr, DEFAULT_PORT).unwrap();
        conn.send(&ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            name: "Net".into(),
            class: Class::Cleric,
        })
        .unwrap();

        let mut my_id = None;
        let mut snapshot = None;
        for _ in 0..200 {
            server.step(0.05);
            while let Some(msg) = conn.poll::<ServerMsg>().unwrap() {
                match msg {
                    ServerMsg::Welcome { id } => my_id = Some(id),
                    ServerMsg::Snapshot(s) => snapshot = Some(s),
                    _ => {}
                }
            }
            if snapshot.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        let id = my_id.expect("welcomed");
        let snapshot = snapshot.expect("got a snapshot");
        let me = snapshot.entities.iter().find(|e| e.id == id).unwrap();
        assert_eq!(me.name, "Net");
        assert_eq!(me.kind, EntityKind::Player(Class::Cleric));
    }

    #[test]
    fn wrong_version_is_rejected() {
        let mut server = Server::bind("127.0.0.1:0").unwrap();
        server.verbose = false;
        let addr = server.local_addr().unwrap().to_string();
        let mut conn = Connection::connect(&addr, DEFAULT_PORT).unwrap();
        conn.send(&ClientMsg::Hello {
            version: PROTOCOL_VERSION + 1,
            name: "Old".into(),
            class: Class::Mage,
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
        assert!(server.world.entities.values().all(|e| e.player().is_none()));
    }
}
