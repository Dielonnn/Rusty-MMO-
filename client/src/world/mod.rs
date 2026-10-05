//! The scenery of every zone: terrain, sky, weather, water,
//! trees, buildings and props, and each zone's colors and light.

mod buildings;
mod dungeon;
mod foliage;
mod paint;
mod props;
mod rocks;
mod sky;
mod terrain;
mod theme;

use std::cell::{OnceCell, RefCell};

use macroquad::models::{Mesh, draw_mesh};
use macroquad::prelude::*;
use shared::world::*;

use crate::gfx::{Batch, Fog, Frame, GroundPaint, Shading, c, dark, mix};

pub use dungeon::dungeon_theme;
use props::{draw_prop, flag};
pub use sky::draw_sky;
use sky::weather;
use terrain::{cave_ceiling, ground_cover, meadows, terrain};
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
    terrain: Vec<Mesh>,
    ground: GroundPaint,
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

/// Static scenery for every zone (built the first time it's seen), the copy
/// of the Sunken Vault you're in, and the shader that lights and fogs it.
pub struct Scene {
    zones: [OnceCell<ZoneScene>; Zone::ALL.len()],
    dungeon: RefCell<Option<dungeon::DungeonScene>>,
    shading: Shading,
    /// The cut-out cards trees, bushes and grass are made of.
    foliage: OnceCell<Texture2D>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            zones: Default::default(),
            dungeon: RefCell::new(None),
            shading: Shading::new(),
            foliage: OnceCell::new(),
        }
    }

    fn get(&self, zone: Zone) -> &ZoneScene {
        self.zones[zone.index()]
            .get_or_init(|| build_zone(zone, self.foliage.get_or_init(paint::foliage)))
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
            mix(t.sky_horizon, t.sky_mid, 0.4),
        );
    }

    /// Like `begin_3d`, for copy `index` of a dungeon.
    pub fn begin_dungeon(&self, index: u32) {
        let t = dungeon_theme(index);
        self.shading.begin(
            &t.light,
            Fog {
                color: t.fog,
                near: t.fog_near,
                far: t.fog_far,
            },
            t.fog,
        );
    }

    /// Draws copy `index` of a dungeon (building it if it's new).
    pub fn draw_dungeon(&self, index: u32) {
        let mut d = self.dungeon.borrow_mut();
        if d.as_ref().is_none_or(|d| d.index != index) {
            *d = Some(dungeon::build(index));
        }
        for m in &d.as_ref().unwrap().meshes {
            draw_mesh(m);
        }
    }

    /// The dungeon's flickering torches and glowing waystone.
    pub fn draw_dungeon_effects(&self, b: &mut Batch, time: f32) {
        if let Some(d) = self.dungeon.borrow().as_ref() {
            glows(b, &d.lamps, &d.fires, time);
        }
    }

    pub fn end_3d(&self) {
        self.shading.end();
    }

    pub fn draw(&self, zone: Zone) {
        let z = self.get(zone);
        self.shading.draw_terrain(&z.terrain, &z.ground);
        for m in &z.meshes {
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
        glows(b, &z.lamps, &z.fires, time);
    }

    /// Draw last: it's see-through.
    pub fn draw_water(&self, zone: Zone) {
        self.shading
            .draw_water(&self.get(zone).water, get_time() as f32);
    }
}

/// Lamps that glow and fires that flicker.
fn glows(b: &mut Batch, lamps: &[Glow], fires: &[Glow], time: f32) {
    for l in lamps {
        let flicker = 0.9 + (time * 9.0 + l.pos.x).sin() * 0.05;
        b.glow_sphere(l.pos, 0.22 * flicker, l.color);
        b.glow_sphere(
            l.pos,
            0.45 * flicker,
            Color::new(l.color.r, l.color.g, l.color.b, 0.25),
        );
    }
    for f in fires {
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

fn build_zone(zone: Zone, foliage: &Texture2D) -> ZoneScene {
    let t = theme(zone);
    let mut ground = Batch::recording();
    terrain(&mut ground, zone);
    let mut b = Batch::recording();
    let mut cards = Batch::recording();
    cards.set_texture(Some(foliage.clone()));
    let mut chimneys = Vec::new();
    let mut lamps = Vec::new();
    let mut fires = Vec::new();
    let mut flags = Vec::new();
    ground_cover(&mut b, &mut cards, zone);
    meadows(&mut cards, zone);
    for p in shared::props::props(zone) {
        draw_prop(
            &mut b,
            &mut cards,
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
    let mut meshes = b.finish();
    meshes.extend(cards.finish());
    ZoneScene {
        terrain: ground.finish(),
        ground: paint::ground(zone),
        meshes,
        water: water(zone, t.water),
        chimneys,
        lamps,
        fires,
        flags,
    }
}

/// The water's surface wherever it might show above the ground, in tiles
/// that each carry how deep the water is at their corners (for the
/// shader's shallows and foam) and whether it's frozen.
fn water(zone: Zone, color: Color) -> Vec<Mesh> {
    const STEP: f32 = 3.0;
    let level = zone.water_level();
    let frozen = if zone == Zone::Frostcog { 1.0 } else { 0.0 };
    let center = zone.center();
    let n = (WORLD_HALF_SIZE * 2.0 / STEP) as i32;
    let at = |i: i32, j: i32| {
        let x = center.x - WORLD_HALF_SIZE + i as f32 * STEP;
        let z = center.y - WORLD_HALF_SIZE + j as f32 * STEP;
        (vec3(x, level, z), level - terrain_height(x, z))
    };
    let mut w = Batch::recording();
    for j in 0..n {
        for i in 0..n {
            let corners = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            // Skip tiles where the ground is well above the water.
            if corners.iter().all(|&(_, deep)| deep < -0.6) {
                continue;
            }
            w.lit(|w| {
                w.quad_uv(
                    corners.map(|(p, _)| p),
                    corners.map(|(_, deep)| vec2(deep, frozen)),
                    Vec3::Y,
                    color,
                )
            });
        }
    }
    w.finish()
}
