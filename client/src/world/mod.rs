//! The scenery of all six starting areas: terrain, sky, weather, water,
//! trees, buildings and props, and each zone's colors and light.

mod buildings;
mod props;
mod sky;
mod terrain;
mod theme;

use std::cell::OnceCell;

use macroquad::models::{Mesh, draw_mesh};
use macroquad::prelude::*;
use shared::world::*;

use crate::gfx::{Batch, Fog, Frame, Shading, c, dark, mix};

use props::{draw_prop, flag};
pub use sky::draw_sky;
use sky::weather;
use terrain::{cave_ceiling, ground_cover, terrain};
pub use terrain::{foliage, map_color};
pub use theme::theme;

/// A lamp or fire that flickers, with its color.
#[derive(Clone, Copy)]
struct Glow {
    pos: Vec3,
    color: Color,
}

/// One zone's static scenery.
struct ZoneScene {
    meshes: Vec<Mesh>,
    water: Vec<Mesh>,
    chimneys: Vec<Vec3>,
    lamps: Vec<Glow>,
    fires: Vec<Glow>,
    flags: Vec<Flag>,
}

/// A banner's cloth, which waves in the wind.
#[derive(Clone, Copy)]
struct Flag {
    /// Where the cloth hangs from the pole.
    top: Vec3,
    /// The direction the cloth streams out.
    yaw: f32,
    color: Color,
}

/// Static scenery for every zone (built the first time it's seen) and the
/// shader that lights and fogs it.
pub struct Scene {
    zones: [OnceCell<ZoneScene>; 6],
    shading: Shading,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            zones: Default::default(),
            shading: Shading::new(),
        }
    }

    fn get(&self, zone: Zone) -> &ZoneScene {
        self.zones[zone.index()].get_or_init(|| build_zone(zone))
    }

    /// Call after `set_camera` for a 3D pass: turns on the zone's light
    /// and fog.
    pub fn begin_3d(&self, zone: Zone) {
        let t = theme(zone);
        self.shading.begin(
            &t.light,
            Fog {
                color: t.fog,
                near: t.fog_near,
                far: t.fog_far,
            },
        );
    }

    pub fn end_3d(&self) {
        self.shading.end();
    }

    pub fn draw(&self, zone: Zone) {
        for m in &self.get(zone).meshes {
            draw_mesh(m);
        }
    }

    /// Smoke, flickering fires and lamp glows.
    pub fn draw_effects(&self, zone: Zone, b: &mut Batch, time: f32, around: Vec3) {
        let z = self.get(zone);
        for f in &z.flags {
            flag(b, f, time);
        }
        weather(b, zone, time, around);
        let smoke = if zone == Zone::Frostcog {
            c(0.85, 0.87, 0.9)
        } else {
            c(0.55, 0.52, 0.55)
        };
        for (i, ch) in z.chimneys.iter().enumerate() {
            for k in 0..5 {
                let t = (time * 0.25 + k as f32 / 5.0 + i as f32 * 0.37) % 1.0;
                let p = *ch + vec3((t * 7.0 + i as f32).sin() * 0.3 + t * 1.2, t * 4.5, t * 0.6);
                b.sphere(
                    p,
                    0.25 + t * 0.6,
                    Color::new(smoke.r, smoke.g, smoke.b, 0.55 * (1.0 - t)),
                );
            }
        }
        for l in &z.lamps {
            let flicker = 0.9 + (time * 9.0 + l.pos.x).sin() * 0.05;
            b.glow_sphere(l.pos, 0.22 * flicker, l.color);
            b.glow_sphere(
                l.pos,
                0.45 * flicker,
                Color::new(l.color.r, l.color.g, l.color.b, 0.25),
            );
        }
        for f in &z.fires {
            for k in 0..4 {
                let t = (time * 1.5 + k as f32 * 0.25) % 1.0;
                let p = f.pos + vec3((time * 5.0 + k as f32).sin() * 0.08, 0.3 + t * 1.0, 0.0);
                let fade = Color::new(f.color.r, f.color.g, f.color.b, 0.0);
                b.glow_sphere(
                    p,
                    0.3 * (1.0 - t * 0.6),
                    mix(Color::new(1.0, 0.95, 0.7, 0.95), fade, t),
                );
            }
        }
    }

    /// Draw last: it's see-through.
    pub fn draw_water(&self, zone: Zone) {
        for m in &self.get(zone).water {
            draw_mesh(m);
        }
    }
}

fn build_zone(zone: Zone) -> ZoneScene {
    let t = theme(zone);
    let mut b = Batch::recording();
    let mut chimneys = Vec::new();
    let mut lamps = Vec::new();
    let mut fires = Vec::new();
    let mut flags = Vec::new();
    terrain(&mut b, zone);
    ground_cover(&mut b, zone);
    for p in shared::props::props(zone) {
        draw_prop(
            &mut b,
            zone,
            &p,
            &mut chimneys,
            &mut lamps,
            &mut fires,
            &mut flags,
        );
    }
    if t.cave {
        cave_ceiling(&mut b, zone);
    }
    let meshes = b.finish();

    let mut w = Batch::recording();
    let s = WORLD_HALF_SIZE;
    let center = zone.center();
    let corners = [(-s, -s), (s, -s), (s, s), (-s, s)]
        .map(|(x, z)| vec3(center.x + x, zone.water_level(), center.y + z));
    w.lit(|w| w.quad(corners, Vec3::Y, t.water));
    ZoneScene {
        meshes,
        water: w.finish(),
        chimneys,
        lamps,
        fires,
        flags,
    }
}
