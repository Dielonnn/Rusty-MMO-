// Release builds on Windows shouldn't open a console window next to the game.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game;
mod hud;
mod menu;
mod render;

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
    /// Playing, plus the account and whether it's solo, to go back to the
    /// character list on logout.
    Game(Box<Game>, String, bool),
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
        next_frame().await;
    }
}
