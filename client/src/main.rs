// Release builds on Windows shouldn't open a console window next to the game.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game;
mod gfx;
mod hud;
mod menu;
mod models;
mod panels;
mod quests_ui;
mod render;
mod vfx;
mod world;

use macroquad::prelude::*;

use game::{Game, Outcome};
use menu::{CharacterOutcome, Characters, Login};
use render::Scene;

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: format!("Rusty MMO {}", shared::VERSION),
            window_width: 1440,
            window_height: 900,
            sample_count: 4,
            window_resizable: true,
            ..Default::default()
        },
        draw_call_vertex_capacity: 65_000,
        draw_call_index_capacity: 200_000,
        ..Default::default()
    }
}

enum Screen {
    Login,
    Characters(Box<Characters>),
    /// Playing, plus the character list to go back to on logout.
    Game(Box<Game>, Box<Characters>),
}

#[macroquad::main(window_conf)]
async fn main() {
    let scene = Scene::new();
    // The login screen lives for the whole run, so solo play keeps using
    // the same local server.
    let mut login = Login::new();
    let mut screen = Screen::Login;
    loop {
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
                CharacterOutcome::Play(game) => Screen::Game(game, chars),
            },
            Screen::Game(mut game, mut chars) => match game.frame(&scene) {
                Outcome::Continue => Screen::Game(game, chars),
                Outcome::Logout => {
                    chars.resume(game.into_connection());
                    Screen::Characters(chars)
                }
                Outcome::Disconnected(reason) => {
                    login = login.with_message(reason);
                    Screen::Login
                }
            },
        };
        next_frame().await;
    }
}
