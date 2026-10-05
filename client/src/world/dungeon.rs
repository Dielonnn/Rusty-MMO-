//! The dungeons: flagstone floors, walls and ceiling, wall torches, and
//! each one's own light. The Sunken Vault is wet stone in drowned
//! blue-green light, the Cinderforge black basalt cracked with lava, and
//! Frosthowl Cavern packed snow under walls of ice. The walls and the rest
//! of the props come from `shared::dungeon`, so you bump into what you see.

use macroquad::models::Mesh;
use macroquad::prelude::*;
use shared::dungeon::{self, DungeonId, WALL_HEIGHT};
use shared::props::Scatter;
use shared::world::Zone;

use super::props::draw_prop;
use super::theme::Theme;
use super::{Flag, Glow};
use crate::gfx::{Batch, Light, c, dark, mix};

/// How a dungeon looks.
struct Look {
    /// The zone whose colors it borrows for its pillars, crystals, rocks
    /// and braziers.
    zone: Zone,
    /// Walls, and the ceiling (darker).
    stone: Color,
    /// Mixed into the foot of the walls, and here and there on the floor.
    stain: Color,
    floor: Color,
    torch: Color,
    /// Glowing cracks in the floor, and how many tiles have one.
    cracks: Option<(Color, f32)>,
}

fn look(id: DungeonId) -> Look {
    match id {
        DungeonId::SunkenVault => Look {
            zone: Zone::Grubdeep,
            stone: STONE,
            stain: MOSS,
            floor: c(0.3, 0.32, 0.33),
            torch: c(0.35, 0.95, 0.85),
            cracks: None,
        },
        DungeonId::Cinderforge => Look {
            zone: Zone::Scorchsand,
            stone: c(0.2, 0.18, 0.18),
            stain: c(0.42, 0.16, 0.08),
            floor: c(0.17, 0.15, 0.15),
            torch: c(1.0, 0.55, 0.15),
            cracks: Some((c(1.0, 0.42, 0.08), 0.18)),
        },
        DungeonId::Frosthowl => Look {
            zone: Zone::Frostcog,
            stone: c(0.6, 0.72, 0.82),
            stain: c(0.92, 0.95, 1.0),
            floor: c(0.82, 0.86, 0.9),
            torch: c(0.5, 0.85, 1.0),
            cracks: None,
        },
    }
}

/// The colors a wall prop is drawn in, picked by the zone the dungeon
/// borrows its looks from.
fn wall_colors(zone: Zone) -> (Color, Color) {
    let id = DungeonId::ALL
        .into_iter()
        .find(|&d| look(d).zone == zone)
        .unwrap_or(DungeonId::SunkenVault);
    let l = look(id);
    (l.stone, l.stain)
}

/// One copy of the vault's scenery.
pub(super) struct DungeonScene {
    pub index: u32,
    pub meshes: Vec<Mesh>,
    pub lamps: Vec<Glow>,
    pub fires: Vec<Glow>,
}

/// The light and fog in copy `index` of a dungeon.
pub fn dungeon_theme(index: u32) -> Theme {
    let (fog, sun, ambient, water) = match dungeon::of(index).id {
        DungeonId::SunkenVault => (
            c(0.04, 0.07, 0.08),
            vec3(0.32, 0.42, 0.44),
            vec3(0.34, 0.37, 0.4),
            Color::new(0.1, 0.35, 0.38, 0.8),
        ),
        // Smoky, lit red from below.
        DungeonId::Cinderforge => (
            c(0.12, 0.05, 0.03),
            vec3(0.62, 0.32, 0.18),
            vec3(0.4, 0.28, 0.24),
            Color::new(0.9, 0.35, 0.08, 0.9),
        ),
        // Pale blue, and brighter: the snow throws the light back.
        DungeonId::Frosthowl => (
            c(0.42, 0.5, 0.6),
            vec3(0.5, 0.58, 0.68),
            vec3(0.5, 0.55, 0.62),
            Color::new(0.5, 0.7, 0.85, 0.8),
        ),
    };
    Theme {
        light: Light {
            sun_dir: vec3(0.25, 0.9, 0.35).normalize(),
            sun,
            ambient,
        },
        sky_top: fog,
        sky_mid: fog,
        sky_horizon: fog,
        fog,
        fog_near: 18.0,
        fog_far: 75.0,
        water,
        cave: true,
        stars: 0.0,
        sun_disc: None,
    }
}

const STONE: Color = Color::new(0.36, 0.39, 0.4, 1.0);
const MOSS: Color = Color::new(0.2, 0.32, 0.28, 1.0);

pub(super) fn build(index: u32) -> DungeonScene {
    let d = dungeon::of(index);
    let look = look(d.id);
    let center = dungeon::center(index);
    let at = |x: f32, y: f32, z: f32| vec3(center.x + x, y, center.y + z);
    let mut b = Batch::recording();
    let mut rng = Scatter(0xD0_0D + index);
    // Flagstones: two-meter tiles, each a slightly different stone, and a
    // matching ceiling of dark slabs.
    for h in d.halls {
        let lo = h.center - h.half;
        let (nx, nz) = ((h.half.x).ceil() as i32, (h.half.y).ceil() as i32);
        for i in 0..nx {
            for j in 0..nz {
                let x0 = lo.x + i as f32 * 2.0;
                let z0 = lo.y + j as f32 * 2.0;
                let x1 = (x0 + 2.0).min(h.center.x + h.half.x);
                let z1 = (z0 + 2.0).min(h.center.y + h.half.y);
                let shade = rng.range(0.85, 1.08);
                let wet = mix(look.floor, look.stain, rng.range(0.0, 0.35));
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
                    dark(look.stone, 0.55 * shade),
                );
                // A glowing crack across some tiles.
                if let Some((glow, share)) = look.cracks
                    && rng.unit() < share
                {
                    let (x, z) = (rng.range(x0, x1), rng.range(z0, z1));
                    let yaw = rng.range(0.0, std::f32::consts::PI);
                    let half = ((x1 - x0).min(z1 - z0) * 0.45).max(0.2);
                    b.lit(|b| b.block(at(x, 0.02, z), vec3(0.07, 0.01, half), yaw, glow));
                }
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
            look.zone,
            &p,
            &mut chimneys,
            &mut lamps,
            &mut fires,
            &mut flags,
        );
    }
    // Torches in iron brackets along the walls.
    for (p, inward) in d.torches() {
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
            color: look.torch,
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

/// A run of a dungeon's wall: big courses of stone (or basalt, or ice),
/// stained at the foot. `zone` is the one the dungeon borrows its looks
/// from.
pub(super) fn wall(b: &mut Batch, zone: Zone, pos: Vec3, yaw: f32, half_len: f32) {
    let (stone, stain) = wall_colors(zone);
    let t = dungeon::WALL_THICKNESS * 0.5;
    let courses = 4;
    let h = WALL_HEIGHT / courses as f32;
    for k in 0..courses {
        let shade = [0.82, 1.0, 0.92, 0.86][k];
        let col = if k == 0 {
            mix(stone, stain, 0.5)
        } else {
            stone
        };
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
