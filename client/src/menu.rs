//! The login screen: pick a name and class, then play solo or join a server.

use macroquad::prelude::*;
use shared::data::Class;
use shared::net::Connection;
use shared::protocol::DEFAULT_PORT;

use crate::game::Game;
use crate::hud::{self, BORDER, button, panel, text, text_centered};
use crate::render::Scene;

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Name,
    Address,
}

pub struct Menu {
    name: String,
    class: Class,
    address: String,
    focus: Field,
    message: Option<String>,
    time: f32,
}

struct MenuLayout {
    panel: Rect,
    name: Rect,
    classes: [Rect; 3],
    address: Rect,
    solo: Rect,
    join: Rect,
    quit: Rect,
}

impl MenuLayout {
    fn new() -> Self {
        let w = 480.0;
        let h = 560.0;
        let x = (screen_width() - w) / 2.0;
        let y = ((screen_height() - h) / 2.0).max(10.0);
        let inner = x + 24.0;
        let inner_w = w - 48.0;
        let class_w = (inner_w - 20.0) / 3.0;
        Self {
            panel: Rect::new(x, y, w, h),
            name: Rect::new(inner, y + 112.0, inner_w, 36.0),
            classes: std::array::from_fn(|i| {
                Rect::new(
                    inner + i as f32 * (class_w + 10.0),
                    y + 186.0,
                    class_w,
                    40.0,
                )
            }),
            address: Rect::new(inner, y + 368.0, inner_w, 36.0),
            solo: Rect::new(inner, y + 424.0, inner_w * 0.5 - 6.0, 44.0),
            join: Rect::new(
                inner + inner_w * 0.5 + 6.0,
                y + 424.0,
                inner_w * 0.5 - 6.0,
                44.0,
            ),
            quit: Rect::new(inner + inner_w * 0.25, y + 488.0, inner_w * 0.5, 40.0),
        }
    }
}

impl Menu {
    pub fn new(message: Option<String>) -> Self {
        Self {
            name: String::new(),
            class: Class::Warrior,
            address: "127.0.0.1".into(),
            focus: Field::Name,
            message,
            time: 0.0,
        }
    }

    /// Keeps the name, class and address from last time.
    pub fn returning(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    pub fn frame(&mut self, scene: &Scene) -> Option<Game> {
        self.time += get_frame_time();
        // Slowly circle the town in the background.
        clear_background(Color::new(0.55, 0.75, 0.95, 1.0));
        let a = self.time * 0.05;
        set_camera(&Camera3D {
            position: vec3(a.cos() * 55.0, 22.0, a.sin() * 55.0),
            target: vec3(0.0, 2.0, 0.0),
            up: Vec3::Y,
            fovy: 1.0,
            z_near: 0.1,
            z_far: 1500.0,
            ..Default::default()
        });
        scene.draw();
        scene.draw_water();
        set_default_camera();

        let l = MenuLayout::new();
        let mouse = vec2(mouse_position().0, mouse_position().1);
        let clicked = is_mouse_button_pressed(MouseButton::Left);

        // Typing.
        let field = match self.focus {
            Field::Name => &mut self.name,
            Field::Address => &mut self.address,
        };
        while let Some(c) = get_char_pressed() {
            let ok = match self.focus {
                Field::Name => c.is_alphanumeric() && field.chars().count() < 12,
                Field::Address => !c.is_control() && !c.is_whitespace() && field.len() < 64,
            };
            if ok {
                field.push(c);
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            field.pop();
        }
        if is_key_pressed(KeyCode::Tab) {
            self.focus = if self.focus == Field::Name {
                Field::Address
            } else {
                Field::Name
            };
        }

        let mut start = None;
        if clicked {
            if l.name.contains(mouse) {
                self.focus = Field::Name;
            } else if l.address.contains(mouse) {
                self.focus = Field::Address;
            }
            for (r, class) in l.classes.iter().zip(Class::ALL) {
                if r.contains(mouse) {
                    self.class = class;
                }
            }
            if l.solo.contains(mouse) {
                start = Some(true);
            } else if l.join.contains(mouse) {
                start = Some(false);
            } else if l.quit.contains(mouse) {
                std::process::exit(0);
            }
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
            start = Some(self.focus == Field::Name);
        }

        self.draw(&l);

        let solo = start?;
        match self.connect(solo) {
            Ok(game) => Some(game),
            Err(e) => {
                self.message = Some(e);
                None
            }
        }
    }

    fn connect(&mut self, solo: bool) -> Result<Game, String> {
        let name = if self.name.is_empty() {
            "Adventurer".to_string()
        } else {
            self.name.clone()
        };
        let addr = if solo {
            let addr = server::spawn_local()
                .map_err(|e| format!("Couldn't start the local server: {e}"))?;
            addr.to_string()
        } else {
            self.address.clone()
        };
        let conn = Connection::connect(&addr, DEFAULT_PORT)
            .map_err(|e| format!("Couldn't connect to {addr}: {e}"))?;
        Game::new(conn, &name, self.class).map_err(|e| format!("Couldn't talk to {addr}: {e}"))
    }

    fn draw(&self, l: &MenuLayout) {
        let p = l.panel;
        panel(p);
        let cx = p.x + p.w / 2.0;
        text_centered(
            "Rusty MMO",
            cx,
            p.y + 54.0,
            48.0,
            Color::new(1.0, 0.82, 0.25, 1.0),
        );
        text_centered(
            &format!("A tab-target adventure  -  {}", shared::VERSION),
            cx,
            p.y + 80.0,
            18.0,
            Color::new(0.85, 0.85, 0.85, 1.0),
        );

        text("Character name", l.name.x, l.name.y - 8.0, 18.0, WHITE);
        input_box(
            l.name,
            &self.name,
            "Adventurer",
            self.focus == Field::Name,
            self.time,
        );

        text("Class", l.classes[0].x, l.classes[0].y - 8.0, 18.0, WHITE);
        let mouse = vec2(mouse_position().0, mouse_position().1);
        for (r, class) in l.classes.iter().zip(Class::ALL) {
            let selected = class == self.class;
            let color = hud::class_color(class);
            let bg = if selected {
                Color::new(color.r * 0.45, color.g * 0.45, color.b * 0.45, 1.0)
            } else {
                Color::new(0.15, 0.15, 0.18, 1.0)
            };
            draw_rectangle(r.x, r.y, r.w, r.h, bg);
            let border = if selected || r.contains(mouse) {
                color
            } else {
                BORDER
            };
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, border);
            text_centered(
                class.name(),
                r.x + r.w / 2.0,
                r.y + 27.0,
                22.0,
                if selected { WHITE } else { color },
            );
        }
        for (i, line) in hud::wrap(self.class.description(), 52).iter().enumerate() {
            text(
                line,
                l.classes[0].x,
                l.classes[0].y + 70.0 + i as f32 * 20.0,
                17.0,
                Color::new(0.85, 0.85, 0.85, 1.0),
            );
        }

        text(
            "Server address (for Join)",
            l.address.x,
            l.address.y - 8.0,
            18.0,
            WHITE,
        );
        input_box(
            l.address,
            &self.address,
            "host:port",
            self.focus == Field::Address,
            self.time,
        );
        button(l.solo, "Play Solo");
        button(l.join, "Join Server");
        button(l.quit, "Quit");

        if let Some(msg) = &self.message {
            let lines = hud::wrap(msg, 60);
            let y = p.bottom() + 28.0;
            for (i, line) in lines.iter().enumerate() {
                text_centered(
                    line,
                    cx,
                    y + i as f32 * 22.0,
                    20.0,
                    Color::new(1.0, 0.45, 0.4, 1.0),
                );
            }
        }
    }
}

fn input_box(r: Rect, value: &str, placeholder: &str, focused: bool, time: f32) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle_lines(
        r.x,
        r.y,
        r.w,
        r.h,
        2.0,
        if focused {
            Color::new(1.0, 0.82, 0.25, 1.0)
        } else {
            BORDER
        },
    );
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
