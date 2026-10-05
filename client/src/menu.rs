//! The screens before the game: login (account and server), character
//! select, and character creation, all over a view of a starting town.

use std::net::SocketAddr;

use macroquad::prelude::*;
use shared::data::{Appearance, Class, Race};
use shared::net::Connection;
use shared::protocol::*;
use shared::world::Zone;

use crate::audio::{self, Sfx};
use crate::game::Game;
use crate::hud::{self, BORDER, GOLD, button, button_ex, panel, text, text_centered};
use crate::render::{self, Batch, Look, Pose, Scene};
use crate::settings::SettingsWindow;

pub use shared::data_dir;

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
pub enum Mode {
    /// A private world on this computer.
    Solo,
    /// A private world with cheats, and its own characters.
    Sandbox,
    /// Someone's server.
    Online,
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Account,
    Password,
    Address,
}

pub struct Login {
    account: String,
    /// Only for Join; never saved.
    password: String,
    address: String,
    focus: Field,
    message: Option<String>,
    time: f32,
    /// The single-player servers, once started: solo and sandbox.
    local: [Option<SocketAddr>; 2],
    batch: Batch,
    settings: Option<SettingsWindow>,
}

impl Login {
    pub fn new() -> Self {
        let (account, address) = load_settings();
        Self {
            account,
            password: String::new(),
            address,
            focus: Field::Account,
            message: None,
            time: 0.0,
            local: [None; 2],
            batch: Batch::new(),
            settings: None,
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
            ((h - 536.0) / 2.0).max(10.0),
            440.0,
            536.0,
        );
        let inner = p.x + 24.0;
        let iw = p.w - 48.0;
        let account_box = Rect::new(inner, p.y + 132.0, iw, 36.0);
        let password_box = Rect::new(inner, p.y + 208.0, iw, 36.0);
        let address_box = Rect::new(inner, p.y + 284.0, iw, 36.0);
        let solo = Rect::new(inner, p.y + 342.0, iw * 0.5 - 6.0, 44.0);
        let join = Rect::new(inner + iw * 0.5 + 6.0, p.y + 342.0, iw * 0.5 - 6.0, 44.0);
        let sandbox = Rect::new(inner, p.y + 396.0, iw * 0.5 - 6.0, 40.0);
        let quit = Rect::new(inner + iw * 0.5 + 6.0, p.y + 396.0, iw * 0.5 - 6.0, 40.0);
        let settings = Rect::new(inner, p.y + 446.0, iw, 40.0);

        // The Settings window takes all the input while it's open. Opened
        // this frame, it starts taking clicks next frame, so the click that
        // opened it doesn't land on it too.
        let settings_open = self.settings.is_some();
        let mut start = None;
        if !settings_open {
            match self.focus {
                Field::Account => type_into(&mut self.account, 24, |c| !c.is_control()),
                Field::Password => type_into(&mut self.password, 64, |c| !c.is_control()),
                Field::Address => type_into(&mut self.address, 64, |c| {
                    !c.is_control() && !c.is_whitespace()
                }),
            }
            if is_key_pressed(KeyCode::Tab) {
                self.focus = match self.focus {
                    Field::Account => Field::Password,
                    Field::Password => Field::Address,
                    Field::Address => Field::Account,
                };
            }
            if is_mouse_button_pressed(MouseButton::Left) {
                let m = mouse();
                if [solo, join, sandbox, quit, settings]
                    .iter()
                    .any(|r| r.contains(m))
                {
                    audio::play(Sfx::Click);
                }
                if account_box.contains(m) {
                    self.focus = Field::Account;
                } else if password_box.contains(m) {
                    self.focus = Field::Password;
                } else if address_box.contains(m) {
                    self.focus = Field::Address;
                } else if solo.contains(m) {
                    start = Some(Mode::Solo);
                } else if join.contains(m) {
                    start = Some(Mode::Online);
                } else if sandbox.contains(m) {
                    start = Some(Mode::Sandbox);
                } else if quit.contains(m) {
                    std::process::exit(0);
                } else if settings.contains(m) {
                    self.settings = Some(SettingsWindow::open());
                }
            }
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                start = Some(if self.focus == Field::Account {
                    Mode::Solo
                } else {
                    Mode::Online
                });
            }
        }

        panel(p);
        let cx = p.x + p.w / 2.0;
        text_centered("Rusty MMO", cx, p.y + 54.0, 48.0, GOLD);
        text_centered(
            &format!("Six starting areas  -  {}", shared::version()),
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
            "Server address (for Join)",
            address_box.x,
            address_box.y - 8.0,
            18.0,
            WHITE,
        );
        text(
            "Password (for Join)",
            password_box.x,
            password_box.y - 8.0,
            18.0,
            WHITE,
        );
        input_box(
            password_box,
            &"*".repeat(self.password.chars().count()),
            "New accounts pick one here",
            self.focus == Field::Password,
            self.time,
        );
        input_box(
            address_box,
            &self.address,
            "host:port",
            self.focus == Field::Address,
            self.time,
        );
        button(solo, "Play Solo");
        button(join, "Join Server");
        button(sandbox, "Sandbox");
        button(quit, "Quit");
        button(settings, "Settings");
        text_centered(
            "Solo and sandbox characters are saved on this computer.",
            cx,
            p.bottom() - 18.0,
            16.0,
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

        if settings_open && self.settings.as_mut().is_some_and(|s| !s.frame()) {
            self.settings = None;
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

    fn connect(&mut self, mode: Mode) -> Result<Characters, String> {
        let account = if self.account.trim().is_empty() {
            "Player".to_string()
        } else {
            self.account.trim().to_string()
        };
        save_settings(&account, &self.address);
        let addr = if mode == Mode::Online {
            self.address.clone()
        } else {
            let (slot, file, sandbox) = match mode {
                Mode::Sandbox => (1, "sandbox_characters.json", true),
                _ => (0, "solo_characters.json", false),
            };
            match self.local[slot] {
                Some(a) => a.to_string(),
                None => {
                    let save = data_dir().join(file);
                    let a = server::spawn_local(save, sandbox)
                        .map_err(|e| format!("Couldn't start the local server: {e}"))?;
                    self.local[slot] = Some(a);
                    a.to_string()
                }
            }
        };
        let mut conn = Connection::connect(&addr, DEFAULT_PORT)
            .map_err(|e| format!("Couldn't connect to {addr}: {e}"))?;
        conn.send(&ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            account: account.clone(),
            password: if mode == Mode::Online {
                self.password.clone()
            } else {
                String::new()
            },
        })
        .map_err(|e| format!("Couldn't talk to {addr}: {e}"))?;
        Ok(Characters::new(conn, account, mode))
    }
}

// ---- Character select and creation ----

struct Create {
    name: String,
    class: Class,
    appearance: Appearance,
    /// The slider being dragged (0: height, 1: weight).
    dragging: Option<usize>,
}

pub struct Characters {
    /// Moves into the game when a character enters the world.
    conn: Option<Connection>,
    account: String,
    mode: Mode,
    list: Option<Vec<CharacterSummary>>,
    selected: usize,
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

/// A labeled slider from 0 to `Appearance::SLIDER_MAX`; `r` is the track
/// area, with the label drawn to its left and the end names under it.
fn slider(r: Rect, label: &str, value: u8, ends: [&str; 2], active: bool) {
    let hover = active || r.contains(mouse());
    text(label, r.x - 110.0, r.y + 21.0, 19.0, WHITE);
    let track_y = r.y + 12.0;
    draw_rectangle(r.x, track_y, r.w, 6.0, Color::new(0.15, 0.11, 0.06, 0.95));
    let t = value as f32 / Appearance::SLIDER_MAX as f32;
    draw_rectangle(
        r.x,
        track_y,
        r.w * t,
        6.0,
        Color::new(0.6, 0.45, 0.18, 0.95),
    );
    draw_rectangle_lines(r.x, track_y, r.w, 6.0, 1.0, BORDER);
    // The middle, where every character starts.
    draw_line(
        r.x + r.w / 2.0,
        track_y - 3.0,
        r.x + r.w / 2.0,
        track_y + 9.0,
        1.0,
        BORDER,
    );
    let knob = vec2(r.x + r.w * t, track_y + 3.0);
    draw_circle(knob.x, knob.y, 8.0, if hover { GOLD } else { WHITE });
    draw_circle_lines(knob.x, knob.y, 8.0, 1.5, BORDER);
    let grey = Color::new(0.7, 0.7, 0.7, 1.0);
    text(ends[0], r.x, r.y + 32.0, 14.0, grey);
    let right = measure_text(ends[1], None, 14, 1.0).width;
    text(ends[1], r.right() - right, r.y + 32.0, 14.0, grey);
}

fn new_character() -> Create {
    Create {
        name: String::new(),
        class: Class::Barbarian,
        appearance: Appearance::default(),
        dragging: None,
    }
}

impl Characters {
    pub fn new(conn: Connection, account: String, mode: Mode) -> Self {
        Self {
            conn: Some(conn),
            account,
            mode,
            list: None,
            selected: 0,
            create: None,
            confirm_delete: false,
            message: None,
            time: 0.0,
            spin: 0.0,
            batch: Batch::new(),
        }
    }

    pub fn account(&self) -> (&str, Mode) {
        (&self.account, self.mode)
    }

    fn send(&mut self, msg: ClientMsg) {
        if let Some(conn) = &mut self.conn {
            let _ = conn.send(&msg);
        }
    }

    pub fn frame(&mut self, scene: &Scene) -> CharacterOutcome {
        let dt = get_frame_time();
        self.time += dt;
        self.spin += dt * 0.5;
        // Network.
        loop {
            let Some(conn) = self.conn.as_mut() else {
                return CharacterOutcome::Back(None);
            };
            match conn.poll::<ServerMsg>() {
                Ok(Some(ServerMsg::Characters(list))) => {
                    // A new character was just made: select it.
                    if let (Some(old), Some(c)) = (&self.list, &self.create)
                        && list.len() > old.len()
                    {
                        self.selected = list
                            .iter()
                            .position(|s| s.name.eq_ignore_ascii_case(&c.name))
                            .unwrap_or(0);
                    }
                    if self.selected >= list.len() {
                        self.selected = 0;
                    }
                    if list.is_empty() {
                        self.create.get_or_insert_with(new_character);
                    } else if self.list.as_ref().is_some_and(|old| list.len() > old.len()) {
                        self.create = None;
                    }
                    self.list = Some(list);
                }
                Ok(Some(ServerMsg::CharacterError(e))) => self.message = Some(e),
                Ok(Some(ServerMsg::Rejected(e))) => return CharacterOutcome::Back(Some(e)),
                Ok(Some(ServerMsg::Welcome { id })) => {
                    let class = self
                        .list
                        .as_ref()
                        .and_then(|l| l.get(self.selected))
                        .map_or(Class::Barbarian, |c| c.class);
                    let conn = self.conn.take().unwrap();
                    let mut game = Game::new(conn, class);
                    game.my_id = Some(id);
                    return CharacterOutcome::Play(Box::new(game));
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => {
                    return CharacterOutcome::Back(Some(format!(
                        "Lost connection to the server ({e})."
                    )));
                }
            }
        }

        let preview = match &self.create {
            Some(c) => Some(Look {
                kind: EntityKind::Player(c.class),
                appearance: c.appearance,
                gear: [None; 5],
                seed: 0,
            }),
            None => self
                .list
                .as_ref()
                .and_then(|l| l.get(self.selected))
                .map(|c| Look {
                    kind: EntityKind::Player(c.class),
                    appearance: c.appearance,
                    gear: c.gear,
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

        if self.list.is_none() {
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
            self.create_screen()
        } else {
            self.select_screen()
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

    fn select_screen(&mut self) -> CharacterOutcome {
        let list = self.list.clone().unwrap_or_default();
        let (w, h) = (screen_width(), screen_height());
        let p = Rect::new(w - 380.0, 30.0, 350.0, h - 60.0);
        panel(p);
        text_centered("Characters", p.x + p.w / 2.0, p.y + 36.0, 30.0, GOLD);
        let where_ = match self.mode {
            Mode::Solo => "Solo".to_string(),
            Mode::Sandbox => "Sandbox: press P in game for cheats".to_string(),
            Mode::Online => format!("Account: {}", self.account),
        };
        text_centered(
            &where_,
            p.x + p.w / 2.0,
            p.y + 60.0,
            16.0,
            Color::new(0.75, 0.75, 0.75, 1.0),
        );
        let rows: Vec<Rect> = (0..list.len())
            .map(|i| Rect::new(p.x + 14.0, p.y + 78.0 + i as f32 * 60.0, p.w - 28.0, 54.0))
            .collect();
        let enter = Rect::new(w / 2.0 - 120.0, h - 110.0, 240.0, 48.0);
        let create = Rect::new(p.x + 14.0, p.bottom() - 112.0, p.w - 28.0, 40.0);
        let delete = Rect::new(p.x + 14.0, p.bottom() - 62.0, (p.w - 40.0) / 2.0, 40.0);
        let back = Rect::new(
            p.x + 26.0 + (p.w - 40.0) / 2.0,
            p.bottom() - 62.0,
            (p.w - 40.0) / 2.0,
            40.0,
        );

        if is_mouse_button_pressed(MouseButton::Left) {
            let m = mouse();
            if rows
                .iter()
                .chain([&enter, &create, &delete, &back])
                .any(|r| r.contains(m))
            {
                audio::play(Sfx::Click);
            }
            if let Some(i) = rows.iter().position(|r| r.contains(m)) {
                self.selected = i;
                self.confirm_delete = false;
            } else if enter.contains(m) && !list.is_empty() {
                self.enter(&list);
            } else if create.contains(m) {
                self.create = Some(new_character());
                self.message = None;
            } else if delete.contains(m) && !list.is_empty() {
                if self.confirm_delete {
                    let name = list[self.selected].name.clone();
                    self.send(ClientMsg::DeleteCharacter(name));
                    self.confirm_delete = false;
                } else {
                    self.confirm_delete = true;
                }
            } else if back.contains(m) {
                return CharacterOutcome::Back(None);
            }
        }
        if !list.is_empty() {
            if is_key_pressed(KeyCode::Down) {
                self.selected = (self.selected + 1) % list.len();
            }
            if is_key_pressed(KeyCode::Up) {
                self.selected = (self.selected + list.len() - 1) % list.len();
            }
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                self.enter(&list);
            }
        }
        if is_key_pressed(KeyCode::Escape) {
            return CharacterOutcome::Back(None);
        }
        while get_char_pressed().is_some() {}

        for (i, (c, r)) in list.iter().zip(&rows).enumerate() {
            let selected = i == self.selected;
            draw_rectangle(
                r.x,
                r.y,
                r.w,
                r.h,
                if selected {
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
                if selected { GOLD } else { BORDER },
            );
            text(
                &c.name,
                r.x + 12.0,
                r.y + 24.0,
                24.0,
                hud::class_color(c.class),
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
        if list.is_empty() {
            text_centered(
                "No characters yet.",
                p.x + p.w / 2.0,
                p.y + 110.0,
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
            !list.is_empty(),
        );
        button(back, "Back");
        if let Some(c) = list.get(self.selected) {
            text_centered(
                &c.name,
                w / 2.0 - 60.0,
                h - 150.0,
                40.0,
                hud::class_color(c.class),
            );
        }
        button_ex(enter, "Enter World", !list.is_empty());
        CharacterOutcome::Stay
    }

    fn enter(&mut self, list: &[CharacterSummary]) {
        if let Some(c) = list.get(self.selected) {
            self.message = None;
            let name = c.name.clone();
            self.send(ClientMsg::EnterWorld(name));
        }
    }

    fn create_screen(&mut self) -> CharacterOutcome {
        let (w, h) = (screen_width(), screen_height());
        let p = Rect::new(w - 420.0, 30.0, 390.0, h - 60.0);
        let inner = p.x + 20.0;
        let iw = p.w - 40.0;
        let name_box = Rect::new(inner, p.y + 92.0, iw, 36.0);
        // A grid of buttons, `across` to a row.
        let grid = |count: usize, across: usize, top: f32, height: f32| -> Vec<Rect> {
            let cw = (iw - 6.0 * (across - 1) as f32) / across as f32;
            (0..count)
                .map(|i| {
                    Rect::new(
                        inner + (i % across) as f32 * (cw + 6.0),
                        top + (i / across) as f32 * (height + 4.0),
                        cw,
                        height,
                    )
                })
                .collect()
        };
        let races = grid(Race::ALL.len(), 4, p.y + 160.0, 30.0);
        let classes = grid(Class::ALL.len(), 3, p.y + 252.0, 28.0);
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
        // Height and weight sliders, below the options.
        let sliders: [Rect; 2] = std::array::from_fn(|i| {
            let y = options_y + (4 + i) as f32 * 38.0;
            Rect::new(inner + 110.0, y, iw - 110.0, 32.0)
        });
        let create = Rect::new(inner, p.bottom() - 62.0, iw * 0.5 - 6.0, 44.0);
        let back = Rect::new(
            inner + iw * 0.5 + 6.0,
            p.bottom() - 62.0,
            iw * 0.5 - 6.0,
            44.0,
        );
        let has_characters = self.list.as_ref().is_some_and(|l| !l.is_empty());

        let c = self.create.as_mut().unwrap();
        type_into(&mut c.name, 12, |ch| ch.is_ascii_alphabetic());
        let mut submit = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if is_mouse_button_pressed(MouseButton::Left) {
            let m = mouse();
            if [create, back].iter().any(|r| r.contains(m)) {
                audio::play(Sfx::Click);
            }
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
            c.dragging = sliders.iter().position(|r| r.contains(m));
            if create.contains(m) {
                submit = true;
            } else if back.contains(m) && has_characters {
                self.create = None;
                self.message = None;
                return CharacterOutcome::Stay;
            }
        }
        if !is_mouse_button_down(MouseButton::Left) {
            c.dragging = None;
        }
        if let Some(i) = c.dragging {
            let r = sliders[i];
            let t = ((mouse().x - r.x) / r.w).clamp(0.0, 1.0);
            let value = (t * Appearance::SLIDER_MAX as f32).round() as u8;
            if i == 0 {
                c.appearance.height = value;
            } else {
                c.appearance.weight = value;
            }
        }
        if is_key_pressed(KeyCode::Escape) && has_characters {
            self.create = None;
            self.message = None;
            return CharacterOutcome::Stay;
        }
        let (name, class, appearance) = (c.name.clone(), c.class, c.appearance);
        if submit {
            if name.len() < 2 {
                self.message = Some("Pick a name of 2 to 12 letters.".into());
            } else {
                self.message = None;
                self.send(ClientMsg::CreateCharacter {
                    name: name.clone(),
                    class,
                    appearance,
                });
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
                    "Male".to_string()
                } else {
                    "Female".to_string()
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
        let slider_values = [
            ("Height", appearance.height, ["Short", "Tall"]),
            ("Weight", appearance.weight, ["Thin", "Heavy"]),
        ];
        for (i, (r, (label, value, ends))) in sliders.iter().zip(slider_values).enumerate() {
            let active = self.create.as_ref().is_some_and(|c| c.dragging == Some(i));
            slider(*r, label, value, ends, active);
        }
        button(create, "Create");
        button_ex(back, "Back", has_characters);
        CharacterOutcome::Stay
    }
}
