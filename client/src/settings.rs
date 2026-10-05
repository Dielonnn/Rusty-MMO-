//! The Settings window, opened from the login screen or the Game Menu, and
//! the settings it saves to `client_settings.txt` in the data folder.
//!
//! Only the Audio tab works so far: its volumes are saved and ready for
//! when the game has sounds. The other tabs are listed but greyed out.

use std::cell::RefCell;

use macroquad::prelude::*;

use crate::hud::{BORDER, GOLD, PANEL, button, button_ex, text, text_centered, window};
use shared::data_dir;

const FILE: &str = "client_settings.txt";

/// The volume sliders, in the order they're shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bus {
    Master,
    Music,
    Effects,
    Ambience,
    Interface,
}

impl Bus {
    pub const ALL: [Bus; 5] = [
        Bus::Master,
        Bus::Music,
        Bus::Effects,
        Bus::Ambience,
        Bus::Interface,
    ];

    fn name(self) -> &'static str {
        match self {
            Bus::Master => "Master",
            Bus::Music => "Music",
            Bus::Effects => "Effects",
            Bus::Ambience => "Ambience",
            Bus::Interface => "Interface",
        }
    }

    /// The key it's saved under.
    fn key(self) -> &'static str {
        match self {
            Bus::Master => "master_volume",
            Bus::Music => "music_volume",
            Bus::Effects => "effects_volume",
            Bus::Ambience => "ambience_volume",
            Bus::Interface => "interface_volume",
        }
    }
}

/// Everything the Settings window remembers between runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Each bus's volume, 0 to 100, in `Bus::ALL` order.
    pub volumes: [u8; 5],
    pub mute_all: bool,
    pub mute_in_background: bool,
}

impl Default for Settings {
    fn default() -> Self {
        // Placeholder defaults until Dielon picks them.
        Self {
            volumes: [80, 50, 80, 60, 70],
            mute_all: false,
            mute_in_background: false,
        }
    }
}

impl Settings {
    /// Reads `key=value` lines. Unknown keys and bad values are skipped, so a
    /// file from an older or newer version still loads.
    fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            if let Some(i) = Bus::ALL.iter().position(|b| b.key() == key) {
                if let Ok(v) = value.parse::<u8>() {
                    s.volumes[i] = v.min(100);
                }
            } else if let Ok(b) = value.parse::<bool>() {
                match key {
                    "mute_all" => s.mute_all = b,
                    "mute_in_background" => s.mute_in_background = b,
                    _ => {}
                }
            }
        }
        s
    }

    fn to_text(&self) -> String {
        let mut out = String::new();
        for (bus, v) in Bus::ALL.iter().zip(self.volumes) {
            out += &format!("{}={v}\n", bus.key());
        }
        out += &format!("mute_all={}\n", self.mute_all);
        out += &format!("mute_in_background={}\n", self.mute_in_background);
        out
    }

    fn load() -> Self {
        std::fs::read_to_string(data_dir().join(FILE))
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }

    fn save(&self) {
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(FILE), self.to_text());
    }
}

thread_local! {
    /// The settings in use, read from disk the first time they're needed.
    static CURRENT: RefCell<Option<Settings>> = const { RefCell::new(None) };
}

/// The settings in use.
pub fn current() -> Settings {
    CURRENT.with(|c| c.borrow_mut().get_or_insert_with(Settings::load).clone())
}

fn set(s: Settings) {
    s.save();
    CURRENT.with(|c| *c.borrow_mut() = Some(s));
}

const TABS: [&str; 5] = ["Audio", "Graphics", "Controls", "Interface", "Gameplay"];

/// Where everything in the window goes this frame.
struct Layout {
    window: Rect,
    tabs: [Rect; 5],
    sliders: [Rect; 5],
    mute_all: Rect,
    mute_background: Rect,
    reset: Rect,
    done: Rect,
}

impl Layout {
    fn new() -> Self {
        let (w, h) = (520.0, 470.0);
        let r = Rect::new(
            (screen_width() - w) / 2.0,
            ((screen_height() - h) / 2.0).max(10.0),
            w,
            h,
        );
        let tab_w = (w - 24.0 - 4.0 * 6.0) / 5.0;
        Self {
            window: r,
            tabs: std::array::from_fn(|i| {
                Rect::new(
                    r.x + 12.0 + i as f32 * (tab_w + 6.0),
                    r.y + 38.0,
                    tab_w,
                    30.0,
                )
            }),
            sliders: std::array::from_fn(|i| {
                Rect::new(r.x + 150.0, r.y + 96.0 + i as f32 * 46.0, 270.0, 24.0)
            }),
            mute_all: Rect::new(r.x + 40.0, r.y + 334.0, 22.0, 22.0),
            mute_background: Rect::new(r.x + 270.0, r.y + 334.0, 22.0, 22.0),
            reset: Rect::new(r.x + 24.0, r.bottom() - 60.0, 200.0, 40.0),
            done: Rect::new(r.right() - 164.0, r.bottom() - 60.0, 140.0, 40.0),
        }
    }
}

/// The open Settings window. Drawn over everything else; while it's open the
/// screen behind it takes no clicks or keys.
pub struct SettingsWindow {
    settings: Settings,
    /// The volume slider being dragged.
    dragging: Option<usize>,
}

impl SettingsWindow {
    pub fn open() -> Self {
        Self {
            settings: current(),
            dragging: None,
        }
    }

    /// Handles input and draws the window. False once it's closed (Done or
    /// Esc). Every change is saved straight away.
    pub fn frame(&mut self) -> bool {
        let l = Layout::new();
        let m = vec2(mouse_position().0, mouse_position().1);
        let before = self.settings.clone();
        let mut open = true;

        if is_mouse_button_pressed(MouseButton::Left) {
            self.dragging = l.sliders.iter().position(|r| r.contains(m));
            if l.mute_all.contains(m) {
                self.settings.mute_all = !self.settings.mute_all;
            } else if l.mute_background.contains(m) {
                self.settings.mute_in_background = !self.settings.mute_in_background;
            } else if l.reset.contains(m) {
                self.settings = Settings::default();
            } else if l.done.contains(m) {
                open = false;
            }
        }
        if !is_mouse_button_down(MouseButton::Left) {
            self.dragging = None;
        }
        if let Some(i) = self.dragging {
            let r = l.sliders[i];
            let t = ((m.x - r.x) / r.w).clamp(0.0, 1.0);
            self.settings.volumes[i] = (t * 100.0).round() as u8;
        }
        if is_key_pressed(KeyCode::Escape) {
            open = false;
        }
        if self.settings != before {
            set(self.settings.clone());
        }

        self.draw(&l);
        open
    }

    fn draw(&self, l: &Layout) {
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        let r = l.window;
        window(r, "Settings");
        for (i, (tab, name)) in l.tabs.iter().zip(TABS).enumerate() {
            if i == 0 {
                draw_rectangle(
                    tab.x,
                    tab.y,
                    tab.w,
                    tab.h,
                    Color::new(0.38, 0.27, 0.11, 0.95),
                );
                draw_rectangle_lines(tab.x, tab.y, tab.w, tab.h, 2.0, GOLD);
                text_centered(name, tab.x + tab.w / 2.0, tab.y + 21.0, 18.0, WHITE);
            } else {
                button_ex(*tab, name, false);
            }
        }

        let grey = Color::new(0.7, 0.7, 0.7, 1.0);
        let muted = self.settings.mute_all;
        for (i, (s, bus)) in l.sliders.iter().zip(Bus::ALL).enumerate() {
            let v = self.settings.volumes[i];
            let hover = self.dragging == Some(i)
                || s.contains(vec2(mouse_position().0, mouse_position().1));
            text(
                bus.name(),
                r.x + 30.0,
                s.y + 18.0,
                20.0,
                if muted { grey } else { WHITE },
            );
            let track_y = s.y + 9.0;
            draw_rectangle(s.x, track_y, s.w, 6.0, Color::new(0.15, 0.11, 0.06, 0.95));
            let t = v as f32 / 100.0;
            let fill = if muted {
                Color::new(0.35, 0.33, 0.3, 0.95)
            } else {
                Color::new(0.6, 0.45, 0.18, 0.95)
            };
            draw_rectangle(s.x, track_y, s.w * t, 6.0, fill);
            draw_rectangle_lines(s.x, track_y, s.w, 6.0, 1.0, BORDER);
            let knob = vec2(s.x + s.w * t, track_y + 3.0);
            draw_circle(knob.x, knob.y, 8.0, if hover { GOLD } else { WHITE });
            draw_circle_lines(knob.x, knob.y, 8.0, 1.5, BORDER);
            text(&format!("{v}%"), s.right() + 18.0, s.y + 18.0, 20.0, WHITE);
        }

        checkbox(l.mute_all, "Mute all", self.settings.mute_all);
        checkbox(
            l.mute_background,
            "Mute in background",
            self.settings.mute_in_background,
        );
        text_centered(
            "The game has no sounds yet. These volumes are used once it does.",
            r.x + r.w / 2.0,
            r.y + 392.0,
            16.0,
            grey,
        );
        button(l.reset, "Reset to Defaults");
        button(l.done, "Done");
    }
}

fn checkbox(r: Rect, label: &str, on: bool) {
    draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, BORDER);
    if on {
        draw_rectangle(r.x + 5.0, r.y + 5.0, r.w - 10.0, r.h - 10.0, GOLD);
    }
    text(label, r.right() + 10.0, r.y + 17.0, 20.0, WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_settings_load_back() {
        let s = Settings {
            volumes: [10, 0, 100, 55, 3],
            mute_all: true,
            mute_in_background: true,
        };
        assert_eq!(Settings::parse(&s.to_text()), s);
    }

    #[test]
    fn missing_or_bad_lines_keep_defaults() {
        let s =
            Settings::parse("music_volume=20\nmaster_volume=abc\nfuture_thing=1\nmute_all=maybe\n");
        assert_eq!(s.volumes, [80, 20, 80, 60, 70]);
        assert!(!s.mute_all);
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn volumes_cap_at_100() {
        assert_eq!(Settings::parse("effects_volume=250").volumes[2], 100);
    }
}
