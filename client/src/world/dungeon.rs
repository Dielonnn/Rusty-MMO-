//! The Sunken Vault: flagstone floors, stone walls and ceiling, wall
//! torches, and its drowned blue-green light. The walls and the rest of the
//! vault's props come from `shared::dungeon`, so you bump into what you see.

use macroquad::models::Mesh;
use macroquad::prelude::*;
use shared::dungeon::{self, HALLS, WALL_HEIGHT};
use shared::props::Scatter;
use shared::world::Zone;

use super::props::draw_prop;
use super::theme::Theme;
use super::{Flag, Glow};
use crate::gfx::{Batch, Light, c, dark, mix};

/// The zone whose colors the vault borrows for its pillars, runes and
/// braziers (a cavern's).
pub(super) const LOOKS_LIKE: Zone = Zone::Grubdeep;

/// One copy of the vault's scenery.
pub(super) struct DungeonScene {
    pub index: u32,
    pub meshes: Vec<Mesh>,
    pub lamps: Vec<Glow>,
    pub fires: Vec<Glow>,
}

pub fn dungeon_theme() -> Theme {
    let fog = c(0.04, 0.07, 0.08);
    Theme {
        light: Light {
            sun_dir: vec3(0.25, 0.9, 0.35).normalize(),
            sun: vec3(0.32, 0.42, 0.44),
            ambient: vec3(0.34, 0.37, 0.4),
        },
        sky_top: fog,
        sky_mid: fog,
        sky_horizon: fog,
        fog,
        fog_near: 18.0,
        fog_far: 75.0,
        water: Color::new(0.1, 0.35, 0.38, 0.8),
        cave: true,
        stars: 0.0,
        sun_disc: None,
    }
}

const STONE: Color = Color::new(0.36, 0.39, 0.4, 1.0);
const MOSS: Color = Color::new(0.2, 0.32, 0.28, 1.0);

pub(super) fn build(index: u32) -> DungeonScene {
    let center = dungeon::center(index);
    let at = |x: f32, y: f32, z: f32| vec3(center.x + x, y, center.y + z);
    let mut b = Batch::recording();
    let mut rng = Scatter(0xD0_0D + index);
    // Flagstones: two-meter tiles, each a slightly different stone, and a
    // matching ceiling of dark slabs.
    for h in &HALLS {
        let lo = h.center - h.half;
        let (nx, nz) = ((h.half.x).ceil() as i32, (h.half.y).ceil() as i32);
        for i in 0..nx {
            for j in 0..nz {
                let x0 = lo.x + i as f32 * 2.0;
                let z0 = lo.y + j as f32 * 2.0;
                let x1 = (x0 + 2.0).min(h.center.x + h.half.x);
                let z1 = (z0 + 2.0).min(h.center.y + h.half.y);
                let shade = rng.range(0.85, 1.08);
                let wet = mix(c(0.3, 0.32, 0.33), MOSS, rng.range(0.0, 0.35));
                let floor = Color::new(wet.r * shade, wet.g * shade, wet.b * shade, 1.0);
                b.quad(
                    [
                        at(x0, 0.01, z0),
                        at(x0, 0.01, z1),
                        at(x1, 0.01, z1),
                        at(x1, 0.01, z0),
                    ],
                    Vec3::Y,
                    floor,
                );
                b.quad(
                    [
                        at(x0, WALL_HEIGHT, z0),
                        at(x1, WALL_HEIGHT, z0),
                        at(x1, WALL_HEIGHT, z1),
                        at(x0, WALL_HEIGHT, z1),
                    ],
                    -Vec3::Y,
                    dark(STONE, 0.55 * shade),
                );
            }
        }
    }
    let mut lamps = Vec::new();
    let mut fires = Vec::new();
    let (mut chimneys, mut flags): (Vec<Vec3>, Vec<Flag>) = (Vec::new(), Vec::new());
    let mut cards = Batch::recording();
    for p in dungeon::props(index) {
        draw_prop(
            &mut b,
            &mut cards,
            LOOKS_LIKE,
            &p,
            &mut chimneys,
            &mut lamps,
            &mut fires,
            &mut flags,
        );
    }
    // Torches in iron brackets along the walls.
    for (p, inward) in dungeon::torches() {
        let p = at(p.x, p.y, p.z);
        let out = vec3(inward.x, 0.0, inward.y);
        let iron = c(0.18, 0.17, 0.17);
        b.block(
            p - out * 0.25 - Vec3::Y * 0.35,
            vec3(0.05, 0.05, 0.25),
            inward.x.atan2(inward.y),
            iron,
        );
        b.cone(p - Vec3::Y * 0.45, Vec3::Y * 0.45, 0.06, 0.14, 6, iron);
        fires.push(Glow {
            pos: p,
            color: c(0.35, 0.95, 0.85),
        });
    }
    let mut meshes = b.finish();
    meshes.extend(cards.finish());
    DungeonScene {
        index,
        meshes,
        lamps,
        fires,
    }
}

/// A run of the vault's wall: big stone courses, mossy at the foot.
pub(super) fn wall(b: &mut Batch, pos: Vec3, yaw: f32, half_len: f32) {
    let t = dungeon::WALL_THICKNESS * 0.5;
    let courses = 4;
    let h = WALL_HEIGHT / courses as f32;
    for k in 0..courses {
        let shade = [0.82, 1.0, 0.92, 0.86][k];
        let col = if k == 0 { mix(STONE, MOSS, 0.5) } else { STONE };
        b.block(
            pos + Vec3::Y * (h * (k as f32 + 0.5)),
            vec3(t, h * 0.5, half_len),
            yaw,
            dark(col, shade),
        );
    }
}

/// A waystone: a tall, rough rune stone with glowing runes up its faces.
pub(super) fn waystone(b: &mut Batch, pos: Vec3, yaw: f32, lamps: &mut Vec<Glow>) {
    let stone = c(0.42, 0.44, 0.5);
    let rune = c(0.45, 0.9, 1.0);
    b.block(
        pos + Vec3::Y * 0.2,
        vec3(1.0, 0.2, 1.0),
        yaw,
        dark(stone, 0.75),
    );
    b.cone(pos + Vec3::Y * 0.4, Vec3::Y * 3.2, 0.6, 0.35, 6, stone);
    b.cone(pos + Vec3::Y * 3.6, Vec3::Y * 0.5, 0.36, 0.0, 6, stone);
    let f = shared::world::forward(yaw);
    let side = vec3(-f.z, 0.0, f.x);
    for face in [f, -f, side, -side] {
        for k in 0..3 {
            let y = 1.1 + k as f32 * 0.75;
            let inset = 0.6 - (y - 0.4) / 3.2 * 0.25;
            b.lit(|b| {
                b.block(
                    pos + face * inset + Vec3::Y * y,
                    vec3(0.12, 0.2, 0.03),
                    face.x.atan2(face.z),
                    rune,
                )
            });
        }
    }
    lamps.push(Glow {
        pos: pos + Vec3::Y * 4.3,
        color: rune,
    });
}
