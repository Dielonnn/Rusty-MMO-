//! The Settings window, opened from the login screen or the Game Menu, and
//! the settings it saves to `client_settings.txt` in the data folder.
//!
//! Five tabs: Audio (volumes), Graphics (window, anti-aliasing, frame
//! cap), Controls (key bindings and mouse), Interface (names, damage
//! numbers, chat) and Gameplay (turning down invites).

use std::cell::RefCell;

use macroquad::prelude::*;

use crate::audio::{self, Sfx};
use crate::hud::{BORDER, GOLD, PANEL, button, text, text_centered, window};
use crate::keys::{self, ACTIONS};
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

/// Window sizes you can pick when not in fullscreen.
pub const WINDOW_SIZES: [(u32, u32); 5] = [
    (1280, 720),
    (1440, 900),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
];
/// Anti-aliasing choices: samples per pixel.
const MSAA: [u8; 4] = [1, 2, 4, 8];
/// Frame cap choices; 0 is no cap.
const FRAME_CAPS: [u16; 5] = [0, 30, 60, 120, 144];

/// Everything the Settings window remembers between runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    // Audio
    /// Each bus's volume, 0 to 100, in `Bus::ALL` order.
    pub volumes: [u8; 5],
    pub mute_all: bool,
    pub mute_in_background: bool,
    // Graphics
    pub fullscreen: bool,
    /// Index into `WINDOW_SIZES`.
    pub window_size: usize,
    /// Samples per pixel (1 is off). Takes effect on restart.
    pub msaa: u8,
    /// Takes effect on restart.
    pub vsync: bool,
    /// Most frames a second; 0 for no cap.
    pub frame_cap: u16,
    pub show_fps: bool,
    // Controls
    /// The key for each action, in `keys::ACTIONS` order.
    pub keys: [KeyCode; ACTIONS.len()],
    /// Percent of normal.
    pub mouse_sensitivity: u8,
    pub invert_mouse: bool,
    /// Percent of normal.
    pub zoom_speed: u8,
    // Interface
    pub names_players: bool,
    pub names_mobs: bool,
    pub names_npcs: bool,
    pub damage_numbers: bool,
    pub chat_size: u8,
    /// Show the controls help when you enter the world.
    pub show_help: bool,
    // Gameplay
    pub decline_invites: bool,
    pub decline_duels: bool,
}

impl Default for Settings {
    fn default() -> Self {
        // Volumes are placeholders until Dielon picks them.
        Self {
            volumes: [80, 50, 80, 60, 70],
            mute_all: false,
            mute_in_background: false,
            fullscreen: false,
            window_size: 1,
            msaa: 4,
            vsync: true,
            frame_cap: 0,
            show_fps: false,
            keys: ACTIONS.map(|a| a.default_key()),
            mouse_sensitivity: 100,
            invert_mouse: false,
            zoom_speed: 100,
            names_players: true,
            names_mobs: true,
            names_npcs: true,
            damage_numbers: true,
            chat_size: 17,
            show_help: true,
            decline_invites: false,
            decline_duels: false,
        }
    }
}

impl Settings {
    /// A bus's volume, 0 to 1.
    pub fn volume(&self, bus: Bus) -> f32 {
        let i = Bus::ALL.iter().position(|b| *b == bus).unwrap_or(0);
        self.volumes[i] as f32 / 100.0
    }

    /// Every on/off setting with the key it's saved under.
    fn flags(&mut self) -> [(&'static str, &mut bool); 13] {
        [
            ("mute_all", &mut self.mute_all),
            ("mute_in_background", &mut self.mute_in_background),
            ("fullscreen", &mut self.fullscreen),
            ("vsync", &mut self.vsync),
            ("show_fps", &mut self.show_fps),
            ("invert_mouse", &mut self.invert_mouse),
            ("names_players", &mut self.names_players),
            ("names_mobs", &mut self.names_mobs),
            ("names_npcs", &mut self.names_npcs),
            ("damage_numbers", &mut self.damage_numbers),
            ("show_help", &mut self.show_help),
            ("decline_invites", &mut self.decline_invites),
            ("decline_duels", &mut self.decline_duels),
        ]
    }

    /// Reads `key=value` lines. Unknown keys and bad values are skipped, so a
    /// file from an older or newer version still loads.
    fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            let number = value.parse::<u32>().ok();
            if let Some(i) = Bus::ALL.iter().position(|b| b.key() == key) {
                if let Some(v) = number {
                    s.volumes[i] = v.min(100) as u8;
                }
            } else if let Some(i) = ACTIONS.iter().position(|a| a.save_key() == key) {
                if let Some(k) = keys::from_save_name(value) {
                    s.keys[i] = k;
                }
            } else if let Ok(b) = value.parse::<bool>() {
                if let Some((_, flag)) = s.flags().into_iter().find(|(k, _)| *k == key) {
                    *flag = b;
                }
            } else if let Some(v) = number {
                match key {
                    "window_size" => s.window_size = (v as usize).min(WINDOW_SIZES.len() - 1),
                    "msaa" if MSAA.contains(&(v as u8)) => s.msaa = v as u8,
                    "frame_cap" if FRAME_CAPS.contains(&(v as u16)) => s.frame_cap = v as u16,
                    "mouse_sensitivity" => s.mouse_sensitivity = v.clamp(25, 250) as u8,
                    "zoom_speed" => s.zoom_speed = v.clamp(25, 250) as u8,
                    "chat_size" => s.chat_size = v.clamp(12, 26) as u8,
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
        let mut copy = self.clone();
        for (key, flag) in copy.flags() {
            out += &format!("{key}={flag}\n");
        }
        out += &format!("window_size={}\n", self.window_size);
        out += &format!("msaa={}\n", self.msaa);
        out += &format!("frame_cap={}\n", self.frame_cap);
        out += &format!("mouse_sensitivity={}\n", self.mouse_sensitivity);
        out += &format!("zoom_speed={}\n", self.zoom_speed);
        out += &format!("chat_size={}\n", self.chat_size);
        for (a, k) in ACTIONS.iter().zip(self.keys) {
            out += &format!("{}={}\n", a.save_key(), keys::save_name(k));
        }
        out
    }

    /// Reads the settings file (or the defaults). Doesn't need the game
    /// window, so it works before the window opens.
    pub fn load() -> Self {
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

/// Looks at the settings in use without copying them.
pub fn with<R>(f: impl FnOnce(&Settings) -> R) -> R {
    CURRENT.with(|c| f(c.borrow_mut().get_or_insert_with(Settings::load)))
}

/// A copy of the settings in use.
pub fn current() -> Settings {
    with(Settings::clone)
}

fn set(s: Settings) {
    s.save();
    CURRENT.with(|c| *c.borrow_mut() = Some(s));
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Audio,
    Graphics,
    Controls,
    Interface,
    Gameplay,
}

const TABS: [(Tab, &str); 5] = [
    (Tab::Audio, "Audio"),
    (Tab::Graphics, "Graphics"),
    (Tab::Controls, "Controls"),
    (Tab::Interface, "Interface"),
    (Tab::Gameplay, "Gameplay"),
];

/// This frame's mouse.
struct Mouse {
    pos: Vec2,
    pressed: bool,
}

impl Mouse {
    fn clicked(&self, r: Rect) -> bool {
        self.pressed && r.contains(self.pos)
    }
}

/// A slider for a number from `min` to `max`. `id` tells sliders apart
/// while one is dragged. True when the mouse lets go of it.
#[allow(clippy::too_many_arguments)]
fn slider(
    m: &Mouse,
    dragging: &mut Option<u32>,
    id: u32,
    x: f32,
    y: f32,
    label: &str,
    value: &mut u8,
    (min, max): (u8, u8),
    shown: String,
    dim: bool,
) -> bool {
    let track = Rect::new(x + 170.0, y, 220.0, 24.0);
    if m.clicked(track) {
        *dragging = Some(id);
    }
    let mut released = false;
    if *dragging == Some(id) {
        if is_mouse_button_down(MouseButton::Left) {
            let t = ((m.pos.x - track.x) / track.w).clamp(0.0, 1.0);
            *value = (min as f32 + t * (max - min) as f32).round() as u8;
        } else {
            *dragging = None;
            released = true;
        }
    }
    let grey = Color::new(0.7, 0.7, 0.7, 1.0);
    text(label, x, y + 18.0, 19.0, if dim { grey } else { WHITE });
    let ty = track.y + 9.0;
    draw_rectangle(
        track.x,
        ty,
        track.w,
        6.0,
        Color::new(0.15, 0.11, 0.06, 0.95),
    );
    let t = (*value - min) as f32 / (max - min) as f32;
    let fill = if dim {
        Color::new(0.35, 0.33, 0.3, 0.95)
    } else {
        Color::new(0.6, 0.45, 0.18, 0.95)
    };
    draw_rectangle(track.x, ty, track.w * t, 6.0, fill);
    draw_rectangle_lines(track.x, ty, track.w, 6.0, 1.0, BORDER);
    let hover = *dragging == Some(id) || track.contains(m.pos);
    let knob = vec2(track.x + track.w * t, ty + 3.0);
    draw_circle(knob.x, knob.y, 8.0, if hover { GOLD } else { WHITE });
    draw_circle_lines(knob.x, knob.y, 8.0, 1.5, BORDER);
    text(&shown, track.right() + 16.0, y + 18.0, 19.0, WHITE);
    released
}

/// An on/off box with its label. True when clicked (and flipped).
fn checkbox(m: &Mouse, x: f32, y: f32, label: &str, on: &mut bool) -> bool {
    let r = Rect::new(x, y, 22.0, 22.0);
    let hit = Rect::new(x, y, 34.0 + crate::hud::text_width(label, 19.0), 22.0);
    let clicked = m.clicked(hit);
    if clicked {
        *on = !*on;
        audio::play(Sfx::Click);
    }
    draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
    draw_rectangle_lines(
        r.x,
        r.y,
        r.w,
        r.h,
        2.0,
        if hit.contains(m.pos) { GOLD } else { BORDER },
    );
    if *on {
        draw_rectangle(r.x + 5.0, r.y + 5.0, r.w - 10.0, r.h - 10.0, GOLD);
    }
    text(label, r.right() + 10.0, r.y + 17.0, 19.0, WHITE);
    clicked
}

/// A "< choice >" picker. True when it changed.
fn choice(m: &Mouse, x: f32, y: f32, label: &str, options: &[String], index: &mut usize) -> bool {
    text(label, x, y + 18.0, 19.0, WHITE);
    let prev = Rect::new(x + 170.0, y - 2.0, 30.0, 28.0);
    let next = Rect::new(x + 360.0, y - 2.0, 30.0, 28.0);
    let mut changed = false;
    if m.clicked(prev) {
        *index = (*index + options.len() - 1) % options.len();
        changed = true;
    } else if m.clicked(next) {
        *index = (*index + 1) % options.len();
        changed = true;
    }
    if changed {
        audio::play(Sfx::Click);
    }
    button(prev, "<");
    button(next, ">");
    text_centered(&options[*index], x + 280.0, y + 18.0, 19.0, WHITE);
    changed
}

/// The open Settings window. Drawn over everything else; while it's open the
/// screen behind it takes no clicks or keys.
pub struct SettingsWindow {
    settings: Settings,
    tab: Tab,
    /// The slider being dragged.
    dragging: Option<u32>,
    /// The action waiting for a key to be pressed.
    binding: Option<usize>,
    /// A note under the key list ("That key can't be used").
    note: Option<String>,
    /// Every window has been put back where it opens by default.
    reset_windows: bool,
}

impl SettingsWindow {
    pub fn open() -> Self {
        Self {
            settings: current(),
            tab: Tab::Audio,
            dragging: None,
            binding: None,
            note: None,
            reset_windows: false,
        }
    }

    /// Handles input and draws the window. False once it's closed (Done or
    /// Esc). Every change is saved and used straight away.
    pub fn frame(&mut self) -> bool {
        let (w, h) = (800.0, 600.0);
        let r = Rect::new(
            (screen_width() - w) / 2.0,
            ((screen_height() - h) / 2.0).max(10.0),
            w,
            h,
        );
        let m = Mouse {
            pos: vec2(mouse_position().0, mouse_position().1),
            pressed: is_mouse_button_pressed(MouseButton::Left),
        };
        let before = self.settings.clone();
        let mut open = true;

        // Waiting for a key: it takes every key, Esc included.
        let waiting = self.binding.is_some();
        if let Some(i) = self.binding {
            let pressed = get_keys_pressed();
            if pressed.contains(&KeyCode::Escape) || m.pressed {
                self.binding = None;
            } else if let Some(k) = pressed.iter().copied().find(|k| keys::bindable(*k)) {
                self.settings.keys[i] = k;
                self.binding = None;
                self.note = None;
                audio::play(Sfx::Click);
            } else if !pressed.is_empty() {
                self.note = Some("That key is kept for chat, menus or Shift. Pick another.".into());
            }
        }

        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        window(r, "Settings");
        let tab_w = (w - 24.0 - 4.0 * 6.0) / 5.0;
        for (i, (tab, name)) in TABS.iter().enumerate() {
            let t = Rect::new(
                r.x + 12.0 + i as f32 * (tab_w + 6.0),
                r.y + 38.0,
                tab_w,
                30.0,
            );
            if !waiting && m.clicked(t) && self.tab != *tab {
                self.tab = *tab;
                self.dragging = None;
                self.note = None;
                audio::play(Sfx::Click);
            }
            if self.tab == *tab {
                draw_rectangle(t.x, t.y, t.w, t.h, Color::new(0.38, 0.27, 0.11, 0.95));
                draw_rectangle_lines(t.x, t.y, t.w, t.h, 2.0, GOLD);
                text_centered(name, t.x + t.w / 2.0, t.y + 21.0, 18.0, WHITE);
            } else {
                button(t, name);
            }
        }

        // A click that ends waiting for a key does nothing else.
        let m = Mouse {
            pressed: m.pressed && !waiting,
            ..m
        };
        let (x, y) = (r.x + 30.0, r.y + 96.0);
        match self.tab {
            Tab::Audio => self.audio_tab(&m, x, y),
            Tab::Graphics => self.graphics_tab(&m, x, y),
            Tab::Controls => self.controls_tab(&m, r, y),
            Tab::Interface => self.interface_tab(&m, x, y),
            Tab::Gameplay => self.gameplay_tab(&m, x, y),
        }

        let reset = Rect::new(r.x + 24.0, r.bottom() - 60.0, 230.0, 40.0);
        let done = Rect::new(r.right() - 164.0, r.bottom() - 60.0, 140.0, 40.0);
        if m.clicked(reset) {
            audio::play(Sfx::Click);
            self.reset_tab();
        } else if m.clicked(done) {
            audio::play(Sfx::Click);
            open = false;
        }
        button(reset, "Reset This Tab");
        button(done, "Done");
        if !waiting && is_key_pressed(KeyCode::Escape) {
            open = false;
        }

        if self.settings != before {
            let s = &self.settings;
            if s.fullscreen != before.fullscreen {
                set_fullscreen(s.fullscreen);
            }
            if s.window_size != before.window_size || (before.fullscreen && !s.fullscreen) {
                let (w, h) = WINDOW_SIZES[s.window_size];
                request_new_screen_size(w as f32, h as f32);
            }
            set(self.settings.clone());
        }
        open
    }

    fn reset_tab(&mut self) {
        let d = Settings::default();
        let s = &mut self.settings;
        match self.tab {
            Tab::Audio => {
                s.volumes = d.volumes;
                s.mute_all = d.mute_all;
                s.mute_in_background = d.mute_in_background;
            }
            Tab::Graphics => {
                s.fullscreen = d.fullscreen;
                s.window_size = d.window_size;
                s.msaa = d.msaa;
                s.vsync = d.vsync;
                s.frame_cap = d.frame_cap;
                s.show_fps = d.show_fps;
            }
            Tab::Controls => {
                s.keys = d.keys;
                s.mouse_sensitivity = d.mouse_sensitivity;
                s.invert_mouse = d.invert_mouse;
                s.zoom_speed = d.zoom_speed;
                self.binding = None;
                self.note = None;
            }
            Tab::Interface => {
                s.names_players = d.names_players;
                s.names_mobs = d.names_mobs;
                s.names_npcs = d.names_npcs;
                s.damage_numbers = d.damage_numbers;
                s.chat_size = d.chat_size;
                s.show_help = d.show_help;
            }
            Tab::Gameplay => {
                s.decline_invites = d.decline_invites;
                s.decline_duels = d.decline_duels;
            }
        }
    }

    fn audio_tab(&mut self, m: &Mouse, x: f32, y: f32) {
        let s = &mut self.settings;
        let muted = s.mute_all;
        for (i, bus) in Bus::ALL.into_iter().enumerate() {
            let shown = format!("{}%", s.volumes[i]);
            let released = slider(
                m,
                &mut self.dragging,
                i as u32,
                x,
                y + i as f32 * 46.0,
                bus.name(),
                &mut s.volumes[i],
                (0, 100),
                shown,
                muted,
            );
            // Letting go plays a sample at the new volume. (Music and
            // ambience change as you drag.)
            if released {
                // The new volume isn't saved yet; save it so the sample
                // plays at it.
                set(s.clone());
                match bus {
                    Bus::Master | Bus::Interface => audio::play(Sfx::Click),
                    Bus::Effects => audio::play(Sfx::Hit),
                    _ => {}
                }
            }
        }
        let y2 = y + 5.0 * 46.0 + 10.0;
        checkbox(m, x + 10.0, y2, "Mute all", &mut s.mute_all);
        checkbox(
            m,
            x + 240.0,
            y2,
            "Mute when minimized",
            &mut s.mute_in_background,
        );
        let test = Rect::new(x + 10.0, y2 + 50.0, 180.0, 34.0);
        if m.clicked(test) {
            audio::play(Sfx::Heal);
        }
        button(test, "Test Sound");
    }

    fn graphics_tab(&mut self, m: &Mouse, x: f32, y: f32) {
        let s = &mut self.settings;
        let grey = Color::new(0.7, 0.7, 0.7, 1.0);
        checkbox(m, x, y, "Fullscreen", &mut s.fullscreen);
        let sizes: Vec<String> = WINDOW_SIZES
            .iter()
            .map(|(w, h)| format!("{w} x {h}"))
            .collect();
        choice(m, x, y + 46.0, "Window size", &sizes, &mut s.window_size);
        let aa: Vec<String> = MSAA
            .iter()
            .map(|&n| {
                if n == 1 {
                    "Off".into()
                } else {
                    format!("{n}x")
                }
            })
            .collect();
        let mut i = MSAA.iter().position(|&n| n == s.msaa).unwrap_or(2);
        if choice(m, x, y + 92.0, "Anti-aliasing", &aa, &mut i) {
            s.msaa = MSAA[i];
        }
        text("(after a restart)", x + 410.0, y + 110.0, 16.0, grey);
        checkbox(m, x, y + 138.0, "VSync", &mut s.vsync);
        text("(after a restart)", x + 110.0, y + 155.0, 16.0, grey);
        let caps: Vec<String> = FRAME_CAPS
            .iter()
            .map(|&c| {
                if c == 0 {
                    "No cap".into()
                } else {
                    format!("{c} fps")
                }
            })
            .collect();
        let mut i = FRAME_CAPS
            .iter()
            .position(|&c| c == s.frame_cap)
            .unwrap_or(0);
        if choice(m, x, y + 184.0, "Frame cap", &caps, &mut i) {
            s.frame_cap = FRAME_CAPS[i];
        }
        checkbox(m, x, y + 230.0, "Show frames per second", &mut s.show_fps);
    }

    fn controls_tab(&mut self, m: &Mouse, r: Rect, y: f32) {
        let s = &mut self.settings;
        let rows = ACTIONS.len().div_ceil(2);
        let row_h = 25.0;
        for (i, action) in ACTIONS.iter().enumerate() {
            let (col, row) = (i / rows, i % rows);
            let x = r.x + 30.0 + col as f32 * 380.0;
            let ry = y - 18.0 + row as f32 * row_h;
            let key_box = Rect::new(x + 190.0, ry, 140.0, row_h - 3.0);
            if m.clicked(key_box) {
                self.binding = Some(i);
                self.note = None;
                audio::play(Sfx::Click);
            }
            text(&action.name(), x, ry + 17.0, 18.0, WHITE);
            let k = s.keys[i];
            let taken = s.keys.iter().filter(|&&o| o == k).count() > 1;
            let waiting = self.binding == Some(i);
            let hover = key_box.contains(m.pos);
            draw_rectangle(
                key_box.x,
                key_box.y,
                key_box.w,
                key_box.h,
                if waiting {
                    Color::new(0.38, 0.27, 0.11, 0.95)
                } else {
                    Color::new(0.12, 0.09, 0.06, 0.95)
                },
            );
            draw_rectangle_lines(
                key_box.x,
                key_box.y,
                key_box.w,
                key_box.h,
                1.5,
                if hover || waiting { GOLD } else { BORDER },
            );
            let (label, color) = if waiting {
                ("Press a key".to_string(), GOLD)
            } else if taken {
                (keys::label(k), Color::new(1.0, 0.4, 0.35, 1.0))
            } else {
                (keys::label(k), WHITE)
            };
            text_centered(&label, key_box.x + key_box.w / 2.0, ry + 17.0, 17.0, color);
        }
        let y2 = y - 18.0 + rows as f32 * row_h + 12.0;
        let grey = Color::new(0.75, 0.75, 0.75, 1.0);
        let note = if let Some(n) = &self.note {
            n.clone()
        } else if s
            .keys
            .iter()
            .enumerate()
            .any(|(i, k)| s.keys[i + 1..].contains(k))
        {
            "Keys in red do two things. Click one to change it.".into()
        } else {
            "Click a key to change it. Shift + hotbar keys use the top bar.".into()
        };
        text(&note, r.x + 30.0, y2 + 4.0, 16.0, grey);
        let x = r.x + 30.0;
        let shown = format!("{}%", s.mouse_sensitivity);
        slider(
            m,
            &mut self.dragging,
            10,
            x,
            y2 + 18.0,
            "Mouse sensitivity",
            &mut s.mouse_sensitivity,
            (25, 250),
            shown,
            false,
        );
        let shown = format!("{}%", s.zoom_speed);
        slider(
            m,
            &mut self.dragging,
            11,
            x,
            y2 + 52.0,
            "Zoom speed",
            &mut s.zoom_speed,
            (25, 250),
            shown,
            false,
        );
        checkbox(m, x + 520.0, y2 + 20.0, "Invert mouse", &mut s.invert_mouse);
    }

    fn interface_tab(&mut self, m: &Mouse, x: f32, y: f32) {
        let s = &mut self.settings;
        text("Names over:", x, y + 17.0, 19.0, GOLD);
        checkbox(m, x + 140.0, y, "Players", &mut s.names_players);
        checkbox(m, x + 270.0, y, "Monsters", &mut s.names_mobs);
        checkbox(
            m,
            x + 420.0,
            y,
            "Merchants and quest givers",
            &mut s.names_npcs,
        );
        checkbox(
            m,
            x,
            y + 46.0,
            "Damage and healing numbers",
            &mut s.damage_numbers,
        );
        let shown = format!("{}", s.chat_size);
        slider(
            m,
            &mut self.dragging,
            20,
            x,
            y + 92.0,
            "Chat text size",
            &mut s.chat_size,
            (12, 26),
            shown,
            false,
        );
        checkbox(
            m,
            x,
            y + 138.0,
            "Show the controls help when you log in",
            &mut s.show_help,
        );
        let reset = Rect::new(x, y + 184.0, 260.0, 34.0);
        if m.clicked(reset) {
            audio::play(Sfx::Click);
            crate::drag::reset();
            self.reset_windows = true;
        }
        button(reset, "Reset Window Positions");
        if self.reset_windows {
            text(
                "Windows will open where they started.",
                x + 280.0,
                y + 206.0,
                16.0,
                Color::new(0.75, 0.75, 0.75, 1.0),
            );
        }
    }

    fn gameplay_tab(&mut self, m: &Mouse, x: f32, y: f32) {
        let s = &mut self.settings;
        checkbox(
            m,
            x,
            y,
            "Turn down party invites automatically",
            &mut s.decline_invites,
        );
        checkbox(
            m,
            x,
            y + 46.0,
            "Turn down duel challenges automatically",
            &mut s.decline_duels,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_settings_load_back() {
        let mut s = Settings {
            volumes: [10, 0, 100, 55, 3],
            mute_all: true,
            mute_in_background: true,
            fullscreen: true,
            window_size: 3,
            msaa: 8,
            vsync: false,
            frame_cap: 60,
            show_fps: true,
            mouse_sensitivity: 150,
            invert_mouse: true,
            zoom_speed: 40,
            names_players: false,
            names_mobs: false,
            names_npcs: false,
            damage_numbers: false,
            chat_size: 22,
            show_help: false,
            decline_invites: true,
            decline_duels: true,
            ..Settings::default()
        };
        s.keys[0] = KeyCode::Up;
        s.keys[10] = KeyCode::Kp1;
        assert_eq!(Settings::parse(&s.to_text()), s);
    }

    #[test]
    fn missing_or_bad_lines_keep_defaults() {
        let s = Settings::parse(
            "music_volume=20\nmaster_volume=abc\nfuture_thing=1\nmute_all=maybe\nkey_jump=Escape\nmsaa=3\n",
        );
        assert_eq!(s.volumes, [80, 20, 80, 60, 70]);
        assert!(!s.mute_all);
        assert_eq!(s.keys, Settings::default().keys);
        assert_eq!(s.msaa, 4);
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn numbers_stay_in_range() {
        let s = Settings::parse("effects_volume=250\nchat_size=99\nwindow_size=40\nzoom_speed=1");
        assert_eq!(s.volumes[2], 100);
        assert_eq!(s.chat_size, 26);
        assert_eq!(s.window_size, WINDOW_SIZES.len() - 1);
        assert_eq!(s.zoom_speed, 25);
    }

    #[test]
    fn v8_3_files_still_load() {
        let s = Settings::parse("master_volume=40\nmute_in_background=true\n");
        assert_eq!(s.volumes[0], 40);
        assert!(s.mute_in_background);
    }
}
