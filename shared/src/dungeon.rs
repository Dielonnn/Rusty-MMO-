//! The Sunken Vault: a dungeon of stone halls under the world, and the
//! waystones that carry players between the towns and into it.
//!
//! Every party (or player on their own) who enters the vault gets a copy of
//! their own, an "instance". The copies sit side by side in a row far from
//! the starting areas (`ROW_Z`), `SPACING` apart, so nothing in one can see
//! or reach anything in another. Everything about the vault's shape is in
//! "vault" coordinates, with the entrance at the origin; `center` says
//! where an instance's origin really is.
//!
//! The halls are rectangles (`HALLS`) on flat ground, walled in wherever
//! open floor meets solid rock. Both sides build the same walls, so the
//! client draws what the server's mobs bump into.

use std::sync::OnceLock;

use glam::{Vec2, Vec3, vec2, vec3};

use crate::data::MobKind;
use crate::props::{Colliders, Prop, PropKind};

pub const NAME: &str = "The Sunken Vault";
/// Everything at or beyond this Z is inside some instance of the vault.
pub const ROW_Z: f32 = 6000.0;
/// Distance between neighbouring instances along X.
pub const SPACING: f32 = 500.0;
/// How many copies of the vault can be open at once.
pub const MAX_INSTANCES: u32 = 64;
/// How far from its entrance an instance reaches, in every direction.
pub const HALF_SIZE: f32 = 200.0;
/// How tall the walls are; the ceiling sits on top of them.
pub const WALL_HEIGHT: f32 = 7.0;
/// How thick the walls are.
pub const WALL_THICKNESS: f32 = 1.0;

/// Where players arrive (and come back after dying), facing into the vault.
pub const ENTRANCE: Vec2 = Vec2::new(0.0, -2.0);
/// The waystone that takes you back out.
pub const EXIT_STONE: Vec2 = Vec2::new(0.0, -6.0);

/// You must be this level to enter.
pub const MIN_LEVEL: u8 = 8;
/// An instance nobody is in is closed (and its mobs are gone) after this
/// many seconds.
pub const EMPTY_RESET: f32 = 300.0;

/// A stretch of open floor: center and half extents.
#[derive(Clone, Copy, Debug)]
pub struct Hall {
    pub name: &'static str,
    pub center: Vec2,
    pub half: Vec2,
}

const fn hall(name: &'static str, x: f32, z: f32, hx: f32, hz: f32) -> Hall {
    Hall {
        name,
        center: Vec2::new(x, z),
        half: Vec2::new(hx, hz),
    }
}

/// The rooms and the corridors between them, in the order you go through
/// them. Corridors have empty names.
pub const HALLS: [Hall; 11] = [
    hall("Flooded Stair", 0.0, 0.0, 8.0, 8.0),
    hall("", 0.0, 16.0, 3.0, 8.0),
    hall("Kennels", 0.0, 36.0, 12.0, 12.0),
    hall("", 20.0, 36.0, 8.0, 3.0),
    hall("Crawler Nest", 42.0, 36.0, 14.0, 14.0),
    hall("", 42.0, 60.0, 3.0, 10.0),
    hall("Warden's Hall", 42.0, 84.0, 12.0, 14.0),
    hall("", 16.0, 84.0, 14.0, 3.0),
    hall("Drowned Chapel", -10.0, 84.0, 12.0, 12.0),
    hall("", -10.0, 106.0, 3.0, 10.0),
    hall("Throne of the Deep", -10.0, 134.0, 18.0, 18.0),
];

/// A group of mobs that fight together: kind, levels and how many, around
/// a spot in the vault.
#[derive(Clone, Copy, Debug)]
pub struct Pack {
    pub center: Vec2,
    pub radius: f32,
    pub mobs: &'static [(MobKind, (u8, u8), usize)],
}

const fn pack(x: f32, z: f32, radius: f32, mobs: &'static [(MobKind, (u8, u8), usize)]) -> Pack {
    Pack {
        center: Vec2::new(x, z),
        radius,
        mobs,
    }
}

use MobKind::*;

/// Every mob in a fresh instance. Pull them one pack at a time.
pub const PACKS: [Pack; 10] = [
    // Kennels
    pack(-6.0, 32.0, 3.0, &[(VaultHound, (9, 9), 2)]),
    pack(6.0, 42.0, 3.0, &[(VaultHound, (9, 9), 2)]),
    pack(
        0.0,
        44.0,
        2.0,
        &[(DrownedEnforcer, (9, 9), 1), (DrownedAdept, (9, 9), 1)],
    ),
    // Crawler Nest
    pack(36.0, 30.0, 4.0, &[(VaultCrawler, (9, 10), 3)]),
    pack(48.0, 42.0, 4.0, &[(VaultCrawler, (9, 10), 3)]),
    // Warden's Hall
    pack(
        38.0,
        78.0,
        2.0,
        &[(DrownedEnforcer, (9, 10), 1), (DrownedAdept, (9, 10), 1)],
    ),
    pack(42.0, 90.0, 1.0, &[(StoneWarden, (10, 10), 1)]),
    // Drowned Chapel
    pack(
        -10.0,
        80.0,
        3.0,
        &[(DrownedEnforcer, (10, 10), 2), (DrownedAdept, (10, 10), 1)],
    ),
    pack(-16.0, 92.0, 2.0, &[(DrownedAdept, (10, 10), 2)]),
    // Throne of the Deep
    pack(-10.0, 142.0, 1.0, &[(SunkenKing, (10, 10), 1)]),
];

/// Where instance `index`'s entrance really is, on the XZ plane.
pub fn center(index: u32) -> Vec2 {
    vec2(index as f32 * SPACING, ROW_Z)
}

/// Which instance of the vault a world position is in, if any.
pub fn instance_at(p: Vec3) -> Option<u32> {
    (p.z > ROW_Z - HALF_SIZE * 2.0).then(|| {
        (p.x / SPACING)
            .round()
            .clamp(0.0, (MAX_INSTANCES - 1) as f32) as u32
    })
}

/// Vault coordinates in instance `index` to a world position on the floor.
pub fn to_world(index: u32, local: Vec2) -> Vec3 {
    let c = center(index) + local;
    vec3(c.x, 0.0, c.y)
}

/// A world position (in any instance) to vault coordinates.
pub fn to_local(p: Vec3) -> Vec2 {
    let c = center(instance_at(p).unwrap_or(0));
    vec2(p.x - c.x, p.z - c.y)
}

/// Whether a spot in vault coordinates is open floor.
pub fn open(local: Vec2) -> bool {
    HALLS.iter().any(|h| {
        let d = (local - h.center).abs();
        d.x <= h.half.x && d.y <= h.half.y
    })
}

/// The named room a spot is in, if any.
pub fn room_at(local: Vec2) -> Option<&'static str> {
    HALLS
        .iter()
        .filter(|h| !h.name.is_empty())
        .find(|h| {
            let d = (local - h.center).abs();
            d.x < h.half.x && d.y < h.half.y
        })
        .map(|h| h.name)
}

/// The corners of the box around every hall, in whole meters.
fn bounds() -> (i32, i32, i32, i32) {
    let mut b = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for h in &HALLS {
        let lo = h.center - h.half;
        let hi = h.center + h.half;
        b.0 = b.0.min(lo.x.floor() as i32 - 1);
        b.1 = b.1.min(lo.y.floor() as i32 - 1);
        b.2 = b.2.max(hi.x.ceil() as i32 + 1);
        b.3 = b.3.max(hi.y.ceil() as i32 + 1);
    }
    b
}

/// One straight piece of wall: its ends (vault coordinates, on the line
/// where floor meets rock) and which side the floor is on.
#[derive(Clone, Copy, Debug)]
pub struct Wall {
    pub from: Vec2,
    pub to: Vec2,
    /// Points from the wall towards the open floor.
    pub inward: Vec2,
}

impl Wall {
    pub fn length(&self) -> f32 {
        self.from.distance(self.to)
    }
}

/// Every wall, found on a one meter grid wherever open floor meets rock,
/// with neighbouring pieces merged into long runs.
pub fn walls() -> &'static [Wall] {
    static WALLS: OnceLock<Vec<Wall>> = OnceLock::new();
    WALLS.get_or_init(|| {
        let (x0, z0, x1, z1) = bounds();
        let cell = |i: i32, j: i32| open(vec2(i as f32 + 0.5, j as f32 + 0.5));
        let mut walls = Vec::new();
        // Walls running along X, on the line between rows j - 1 and j.
        for j in z0..=z1 {
            let mut run: Option<(i32, bool)> = None;
            for i in x0..=x1 + 1 {
                let edge = (i <= x1).then(|| (cell(i, j - 1), cell(i, j)));
                let side = edge.filter(|(a, b)| a != b).map(|(a, _)| a);
                match (run, side) {
                    (Some((_, s)), Some(n)) if s == n => {}
                    _ => {
                        if let Some((start, below_open)) = run {
                            walls.push(Wall {
                                from: vec2(start as f32, j as f32),
                                to: vec2(i as f32, j as f32),
                                inward: vec2(0.0, if below_open { -1.0 } else { 1.0 }),
                            });
                        }
                        run = side.map(|s| (i, s));
                    }
                }
            }
        }
        // Walls running along Z, on the line between columns i - 1 and i.
        for i in x0..=x1 {
            let mut run: Option<(i32, bool)> = None;
            for j in z0..=z1 + 1 {
                let edge = (j <= z1).then(|| (cell(i - 1, j), cell(i, j)));
                let side = edge.filter(|(a, b)| a != b).map(|(a, _)| a);
                match (run, side) {
                    (Some((_, s)), Some(n)) if s == n => {}
                    _ => {
                        if let Some((start, left_open)) = run {
                            walls.push(Wall {
                                from: vec2(i as f32, start as f32),
                                to: vec2(i as f32, j as f32),
                                inward: vec2(if left_open { -1.0 } else { 1.0 }, 0.0),
                            });
                        }
                        run = side.map(|s| (j, s));
                    }
                }
            }
        }
        walls
    })
}

/// Wall torches: where each one hangs (vault coordinates) and which way
/// it faces.
pub fn torches() -> Vec<(Vec3, Vec2)> {
    let mut out = Vec::new();
    for w in walls() {
        let n = (w.length() / 9.0).floor() as i32;
        for k in 0..n {
            let t = (k as f32 + 0.5) / n as f32;
            let p = w.from.lerp(w.to, t) + w.inward * 0.35;
            out.push((vec3(p.x, 2.8, p.y), w.inward));
        }
    }
    out
}

/// Everything solid or decorative in the vault, in vault coordinates
/// (instance 0's world position minus its center).
fn local_props() -> &'static [Prop] {
    static PROPS: OnceLock<Vec<Prop>> = OnceLock::new();
    PROPS.get_or_init(|| {
        let mut props = Vec::new();
        let mut add = |kind, x: f32, z: f32, yaw: f32, size: f32, variant: u8| {
            props.push(Prop {
                kind,
                pos: vec3(x, 0.0, z),
                yaw,
                size,
                variant,
            });
        };
        for w in walls() {
            // Thick blocks behind the line, a little longer than the run so
            // the corners close.
            let mid = (w.from + w.to) * 0.5 - w.inward * WALL_THICKNESS * 0.5;
            let along = (w.to - w.from).normalize();
            let yaw = along.x.atan2(along.y);
            let half = w.length() * 0.5 + WALL_THICKNESS * 0.5;
            add(PropKind::VaultWall, mid.x, mid.y, yaw, half, 0);
        }
        add(PropKind::Waystone, EXIT_STONE.x, EXIT_STONE.y, 0.0, 1.0, 0);
        // The stair: crates and barrels left by the first explorers.
        for (x, z) in [(-6.0, -5.5), (6.0, -5.0), (6.5, 5.5)] {
            add(PropKind::Barrel, x, z, 0.0, 1.0, 0);
        }
        add(PropKind::Crate, -6.0, 5.5, 0.4, 0.55, 0);
        // Bones in the kennels and the nest.
        for (i, (x, z)) in [
            (-8.0, 28.0),
            (8.0, 30.0),
            (-4.0, 46.0),
            (34.0, 44.0),
            (50.0, 28.0),
            (44.0, 48.0),
            (30.0, 36.0),
        ]
        .into_iter()
        .enumerate()
        {
            add(PropKind::Bones, x, z, i as f32 * 1.3, 1.0, i as u8);
        }
        // Pillars down the Warden's Hall and around the throne.
        for z in [74.0, 94.0] {
            for x in [34.0, 50.0] {
                add(PropKind::Pillar, x, z, 0.0, WALL_HEIGHT, 0);
            }
        }
        for k in 0..8 {
            // Turned half a step so none stands in the doorway.
            let a = (k as f32 + 0.5) * std::f32::consts::TAU / 8.0;
            let p = vec2(-10.0, 134.0) + vec2(a.cos(), a.sin()) * 13.0;
            add(PropKind::Pillar, p.x, p.y, a, WALL_HEIGHT, k as u8);
        }
        for k in 0..6 {
            let a = k as f32 * 1.047;
            let p = vec2(-10.0, 140.0) + vec2(a.cos(), a.sin()) * 5.0;
            add(PropKind::RuneTile, p.x, p.y, a, 1.0, 0);
        }
        // Braziers in the chapel.
        for x in [-18.0, -2.0] {
            add(PropKind::Campfire, x, 76.0, 0.0, 1.0, 0);
        }
        props
    })
}

/// Everything in instance `index`, in world coordinates.
pub fn props(index: u32) -> Vec<Prop> {
    let c = center(index);
    local_props()
        .iter()
        .map(|p| Prop {
            pos: p.pos + vec3(c.x, 0.0, c.y),
            ..*p
        })
        .collect()
}

/// What you bump into, in vault coordinates.
fn colliders() -> &'static Colliders {
    static COLLIDERS: OnceLock<Colliders> = OnceLock::new();
    COLLIDERS.get_or_init(|| Colliders::new(local_props()))
}

/// Pushes a circle at a world position (in any instance) out of the walls.
pub fn resolve(pos: Vec3, radius: f32) -> Vec3 {
    let c = center(instance_at(pos).unwrap_or(0));
    let offset = vec3(c.x, 0.0, c.y);
    colliders().resolve(pos - offset, radius) + offset
}

/// Whether a circle at a world position overlaps a wall or anything solid.
pub fn blocked(pos: Vec3, radius: f32) -> bool {
    let c = center(instance_at(pos).unwrap_or(0));
    colliders().blocked(pos - vec3(c.x, 0.0, c.y), radius)
}

/// Keeps a position in its instance, on the floor.
pub fn clamp(pos: Vec3) -> Vec3 {
    let c = center(instance_at(pos).unwrap_or(0));
    let limit = HALF_SIZE - 2.0;
    vec3(
        pos.x.clamp(c.x - limit, c.x + limit),
        pos.y.max(0.0),
        pos.z.clamp(c.y - limit, c.y + limit),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instances_are_apart_from_each_other_and_the_world() {
        for i in [0, 1, 17, MAX_INSTANCES - 1] {
            let p = to_world(i, vec2(30.0, 140.0));
            assert_eq!(instance_at(p), Some(i));
            assert!(to_local(p).distance(vec2(30.0, 140.0)) < 1e-3);
            // Nothing in one instance is within sight of the next.
            const { assert!(SPACING - 2.0 * HALF_SIZE > crate::world::VIEW_DISTANCE * 0.5) };
        }
        for zone in crate::world::Zone::ALL {
            assert_eq!(instance_at(zone.graveyard()), None);
        }
    }

    #[test]
    fn halls_join_up_and_fit() {
        // Walk from each hall's center to the next one's: every step is
        // on open floor, through the corridor between them.
        for pair in HALLS.windows(2) {
            let (a, b) = (pair[0].center, pair[1].center);
            // Corridors run straight along one axis from their room.
            let corner = if (a.x - b.x).abs() < 0.1 || (a.y - b.y).abs() < 0.1 {
                b
            } else {
                vec2(b.x, a.y)
            };
            for (from, to) in [(a, corner), (corner, b)] {
                for k in 0..=50 {
                    let p = from.lerp(to, k as f32 / 50.0);
                    assert!(open(p), "{p} between {} and {}", pair[0].name, pair[1].name);
                }
            }
        }
        for h in &HALLS {
            let far = (h.center.abs() + h.half).max_element();
            assert!(far < HALF_SIZE - 10.0, "{} reaches too far", h.name);
        }
    }

    #[test]
    fn walls_close_in_the_floor() {
        // A point just outside every hall's edge is solid, and the middle
        // of every hall is clear.
        let r = 0.45;
        for h in &HALLS {
            assert!(!blocked(to_world(0, h.center), r), "{}", h.name);
        }
        assert!(blocked(to_world(3, vec2(0.0, -8.3)), r));
        assert!(blocked(to_world(3, vec2(-12.3, 36.0)), r));
        // You can't walk through a wall: pushing from inside a wall puts
        // you on the floor side or in the rock, never across.
        let out = resolve(to_world(0, vec2(-7.9, 0.0)), r);
        assert!(to_local(out).x > -8.0 + r - 0.05);
        assert!(walls().len() > 20);
        assert!(!torches().is_empty());
    }

    #[test]
    fn entrance_packs_and_stone_are_on_open_floor() {
        assert!(open(ENTRANCE) && open(EXIT_STONE));
        assert!(!blocked(to_world(0, ENTRANCE), 0.45));
        for p in &PACKS {
            assert!(open(p.center), "pack at {}", p.center);
            for k in 0..8 {
                let a = k as f32 * 0.785;
                let q = p.center + vec2(a.cos(), a.sin()) * p.radius;
                assert!(open(q), "pack at {} spills into rock", p.center);
                assert!(
                    !blocked(to_world(0, q), 0.5),
                    "pack at {} hits a wall",
                    p.center
                );
            }
            // Nobody waits right at the door.
            assert!(p.center.distance(ENTRANCE) > 25.0);
        }
        let bosses: Vec<MobKind> = PACKS
            .iter()
            .flat_map(|p| p.mobs.iter().map(|m| m.0))
            .filter(|k| k.template().elite)
            .collect();
        assert_eq!(bosses, [MobKind::StoneWarden, MobKind::SunkenKing]);
    }
}
