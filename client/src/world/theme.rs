//! Each zone's light, sky, fog and water colors.

use macroquad::prelude::*;
use shared::world::Zone;

use crate::gfx::{Light, c};

/// The look of a zone: light, sky, fog and water.
#[derive(Clone, Copy)]
pub struct Theme {
    pub light: Light,
    pub sky_top: Color,
    pub sky_mid: Color,
    pub sky_horizon: Color,
    pub fog: Color,
    pub fog_near: f32,
    pub fog_far: f32,
    pub water: Color,
    /// Underground: no sky, a rock ceiling.
    pub cave: bool,
    /// How bright the stars are.
    pub stars: f32,
    /// The sun's (or moon's) disc in the sky.
    pub sun_disc: Option<Color>,
}

pub fn theme(zone: Zone) -> Theme {
    match zone {
        // Autumn dusk.
        Zone::Amberfall => Theme {
            light: Light {
                sun_dir: vec3(-0.75, 0.28, 0.45).normalize(),
                sun: vec3(1.0, 0.76, 0.55),
                ambient: vec3(0.42, 0.4, 0.52),
            },
            sky_top: c(0.16, 0.15, 0.34),
            sky_mid: c(0.55, 0.36, 0.5),
            sky_horizon: c(0.98, 0.6, 0.36),
            fog: c(0.74, 0.5, 0.44),
            fog_near: 60.0,
            fog_far: 260.0,
            water: Color::new(0.32, 0.36, 0.55, 0.78),
            cave: false,
            stars: 0.6,
            sun_disc: Some(c(1.0, 0.88, 0.6)),
        },
        // Blazing desert afternoon.
        Zone::Scorchsand => Theme {
            light: Light {
                sun_dir: vec3(0.35, 0.8, 0.3).normalize(),
                sun: vec3(1.05, 0.95, 0.8),
                ambient: vec3(0.5, 0.46, 0.42),
            },
            sky_top: c(0.3, 0.52, 0.85),
            sky_mid: c(0.55, 0.72, 0.92),
            sky_horizon: c(0.96, 0.86, 0.66),
            fog: c(0.92, 0.8, 0.62),
            fog_near: 80.0,
            fog_far: 320.0,
            water: Color::new(0.2, 0.62, 0.66, 0.82),
            cave: false,
            stars: 0.0,
            sun_disc: Some(c(1.0, 0.98, 0.9)),
        },
        // Silver twilight under great trees.
        Zone::Silverbough => Theme {
            light: Light {
                sun_dir: vec3(0.3, 0.6, -0.6).normalize(),
                sun: vec3(0.78, 0.82, 1.0),
                ambient: vec3(0.38, 0.42, 0.55),
            },
            sky_top: c(0.08, 0.1, 0.28),
            sky_mid: c(0.22, 0.3, 0.55),
            sky_horizon: c(0.55, 0.62, 0.85),
            fog: c(0.4, 0.46, 0.68),
            fog_near: 50.0,
            fog_far: 230.0,
            water: Color::new(0.35, 0.55, 0.8, 0.75),
            cave: false,
            stars: 0.9,
            sun_disc: Some(c(0.9, 0.93, 1.0)),
        },
        // A dark cave lit by crystals and mushrooms.
        Zone::Grubdeep => Theme {
            light: Light {
                sun_dir: vec3(0.2, 1.0, 0.1).normalize(),
                sun: vec3(0.3, 0.32, 0.4),
                ambient: vec3(0.42, 0.42, 0.52),
            },
            sky_top: c(0.03, 0.03, 0.05),
            sky_mid: c(0.05, 0.05, 0.07),
            sky_horizon: c(0.07, 0.08, 0.1),
            fog: c(0.06, 0.07, 0.1),
            fog_near: 25.0,
            fog_far: 140.0,
            water: Color::new(0.08, 0.18, 0.24, 0.85),
            cave: true,
            stars: 0.0,
            sun_disc: None,
        },
        // A crisp, bright snowy day.
        Zone::Frostcog => Theme {
            light: Light {
                sun_dir: vec3(-0.4, 0.6, -0.5).normalize(),
                sun: vec3(0.95, 0.97, 1.0),
                ambient: vec3(0.52, 0.56, 0.66),
            },
            sky_top: c(0.38, 0.55, 0.82),
            sky_mid: c(0.62, 0.74, 0.9),
            sky_horizon: c(0.88, 0.92, 0.97),
            fog: c(0.86, 0.9, 0.96),
            fog_near: 55.0,
            fog_far: 240.0,
            water: Color::new(0.78, 0.86, 0.95, 0.95),
            cave: false,
            stars: 0.0,
            sun_disc: Some(c(1.0, 1.0, 0.95)),
        },
        // A sickly, dying forest under a purple sky.
        Zone::Witherwood => Theme {
            light: Light {
                sun_dir: vec3(0.6, 0.35, 0.5).normalize(),
                sun: vec3(0.72, 0.8, 0.62),
                ambient: vec3(0.44, 0.42, 0.5),
            },
            sky_top: c(0.1, 0.06, 0.16),
            sky_mid: c(0.24, 0.16, 0.3),
            sky_horizon: c(0.42, 0.48, 0.34),
            fog: c(0.34, 0.38, 0.3),
            fog_near: 35.0,
            fog_far: 190.0,
            water: Color::new(0.24, 0.32, 0.16, 0.9),
            cave: false,
            stars: 0.4,
            sun_disc: Some(c(0.75, 0.85, 0.6)),
        },
        // A clear, breezy highland morning.
        Zone::Sunfold => Theme {
            light: Light {
                sun_dir: vec3(0.55, 0.5, -0.4).normalize(),
                sun: vec3(1.0, 0.95, 0.82),
                ambient: vec3(0.48, 0.5, 0.58),
            },
            sky_top: c(0.26, 0.45, 0.78),
            sky_mid: c(0.52, 0.68, 0.88),
            sky_horizon: c(0.95, 0.9, 0.78),
            fog: c(0.78, 0.82, 0.84),
            fog_near: 70.0,
            fog_far: 300.0,
            water: Color::new(0.28, 0.45, 0.6, 0.82),
            cave: false,
            stars: 0.0,
            sun_disc: Some(c(1.0, 0.96, 0.82)),
        },
        // Red badlands under a smoky, sickly green sky.
        Zone::Blightscar => Theme {
            light: Light {
                sun_dir: vec3(-0.5, 0.42, 0.55).normalize(),
                sun: vec3(1.0, 0.78, 0.58),
                ambient: vec3(0.46, 0.4, 0.4),
            },
            sky_top: c(0.18, 0.14, 0.12),
            sky_mid: c(0.42, 0.3, 0.22),
            sky_horizon: c(0.72, 0.62, 0.36),
            fog: c(0.55, 0.44, 0.32),
            fog_near: 50.0,
            fog_far: 240.0,
            water: Color::new(0.3, 0.42, 0.14, 0.88),
            cave: false,
            stars: 0.2,
            sun_disc: Some(c(1.0, 0.7, 0.4)),
        },
    }
}
