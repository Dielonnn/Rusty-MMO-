// Release builds on Windows shouldn't open a console window next to the game.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod drag;
mod game;
mod gfx;
mod hud;
mod keys;
mod menu;
mod models;
mod music;
mod panels;
mod quests_ui;
mod render;
mod settings;
mod spellbook;
mod synth;
mod vfx;
mod world;

use macroquad::prelude::*;

use game::{Game, Outcome};
use menu::{CharacterOutcome, Characters, Login, Mode};
use render::Scene;

fn window_conf() -> macroquad::conf::Conf {
    // The window is made before the game starts, so these settings take
    // effect on the next start.
    let s = settings::Settings::load();
    let (w, h) = settings::WINDOW_SIZES[s.window_size];
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: format!("Rusty MMO {}", shared::version()),
            window_width: w as i32,
            window_height: h as i32,
            fullscreen: s.fullscreen,
            sample_count: s.msaa as i32,
            window_resizable: true,
            platform: miniquad::conf::Platform {
                swap_interval: Some(if s.vsync { 1 } else { 0 }),
                ..Default::default()
            },
            ..Default::default()
        },
        draw_call_vertex_capacity: 65_000,
        draw_call_index_capacity: 200_000,
        ..Default::default()
    }
}

/// Holds the frame rate down to the cap in Settings, and shows it if asked.
fn pace_frame(started: f64) {
    let (cap, show) = settings::with(|s| (s.frame_cap, s.show_fps));
    if show {
        let fps = format!("{} fps", get_fps());
        hud::text(&fps, screen_width() / 2.0 - 30.0, 20.0, 18.0, WHITE);
    }
    if cap > 0 {
        let left = 1.0 / cap as f64 - (get_time() - started);
        if left > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f64(left));
        }
    }
}

enum Screen {
    Login,
    Characters(Box<Characters>),
    /// Playing, plus the account and how you're playing, to go back to the
    /// character list on logout.
    Game(Box<Game>, String, Mode),
}

#[macroquad::main(window_conf)]
async fn main() {
    let scene = Scene::new();
    audio::init().await;
    // The login screen lives for the whole run, so solo play keeps using
    // the same local server.
    let mut login = Login::new();
    let mut screen = Screen::Login;
    loop {
        let started = get_time();
        audio::update();
        if !matches!(screen, Screen::Game(..)) {
            audio::set_music(Some(music::Track::Menu));
            audio::set_ambience(None);
        }
        screen = match screen {
            Screen::Login => match login.frame(&scene) {
                Some(chars) => Screen::Characters(Box::new(chars)),
                None => Screen::Login,
            },
            Screen::Characters(mut chars) => match chars.frame(&scene) {
                CharacterOutcome::Stay => Screen::Characters(chars),
                CharacterOutcome::Back(message) => {
                    if let Some(m) = message {
                        login = login.with_message(m);
                    }
                    Screen::Login
                }
                CharacterOutcome::Play(game) => {
                    let (account, solo) = chars.account();
                    Screen::Game(game, account.to_string(), solo)
                }
            },
            Screen::Game(mut game, account, solo) => match game.frame(&scene) {
                Outcome::Continue => Screen::Game(game, account, solo),
                Outcome::Logout => Screen::Characters(Box::new(Characters::new(
                    game.into_connection(),
                    account,
                    solo,
                ))),
                Outcome::Disconnected(reason) => {
                    login = login.with_message(reason);
                    Screen::Login
                }
            },
        };
        pace_frame(started);
        next_frame().await;
    }
}
