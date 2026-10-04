//! The screens before the game: login (account and server), character
//! select, and character creation, all over a view of a starting town.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use macroquad::prelude::*;
use shared::data::{Appearance, Class, Race};
use shared::net::Connection;
use shared::protocol::*;
use shared::world::Zone;

use crate::game::Game;
use crate::hud::{self, BORDER, GOLD, button, button_ex, panel, text, text_centered};
use crate::render::{self, Batch, Look, Pose, Scene};

/// Where saves and settings live: `%APPDATA%\RustyMMO` on Windows,
/// `~/Library/Application Support/RustyMMO` on macOS and
/// `~/.local/share/rusty-mmo` elsewhere.
pub fn data_dir() -> PathBuf {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        if let Some(d) = env("APPDATA") {
            return d.join("RustyMMO");
        }
    } else if cfg!(target_os = "macos") {
        if let Some(h) = env("HOME") {
            return h.join("Library/Application Support/RustyMMO");
        }
    } else if let Some(d) = env("XDG_DATA_HOME") {
        return d.join("rusty-mmo");
    } else if let Some(h) = env("HOME") {
        return h.join(".local/share/rusty-mmo");
    }
    PathBuf::from(".")
}

/// The account and server last used, remembered between runs.
fn load_settings() -> (String, String) {
    let text = std::fs::read_to_string(data_dir().join("client.txt")).unwrap_or_default();
    let mut lines = text.lines();
    let account = lines.next().unwrap_or("").to_string();
    let address = lines
        .next()
        .filter(|a| !a.is_empty())
        .unwrap_or("127.0.0.1")
        .to_string();
    (account, address)
}

fn save_settings(account: &str, address: &str) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("client.txt"), format!("{account}\n{address}\n"));
}

/// The menu camera slowly circles a town, or shows a character close up
/// in the town square of their race's starting area.
fn draw_backdrop(
    scene: &Scene,
    zone: Zone,
    time: f32,
    batch: &mut Batch,
    preview: Option<(&Look, f32)>,
) {
    let a = time * 0.04;
    // Points in the town's own layout, placed in the world.
    let at = |x: f32, y: f32, z: f32| {
        let w = zone.to_world(vec2(x, z));
        vec3(w.x, y, w.y)
    };
    let ground = zone.ground_local(vec2(-3.4, 4.0));
    let (position, target) = match preview {
        Some(_) => (at(-2.2, ground.y + 1.9, 9.6), at(-2.6, ground.y + 1.2, 4.0)),
        None => (at(a.cos() * 55.0, 22.0, a.sin() * 55.0), at(0.0, 2.0, 0.0)),
    };
    let cam = Camera3D {
        position,
        target,
        up: Vec3::Y,
        fovy: 0.9,
        z_near: 0.1,
        z_far: 1500.0,
        ..Default::default()
    };
    render::draw_sky(&cam, zone, time, |p| hud::project(&cam, p));
    set_camera(&cam);
    scene.begin_3d(zone);
    scene.draw(zone);
    if let Some((look, yaw)) = preview {
        let pose = Pose {
            time,
            ..Default::default()
        };
        render::draw_model(batch, look, ground, zone.yaw_to_world(yaw), pose);
    }
    scene.draw_effects(zone, batch, time, target);
    batch.flush();
    scene.draw_water(zone);
    scene.end_3d();
    set_default_camera();
}

/// A selectable button in a grid of choices.
fn choice(r: Rect, label: &str, selected: bool, color: Color, size: f32) {
    let hover = r.contains(mouse());
    let bg = if selected {
        Color::new(color.r * 0.45, color.g * 0.45, color.b * 0.45, 1.0)
    } else {
        Color::new(0.15, 0.15, 0.18, 1.0)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle_lines(
        r.x,
        r.y,
        r.w,
        r.h,
        2.0,
        if selected || hover { color } else { BORDER },
    );
    text_centered(
        label,
        r.x + r.w / 2.0,
        r.y + r.h / 2.0 + size * 0.3,
        size,
        if selected { WHITE } else { color },
    );
}

fn input_box(r: Rect, value: &str, placeholder: &str, focused: bool, time: f32) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if focused { GOLD } else { BORDER });
    if value.is_empty() && !focused {
        text(
            placeholder,
            r.x + 10.0,
            r.y + 25.0,
            22.0,
            Color::new(0.5, 0.5, 0.5, 1.0),
        );
    } else {
        let cursor = if focused && (time * 2.0) as i32 % 2 == 0 {
            "_"
        } else {
            ""
        };
        text(
            &format!("{value}{cursor}"),
            r.x + 10.0,
            r.y + 25.0,
            22.0,
            WHITE,
        );
    }
}

fn mouse() -> Vec2 {
    vec2(mouse_position().0, mouse_position().1)
}

/// Types into a text field. `allow` filters characters.
fn type_into(field: &mut String, max: usize, allow: impl Fn(char) -> bool) {
    while let Some(c) = get_char_pressed() {
        if allow(c) && field.chars().count() < max {
            field.push(c);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        field.pop();
    }
}

// ---- Login ----

/// How you're playing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// Your solo characters and your characters on a server, in one list.
    Play,
    /// A private world with cheats, and its own characters.
    Sandbox,
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Account,
    Address,
}

pub struct Login {
    account: String,
    address: String,
    focus: Field,
    message: Option<String>,
    time: f32,
    /// The single-player servers, once started: solo and sandbox.
    local: [Option<SocketAddr>; 2],
    batch: Batch,
}

impl Login {
    pub fn new() -> Self {
        let (account, address) = load_settings();
        Self {
            account,
            address,
            focus: Field::Account,
            message: None,
            time: 0.0,
            local: [None; 2],
            batch: Batch::new(),
        }
    }

    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    pub fn frame(&mut self, scene: &Scene) -> Option<Characters> {
        self.time += get_frame_time();
        draw_backdrop(scene, Zone::Amberfall, self.time, &mut self.batch, None);

        let (w, h) = (screen_width(), screen_height());
        let p = Rect::new(
            (w - 440.0) / 2.0,
            ((h - 440.0) / 2.0).max(10.0),
            440.0,
            440.0,
        );
        let inner = p.x + 24.0;
        let iw = p.w - 48.0;
        let account_box = Rect::new(inner, p.y + 132.0, iw, 36.0);
        let address_box = Rect::new(inner, p.y + 212.0, iw, 36.0);
        let play = Rect::new(inner, p.y + 270.0, iw, 44.0);
        let sandbox = Rect::new(inner, p.y + 324.0, iw * 0.5 - 6.0, 40.0);
        let quit = Rect::new(inner + iw * 0.5 + 6.0, p.y + 324.0, iw * 0.5 - 6.0, 40.0);

        match self.focus {
            Field::Account => type_into(&mut self.account, 24, |c| !c.is_control()),
            Field::Address => type_into(&mut self.address, 64, |c| {
                !c.is_control() && !c.is_whitespace()
            }),
        }
        if is_key_pressed(KeyCode::Tab) {
            self.focus = if self.focus == Field::Account {
                Field::Address
            } else {
                Field::Account
            };
        }
        let mut start = None;
        if is_mouse_button_pressed(MouseButton::Left) {
            let m = mouse();
            if account_box.contains(m) {
                self.focus = Field::Account;
            } else if address_box.contains(m) {
                self.focus = Field::Address;
            } else if play.contains(m) {
                start = Some(Mode::Play);
            } else if sandbox.contains(m) {
                start = Some(Mode::Sandbox);
            } else if quit.contains(m) {
                std::process::exit(0);
            }
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
            start = Some(Mode::Play);
        }

        panel(p);
        let cx = p.x + p.w / 2.0;
        text_centered("Rusty MMO", cx, p.y + 54.0, 48.0, GOLD);
        text_centered(
            &format!("Six starting areas  -  {}", shared::VERSION),
            cx,
            p.y + 82.0,
            18.0,
            Color::new(0.88, 0.82, 0.75, 1.0),
        );
        text(
            "Account name",
            account_box.x,
            account_box.y - 8.0,
            18.0,
            WHITE,
        );
        input_box(
            account_box,
            &self.account,
            "Player",
            self.focus == Field::Account,
            self.time,
        );
        text(
            "Server address (empty to play solo only)",
            address_box.x,
            address_box.y - 8.0,
            18.0,
            WHITE,
        );
        input_box(
            address_box,
            &self.address,
            "host:port",
            self.focus == Field::Address,
            self.time,
        );
        button(play, "Play");
        button(sandbox, "Sandbox");
        button(quit, "Quit");
        text_centered(
            "Solo characters are saved on this computer, the rest on their server.",
            cx,
            p.bottom() - 18.0,
            15.0,
            Color::new(0.7, 0.7, 0.7, 1.0),
        );
        if let Some(msg) = &self.message {
            for (i, line) in hud::wrap(msg, 60).iter().enumerate() {
                text_centered(
                    line,
                    cx,
                    p.bottom() + 28.0 + i as f32 * 22.0,
                    20.0,
                    Color::new(1.0, 0.45, 0.4, 1.0),
                );
            }
        }

        let mode = start?;
        match self.connect(mode) {
            Ok(screen) => Some(screen),
            Err(e) => {
                self.message = Some(e);
                None
            }
        }
    }

    /// The address of a single-player server, starting it the first time.
    fn local_server(&mut self, mode: Mode) -> Result<SocketAddr, String> {
        let (slot, file, sandbox) = match mode {
            Mode::Sandbox => (1, "sandbox_characters.json", true),
            Mode::Play => (0, "solo_characters.json", false),
        };
        if let Some(a) = self.local[slot] {
            return Ok(a);
        }
        let save = data_dir().join(file);
        let a = server::spawn_local(save, sandbox)
            .map_err(|e| format!("Couldn't start the local server: {e}"))?;
        self.local[slot] = Some(a);
        Ok(a)
    }

    fn connect(&mut self, mode: Mode) -> Result<Characters, String> {
        let account = if self.account.trim().is_empty() {
            "Player".to_string()
        } else {
            self.account.trim().to_string()
        };
        save_settings(&account, &self.address);
        let local = self.local_server(mode)?;
        let mut sources = vec![Source {
            place: if mode == Mode::Sandbox {
                Place::Sandbox
            } else {
                Place::Solo
            },
            link: Link::Open(open(&local.to_string(), &account)?),
            list: None,
        }];
        let address = self.address.trim();
        if mode == Mode::Play && !address.is_empty() {
            sources.push(Source {
                place: Place::Online(address.to_string()),
                link: Link::connecting(address.to_string(), account.clone()),
                list: None,
            });
        }
        Ok(Characters::new(account, sources))
    }
}

/// Connects to a server and says hello.
fn open(addr: &str, account: &str) -> Result<Connection, String> {
    let mut conn = Connection::connect(addr, DEFAULT_PORT)
        .map_err(|e| format!("Couldn't connect to {addr}: {e}"))?;
    conn.send(&ClientMsg::Hello {
        version: PROTOCOL_VERSION,
        account: account.to_string(),
    })
    .map_err(|e| format!("Couldn't talk to {addr}: {e}"))?;
    Ok(conn)
}

// ---- Character select and creation ----

/// Where a character lives.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Place {
    /// On this computer.
    Solo,
    /// On this computer, in the world with cheats.
    Sandbox,
    /// On the server at this address.
    Online(String),
}

impl Place {
    /// The tag next to a character in the list.
    fn label(&self) -> &str {
        match self {
            Place::Solo => "Solo",
            Place::Sandbox => "Sandbox",
            Place::Online(addr) => addr,
        }
    }

    fn color(&self) -> Color {
        match self {
            Place::Solo | Place::Sandbox => Color::new(0.65, 0.85, 0.6, 1.0),
            Place::Online(_) => Color::new(0.55, 0.75, 1.0, 1.0),
        }
    }
}

enum Link {
    /// Connecting in the background, so a slow or missing server doesn't
    /// freeze the screen.
    Connecting(Receiver<Result<Connection, String>>),
    Open(Connection),
    /// The connection is in the game.
    Playing,
    /// Couldn't connect, or lost the connection: why.
    Down(String),
}

impl Link {
    fn connecting(addr: String, account: String) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(open(&addr, &account).map_err(|_| "offline".to_string()));
        });
        Link::Connecting(rx)
    }
}

/// One place characters come from, and the ones it has.
struct Source {
    place: Place,
    link: Link,
    /// `None` until the list arrives.
    list: Option<Vec<CharacterSummary>>,
}

struct Create {
    name: String,
    class: Class,
    appearance: Appearance,
    /// Which source to make it in.
    source: usize,
}

pub struct Characters {
    account: String,
    /// Solo or sandbox first, then the server if there is one.
    sources: Vec<Source>,
    /// The chosen character: its source and name.
    selected: Option<(usize, String)>,
    /// The first row shown, when there are more than fit.
    scroll: usize,
    create: Option<Create>,
    confirm_delete: bool,
    message: Option<String>,
    time: f32,
    spin: f32,
    batch: Batch,
}

pub enum CharacterOutcome {
    Stay,
    Back(Option<String>),
    Play(Box<Game>),
}

/// One row of the merged list.
struct Entry {
    source: usize,
    character: CharacterSummary,
}

impl Characters {
    fn new(account: String, sources: Vec<Source>) -> Self {
        Self {
            account,
            sources,
            selected: None,
            scroll: 0,
            create: None,
            confirm_delete: false,
            message: None,
            time: 0.0,
            spin: 0.0,
            batch: Batch::new(),
        }
    }

    /// Back from the game: the connection returns to the source it was
    /// playing on, and the list comes again from every source.
    pub fn resume(&mut self, conn: Connection) {
        for s in &mut self.sources {
            if matches!(s.link, Link::Playing) {
                s.link = Link::Open(conn);
                // The server sends a fresh list after logout.
                s.list = None;
                break;
            }
        }
        self.message = None;
        self.create = None;
        self.confirm_delete = false;
    }

    fn entries(&self) -> Vec<Entry> {
        self.sources
            .iter()
            .enumerate()
            .flat_map(|(i, s)| {
                s.list.iter().flatten().map(move |c| Entry {
                    source: i,
                    character: c.clone(),
                })
            })
            .collect()
    }

    fn selected_index(&self, entries: &[Entry]) -> usize {
        self.selected
            .as_ref()
            .and_then(|(src, name)| {
                entries
                    .iter()
                    .position(|e| e.source == *src && e.character.name == *name)
            })
            .unwrap_or(0)
    }

    fn select(&mut self, e: &Entry) {
        self.selected = Some((e.source, e.character.name.clone()));
    }

    fn send(&mut self, source: usize, msg: ClientMsg) {
        if let Link::Open(conn) = &mut self.sources[source].link {
            let _ = conn.send(&msg);
        }
    }

    /// A source lost its server. With only one, go back to the login screen.
    fn source_down(&mut self, i: usize, why: String) -> Option<CharacterOutcome> {
        if self.sources.len() == 1 {
            return Some(CharacterOutcome::Back(Some(why)));
        }
        self.sources[i].link = Link::Down(why);
        self.sources[i].list = None;
        if self.create.as_ref().is_some_and(|c| c.source == i) {
            self.create.as_mut().unwrap().source = 0;
        }
        None
    }

    fn got_list(&mut self, i: usize, list: Vec<CharacterSummary>) {
        // A character was just made here: select it and close the form.
        if let (Some(old), Some(c)) = (&self.sources[i].list, &self.create)
            && c.source == i
            && list.len() > old.len()
        {
            if let Some(new) = list.iter().find(|s| s.name.eq_ignore_ascii_case(&c.name)) {
                self.selected = Some((i, new.name.clone()));
            }
            self.create = None;
        }
        self.sources[i].list = Some(list);
    }

    /// Reads what each server sent.
    fn network(&mut self) -> Option<CharacterOutcome> {
        for i in 0..self.sources.len() {
            if let Link::Connecting(rx) = &self.sources[i].link {
                let result = rx.try_recv();
                match result {
                    Ok(Ok(conn)) => self.sources[i].link = Link::Open(conn),
                    Ok(Err(e)) => self.sources[i].link = Link::Down(e),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        self.sources[i].link = Link::Down("Couldn't connect.".into())
                    }
                }
            }
            while let Link::Open(conn) = &mut self.sources[i].link {
                match conn.poll::<ServerMsg>() {
                    Ok(Some(ServerMsg::Characters(list))) => self.got_list(i, list),
                    Ok(Some(ServerMsg::CharacterError(e))) => self.message = Some(e),
                    Ok(Some(ServerMsg::Rejected(e))) => {
                        if let Some(out) = self.source_down(i, e) {
                            return Some(out);
                        }
                    }
                    Ok(Some(ServerMsg::Welcome { id })) => {
                        let Link::Open(conn) =
                            std::mem::replace(&mut self.sources[i].link, Link::Playing)
                        else {
                            unreachable!()
                        };
                        let entries = self.entries();
                        let class = entries
                            .get(self.selected_index(&entries))
                            .map_or(Class::Barbarian, |e| e.character.class);
                        let mut game = Game::new(conn, class);
                        game.my_id = Some(id);
                        return Some(CharacterOutcome::Play(Box::new(game)));
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(e) => {
                        let why = format!("Lost connection to the server ({e}).");
                        if let Some(out) = self.source_down(i, why) {
                            return Some(out);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn frame(&mut self, scene: &Scene) -> CharacterOutcome {
        let dt = get_frame_time();
        self.time += dt;
        self.spin += dt * 0.5;
        if let Some(out) = self.network() {
            return out;
        }
        let entries = self.entries();
        // Nothing to pick from once every source has answered: make one.
        let settled = self
            .sources
            .iter()
            .all(|s| s.list.is_some() || matches!(s.link, Link::Down(_) | Link::Playing));
        if settled && entries.is_empty() && self.create.is_none() {
            self.create = Some(self.new_character());
        }

        let preview = match &self.create {
            Some(c) => Some(Look {
                kind: EntityKind::Player(c.class),
                appearance: c.appearance,
                gear: [None; 5],
                seed: 0,
            }),
            None => entries.get(self.selected_index(&entries)).map(|e| Look {
                kind: EntityKind::Player(e.character.class),
                appearance: e.character.appearance,
                gear: e.character.gear,
                seed: 0,
            }),
        };
        let yaw = 0.4 + (self.spin * 0.8).sin() * 0.6;
        let zone = preview
            .as_ref()
            .map_or(Zone::Amberfall, |l| l.appearance.race.zone());
        draw_backdrop(
            scene,
            zone,
            self.time,
            &mut self.batch,
            preview.as_ref().map(|l| (l, yaw)),
        );

        // Wait for the characters on this computer; a server can follow.
        if self.sources[0].list.is_none() {
            text_centered(
                "Loading characters...",
                screen_width() / 2.0,
                screen_height() / 2.0,
                30.0,
                WHITE,
            );
            return CharacterOutcome::Stay;
        }
        let outcome = if self.create.is_some() {
            self.create_screen(!entries.is_empty())
        } else {
            self.select_screen(&entries)
        };
        if let Some(msg) = &self.message {
            text_centered(
                msg,
                screen_width() / 2.0,
                screen_height() - 40.0,
                22.0,
                Color::new(1.0, 0.45, 0.4, 1.0),
            );
        }
        outcome
    }

    /// A blank character, made where the selected one lives.
    fn new_character(&self) -> Create {
        let source = self
            .selected
            .as_ref()
            .map_or(0, |(s, _)| *s)
            .min(self.sources.len() - 1);
        let source = if matches!(self.sources[source].link, Link::Open(_)) {
            source
        } else {
            0
        };
        Create {
            name: String::new(),
            class: Class::Barbarian,
            appearance: Appearance::default(),
            source,
        }
    }

    /// The servers that aren't showing characters, and why.
    fn server_status(&self) -> Option<String> {
        self.sources.iter().find_map(|s| {
            let Place::Online(addr) = &s.place else {
                return None;
            };
            match &s.link {
                Link::Connecting(_) => Some(format!("Connecting to {addr}...")),
                Link::Open(_) if s.list.is_none() => Some(format!("Connecting to {addr}...")),
                Link::Down(why) => Some(format!("Server {addr}: {why}")),
                _ => None,
            }
        })
    }

    fn select_screen(&mut self, entries: &[Entry]) -> CharacterOutcome {
        let (w, h) = (screen_width(), screen_height());
        let p = Rect::new(w - 380.0, 30.0, 350.0, h - 60.0);
        panel(p);
        text_centered("Characters", p.x + p.w / 2.0, p.y + 36.0, 30.0, GOLD);
        let where_ = if self.sources[0].place == Place::Sandbox {
            "Sandbox: press P in game for cheats".to_string()
        } else {
            format!("Account: {}", self.account)
        };
        let grey = Color::new(0.75, 0.75, 0.75, 1.0);
        text_centered(&where_, p.x + p.w / 2.0, p.y + 60.0, 16.0, grey);
        if let Some(status) = self.server_status() {
            for (i, line) in hud::wrap(&status, 40).iter().take(3).enumerate() {
                text_centered(
                    line,
                    p.x + p.w / 2.0,
                    p.y + 78.0 + i as f32 * 16.0,
                    15.0,
                    Color::new(0.95, 0.7, 0.5, 1.0),
                );
            }
        }
        let create = Rect::new(p.x + 14.0, p.bottom() - 112.0, p.w - 28.0, 40.0);
        let top = p.y + 126.0;
        let fits = (((create.y - 8.0 - top) / 60.0).floor() as usize).max(1);
        let selected = self.selected_index(entries);
        // Keep the selected row in view.
        self.scroll = self
            .scroll
            .min(entries.len().saturating_sub(fits))
            .min(selected)
            .max((selected + 1).saturating_sub(fits));
        let rows: Vec<(usize, Rect)> = (self.scroll..entries.len().min(self.scroll + fits))
            .enumerate()
            .map(|(row, i)| {
                (
                    i,
                    Rect::new(p.x + 14.0, top + row as f32 * 60.0, p.w - 28.0, 54.0),
                )
            })
            .collect();
        let enter = Rect::new(w / 2.0 - 120.0, h - 110.0, 240.0, 48.0);
        let delete = Rect::new(p.x + 14.0, p.bottom() - 62.0, (p.w - 40.0) / 2.0, 40.0);
        let back = Rect::new(
            p.x + 26.0 + (p.w - 40.0) / 2.0,
            p.bottom() - 62.0,
            (p.w - 40.0) / 2.0,
            40.0,
        );

        let wheel = mouse_wheel().1;
        if wheel != 0.0 && entries.len() > fits {
            let pick = if wheel < 0.0 {
                (selected + 1).min(entries.len() - 1)
            } else {
                selected.saturating_sub(1)
            };
            self.select(&entries[pick]);
        }
        if is_mouse_button_pressed(MouseButton::Left) {
            let m = mouse();
            if let Some(&(i, _)) = rows.iter().find(|(_, r)| r.contains(m)) {
                self.select(&entries[i]);
                self.confirm_delete = false;
            } else if enter.contains(m) && !entries.is_empty() {
                self.enter(&entries[selected]);
            } else if create.contains(m) {
                self.create = Some(self.new_character());
                self.message = None;
            } else if delete.contains(m) && !entries.is_empty() {
                if self.confirm_delete {
                    let e = &entries[selected];
                    self.send(
                        e.source,
                        ClientMsg::DeleteCharacter(e.character.name.clone()),
                    );
                    self.confirm_delete = false;
                } else {
                    self.confirm_delete = true;
                }
            } else if back.contains(m) {
                return CharacterOutcome::Back(None);
            }
        }
        if !entries.is_empty() {
            if is_key_pressed(KeyCode::Down) {
                self.select(&entries[(selected + 1) % entries.len()]);
                self.confirm_delete = false;
            }
            if is_key_pressed(KeyCode::Up) {
                self.select(&entries[(selected + entries.len() - 1) % entries.len()]);
                self.confirm_delete = false;
            }
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                self.enter(&entries[selected]);
            }
        }
        if is_key_pressed(KeyCode::Escape) {
            return CharacterOutcome::Back(None);
        }
        while get_char_pressed().is_some() {}

        // Draw with the selection as it is after this frame's input.
        let selected = self.selected_index(entries);
        for (i, r) in &rows {
            let e = &entries[*i];
            let c = &e.character;
            let is_selected = *i == selected;
            draw_rectangle(
                r.x,
                r.y,
                r.w,
                r.h,
                if is_selected {
                    Color::new(0.35, 0.25, 0.1, 0.9)
                } else {
                    Color::new(0.0, 0.0, 0.0, 0.4)
                },
            );
            draw_rectangle_lines(
                r.x,
                r.y,
                r.w,
                r.h,
                2.0,
                if is_selected { GOLD } else { BORDER },
            );
            text(
                &c.name,
                r.x + 12.0,
                r.y + 24.0,
                24.0,
                hud::class_color(c.class),
            );
            let place = &self.sources[e.source].place;
            let tag = place.label();
            text(
                tag,
                r.right() - 10.0 - hud::text_width(tag, 15.0),
                r.y + 20.0,
                15.0,
                place.color(),
            );
            text(
                &format!(
                    "Level {} {} {}",
                    c.level,
                    c.appearance.race.name(),
                    c.class.name()
                ),
                r.x + 12.0,
                r.y + 44.0,
                17.0,
                Color::new(0.85, 0.85, 0.85, 1.0),
            );
        }
        if self.scroll > 0 {
            text_centered("more above", p.x + p.w / 2.0, top - 4.0, 14.0, grey);
        }
        if self.scroll + fits < entries.len() {
            text_centered("more below", p.x + p.w / 2.0, create.y - 2.0, 14.0, grey);
        }
        if entries.is_empty() {
            text_centered(
                "No characters yet.",
                p.x + p.w / 2.0,
                top + 30.0,
                20.0,
                WHITE,
            );
        }
        button(create, "Create New Character");
        button_ex(
            delete,
            if self.confirm_delete {
                "Really delete?"
            } else {
                "Delete"
            },
            !entries.is_empty(),
        );
        button(back, "Back");
        if let Some(e) = entries.get(selected) {
            text_centered(
                &e.character.name,
                w / 2.0 - 60.0,
                h - 150.0,
                40.0,
                hud::class_color(e.character.class),
            );
        }
        button_ex(enter, "Enter World", !entries.is_empty());
        CharacterOutcome::Stay
    }

    fn enter(&mut self, e: &Entry) {
        self.message = None;
        self.select(e);
        self.send(e.source, ClientMsg::EnterWorld(e.character.name.clone()));
    }

    fn create_screen(&mut self, has_characters: bool) -> CharacterOutcome {
        let (w, h) = (screen_width(), screen_height());
        let p = Rect::new(w - 420.0, 30.0, 390.0, h - 60.0);
        let inner = p.x + 20.0;
        let iw = p.w - 40.0;
        let name_box = Rect::new(inner, p.y + 92.0, iw, 36.0);
        // A grid of buttons, three to a row.
        let grid = |count: usize, top: f32, height: f32| -> Vec<Rect> {
            let cw = (iw - 12.0) / 3.0;
            (0..count)
                .map(|i| {
                    Rect::new(
                        inner + (i % 3) as f32 * (cw + 6.0),
                        top + (i / 3) as f32 * (height + 4.0),
                        cw,
                        height,
                    )
                })
                .collect()
        };
        let races = grid(Race::ALL.len(), p.y + 160.0, 30.0);
        let classes = grid(Class::ALL.len(), p.y + 252.0, 28.0);
        let options_y = p.y + 428.0;
        let option_rows: Vec<(Rect, Rect)> = (0..4)
            .map(|i| {
                let y = options_y + i as f32 * 38.0;
                (
                    Rect::new(inner + iw - 170.0, y, 34.0, 32.0),
                    Rect::new(inner + iw - 34.0, y, 34.0, 32.0),
                )
            })
            .collect();
        // Where to make it, when there's a server as well as solo.
        let places: Vec<Rect> = if self.sources.len() > 1 {
            let cw = (iw - 6.0) / 2.0;
            (0..self.sources.len())
                .map(|i| Rect::new(inner + i as f32 * (cw + 6.0), p.y + 616.0, cw, 30.0))
                .collect()
        } else {
            Vec::new()
        };
        let available: Vec<bool> = self
            .sources
            .iter()
            .map(|s| matches!(s.link, Link::Open(_)) && s.list.is_some())
            .collect();
        let create = Rect::new(inner, p.bottom() - 62.0, iw * 0.5 - 6.0, 44.0);
        let back = Rect::new(
            inner + iw * 0.5 + 6.0,
            p.bottom() - 62.0,
            iw * 0.5 - 6.0,
            44.0,
        );

        let c = self.create.as_mut().unwrap();
        if !available[c.source] {
            c.source = 0;
        }
        type_into(&mut c.name, 12, |ch| ch.is_ascii_alphabetic());
        let mut submit = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if is_mouse_button_pressed(MouseButton::Left) {
            let m = mouse();
            for (r, class) in classes.iter().zip(Class::ALL) {
                if r.contains(m) {
                    c.class = class;
                }
            }
            for (r, race) in races.iter().zip(Race::ALL) {
                if r.contains(m) {
                    c.appearance.race = race;
                }
            }
            for (i, r) in places.iter().enumerate() {
                if r.contains(m) && available[i] {
                    c.source = i;
                }
            }
            let a = &mut c.appearance;
            let fields: [(&mut u8, u8); 4] = [
                (&mut a.body, Appearance::BODIES),
                (&mut a.skin, Appearance::SKINS),
                (&mut a.hair_style, Appearance::HAIR_STYLES),
                (&mut a.hair_color, Appearance::HAIR_COLORS),
            ];
            for ((value, count), (prev, next)) in fields.into_iter().zip(&option_rows) {
                if prev.contains(m) {
                    *value = (*value + count - 1) % count;
                } else if next.contains(m) {
                    *value = (*value + 1) % count;
                }
            }
            if create.contains(m) {
                submit = true;
            } else if back.contains(m) && has_characters {
                self.create = None;
                self.message = None;
                return CharacterOutcome::Stay;
            }
        }
        if is_key_pressed(KeyCode::Escape) && has_characters {
            self.create = None;
            self.message = None;
            return CharacterOutcome::Stay;
        }
        let (name, class, appearance, source) = (c.name.clone(), c.class, c.appearance, c.source);
        if submit {
            if name.len() < 2 {
                self.message = Some("Pick a name of 2 to 12 letters.".into());
            } else {
                self.message = None;
                self.send(
                    source,
                    ClientMsg::CreateCharacter {
                        name: name.clone(),
                        class,
                        appearance,
                    },
                );
            }
        }

        panel(p);
        text_centered(
            "Create a Character",
            p.x + p.w / 2.0,
            p.y + 36.0,
            28.0,
            GOLD,
        );
        let race = appearance.race;
        text_centered(
            &format!("{} - starts in {}", race.name(), race.zone().name()),
            p.x + p.w / 2.0,
            p.y + 58.0,
            16.0,
            Color::new(0.8, 0.8, 0.8, 1.0),
        );
        text("Name", name_box.x, name_box.y - 8.0, 18.0, WHITE);
        input_box(name_box, &name, "Letters only", true, self.time);
        text("Race", inner, p.y + 152.0, 18.0, WHITE);
        let race_color = Color::new(0.85, 0.75, 0.55, 1.0);
        for (r, rc) in races.iter().zip(Race::ALL) {
            choice(*r, rc.name(), rc == race, race_color, 18.0);
        }
        text("Class", inner, p.y + 244.0, 18.0, WHITE);
        for (r, cl) in classes.iter().zip(Class::ALL) {
            choice(*r, cl.name(), cl == class, hud::class_color(cl), 17.0);
        }
        if !places.is_empty() {
            text("Play on", inner, p.y + 608.0, 18.0, WHITE);
            for (i, r) in places.iter().enumerate() {
                let place = &self.sources[i].place;
                if available[i] {
                    choice(*r, place.label(), i == source, place.color(), 16.0);
                } else {
                    button_ex(*r, &format!("{} (offline)", place.label()), false);
                }
            }
        }

        // Descriptions of the chosen race and class, bottom left.
        let info = Rect::new(30.0, h - 250.0, 440.0, 220.0);
        panel(info);
        let mut y = info.y + 28.0;
        text(race.name(), info.x + 14.0, y, 22.0, race_color);
        for line in hud::wrap(race.description(), 50) {
            y += 19.0;
            text(
                &line,
                info.x + 14.0,
                y,
                16.0,
                Color::new(0.85, 0.85, 0.85, 1.0),
            );
        }
        y += 32.0;
        text(
            class.name(),
            info.x + 14.0,
            y,
            22.0,
            hud::class_color(class),
        );
        for line in hud::wrap(class.description(), 50) {
            y += 19.0;
            text(
                &line,
                info.x + 14.0,
                y,
                16.0,
                Color::new(0.85, 0.85, 0.85, 1.0),
            );
        }
        let values = [
            (
                "Body",
                if appearance.body == 0 {
                    "Broad".to_string()
                } else {
                    "Slender".to_string()
                },
            ),
            ("Skin", format!("{}", appearance.skin + 1)),
            ("Hair", appearance.hair_style_name().to_string()),
            ("Hair color", format!("{}", appearance.hair_color + 1)),
        ];
        for ((label, value), (prev, next)) in values.iter().zip(&option_rows) {
            text(label, inner, prev.y + 23.0, 19.0, WHITE);
            text_centered(
                value,
                (prev.right() + next.x) / 2.0,
                prev.y + 23.0,
                19.0,
                GOLD,
            );
            button(*prev, "<");
            button(*next, ">");
        }
        // Swatches next to the color options.
        draw_rectangle(
            inner + 110.0,
            option_rows[1].0.y + 6.0,
            22.0,
            22.0,
            render::skin_color(appearance.race, appearance.skin),
        );
        draw_rectangle(
            inner + 110.0,
            option_rows[3].0.y + 6.0,
            22.0,
            22.0,
            render::hair_color(appearance.hair_color),
        );
        button(create, "Create");
        button_ex(back, "Back", has_characters);
        CharacterOutcome::Stay
    }
}
