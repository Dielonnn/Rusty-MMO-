//! The screens before the game: login (account and server), character
//! select, and character creation, all over a slowly circling view of town.

use std::net::SocketAddr;
use std::path::PathBuf;

use macroquad::prelude::*;
use shared::data::{Appearance, Class};
use shared::net::Connection;
use shared::protocol::*;

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

/// The menu camera slowly circles the town.
fn draw_backdrop(scene: &Scene, time: f32, batch: &mut Batch, preview: Option<(&Look, f32)>) {
    let a = time * 0.04;
    let (position, target) = match preview {
        // Close up on the character standing in the town square.
        Some(_) => (vec3(-2.2, 1.9, 9.6), vec3(-2.6, 1.2, 4.0)),
        None => (
            vec3(a.cos() * 55.0, 22.0, a.sin() * 55.0),
            vec3(0.0, 2.0, 0.0),
        ),
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
    render::draw_sky(&cam, |p| hud::project(&cam, p));
    set_camera(&cam);
    scene.begin_3d();
    scene.draw();
    if let Some((look, yaw)) = preview {
        let pose = Pose {
            time,
            ..Default::default()
        };
        render::draw_model(batch, look, vec3(-3.4, 0.0, 4.0), yaw, pose);
    }
    scene.draw_effects(batch, time);
    batch.flush();
    scene.draw_water();
    scene.end_3d();
    set_default_camera();
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
    /// The single-player server, once started.
    local: Option<SocketAddr>,
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
            local: None,
            batch: Batch::new(),
        }
    }

    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    pub fn frame(&mut self, scene: &Scene) -> Option<Characters> {
        self.time += get_frame_time();
        draw_backdrop(scene, self.time, &mut self.batch, None);

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
        let solo = Rect::new(inner, p.y + 270.0, iw * 0.5 - 6.0, 44.0);
        let join = Rect::new(inner + iw * 0.5 + 6.0, p.y + 270.0, iw * 0.5 - 6.0, 44.0);
        let quit = Rect::new(inner + iw * 0.25, p.y + 334.0, iw * 0.5, 40.0);

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
            } else if solo.contains(m) {
                start = Some(true);
            } else if join.contains(m) {
                start = Some(false);
            } else if quit.contains(m) {
                std::process::exit(0);
            }
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
            start = Some(self.focus == Field::Account);
        }

        panel(p);
        let cx = p.x + p.w / 2.0;
        text_centered("Rusty MMO", cx, p.y + 54.0, 48.0, GOLD);
        text_centered(
            &format!("{}  -  {}", render::ZONE_NAME, shared::VERSION),
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
        input_box(
            address_box,
            &self.address,
            "host:port",
            self.focus == Field::Address,
            self.time,
        );
        button(solo, "Play Solo");
        button(join, "Join Server");
        button(quit, "Quit");
        text_centered(
            "Solo characters are saved on this computer.",
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

        let solo = start?;
        match self.connect(solo) {
            Ok(screen) => Some(screen),
            Err(e) => {
                self.message = Some(e);
                None
            }
        }
    }

    fn connect(&mut self, solo: bool) -> Result<Characters, String> {
        let account = if self.account.trim().is_empty() {
            "Player".to_string()
        } else {
            self.account.trim().to_string()
        };
        save_settings(&account, &self.address);
        let addr = if solo {
            match self.local {
                Some(a) => a.to_string(),
                None => {
                    let save = data_dir().join("solo_characters.json");
                    let a = server::spawn_local(save)
                        .map_err(|e| format!("Couldn't start the local server: {e}"))?;
                    self.local = Some(a);
                    a.to_string()
                }
            }
        } else {
            self.address.clone()
        };
        let mut conn = Connection::connect(&addr, DEFAULT_PORT)
            .map_err(|e| format!("Couldn't connect to {addr}: {e}"))?;
        conn.send(&ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            account: account.clone(),
        })
        .map_err(|e| format!("Couldn't talk to {addr}: {e}"))?;
        Ok(Characters::new(conn, account, solo))
    }
}

// ---- Character select and creation ----

struct Create {
    name: String,
    class: Class,
    appearance: Appearance,
}

pub struct Characters {
    /// Moves into the game when a character enters the world.
    conn: Option<Connection>,
    account: String,
    solo: bool,
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

impl Characters {
    pub fn new(conn: Connection, account: String, solo: bool) -> Self {
        Self {
            conn: Some(conn),
            account,
            solo,
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

    pub fn account(&self) -> (&str, bool) {
        (&self.account, self.solo)
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
                        self.create.get_or_insert_with(|| Create {
                            name: String::new(),
                            class: Class::Warrior,
                            appearance: Appearance::default(),
                        });
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
                        .map_or(Class::Warrior, |c| c.class);
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
        draw_backdrop(
            scene,
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
        let where_ = if self.solo {
            "Solo".to_string()
        } else {
            format!("Account: {}", self.account)
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
            if let Some(i) = rows.iter().position(|r| r.contains(m)) {
                self.selected = i;
                self.confirm_delete = false;
            } else if enter.contains(m) && !list.is_empty() {
                self.enter(&list);
            } else if create.contains(m) {
                self.create = Some(Create {
                    name: String::new(),
                    class: Class::Warrior,
                    appearance: Appearance::default(),
                });
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
                &format!("Level {} Human {}", c.level, c.class.name()),
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
        let class_w = (iw - 10.0) / 2.0;
        let classes: Vec<Rect> = (0..4)
            .map(|i| {
                Rect::new(
                    inner + (i % 2) as f32 * (class_w + 10.0),
                    p.y + 162.0 + (i / 2) as f32 * 46.0,
                    class_w,
                    40.0,
                )
            })
            .collect();
        let options_y = p.y + 360.0;
        let option_rows: Vec<(Rect, Rect)> = (0..4)
            .map(|i| {
                let y = options_y + i as f32 * 44.0;
                (
                    Rect::new(inner + iw - 170.0, y, 36.0, 34.0),
                    Rect::new(inner + iw - 36.0, y, 36.0, 34.0),
                )
            })
            .collect();
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
            for (r, class) in classes.iter().zip(Class::ALL) {
                if r.contains(m) {
                    c.class = class;
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
        text_centered(
            "Human - Amberfall Vale",
            p.x + p.w / 2.0,
            p.y + 58.0,
            16.0,
            Color::new(0.8, 0.8, 0.8, 1.0),
        );
        text("Name", name_box.x, name_box.y - 8.0, 18.0, WHITE);
        input_box(name_box, &name, "Letters only", true, self.time);
        text("Class", inner, p.y + 154.0, 18.0, WHITE);
        let m = mouse();
        for (r, cl) in classes.iter().zip(Class::ALL) {
            let selected = cl == class;
            let color = hud::class_color(cl);
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
                if selected || r.contains(m) {
                    color
                } else {
                    BORDER
                },
            );
            text_centered(
                cl.name(),
                r.x + r.w / 2.0,
                r.y + 27.0,
                22.0,
                if selected { WHITE } else { color },
            );
        }
        for (i, line) in hud::wrap(class.description(), 44).iter().enumerate() {
            text(
                line,
                inner,
                p.y + 270.0 + i as f32 * 19.0,
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
            render::skin_color(appearance.skin),
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
