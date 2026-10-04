// Release builds on Windows shouldn't open a console window next to the game.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game;
mod hud;
mod menu;
mod render;

use macroquad::prelude::*;

use game::{Game, Outcome};
use menu::Menu;
use render::Scene;

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: format!("Rusty MMO v{}", env!("CARGO_PKG_VERSION")),
            window_width: 1440,
            window_height: 900,
            sample_count: 4,
            window_resizable: true,
            ..Default::default()
        },
        // The terrain is one big mesh.
        draw_call_vertex_capacity: 65_000,
        draw_call_index_capacity: 200_000,
        ..Default::default()
    }
}

enum Screen {
    Menu(Menu),
    Game(Box<Game>, Menu),
}

#[macroquad::main(window_conf)]
async fn main() {
    let scene = Scene::new();
    let mut screen = Screen::Menu(Menu::new(None));
    loop {
        screen = match screen {
            Screen::Menu(mut menu) => match menu.frame(&scene) {
                Some(game) => Screen::Game(Box::new(game), menu),
                None => Screen::Menu(menu),
            },
            Screen::Game(mut game, menu) => match game.frame(&scene) {
                Outcome::Continue => Screen::Game(game, menu),
                Outcome::Leave(reason) => Screen::Menu(menu.returning(reason)),
            },
        };
        next_frame().await;
    }
}
