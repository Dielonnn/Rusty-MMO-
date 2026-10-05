//! The shape of the world: six zones (one starting area per race), their
//! terrain, bounds and a few geometry helpers.
//!
//! The zones sit side by side along X, `ZONE_SPACING` apart, and nobody can
//! walk between them: the waystones in each town carry players across (see
//! `dungeon`, which also places the Sunken Vault's copies far from them all). Each zone has its own layout (see `layout`), and is
//! also turned and mirrored differently. "Local" coordinates are in that shared layout, with the town at the
//! origin; "world" coordinates are where things really are.
//!
//! Both sides use the same terrain function, so the server can put mobs on
//! the ground and the client can render the same hills it walks on.
//!
//! Directions: `yaw` 0 faces +Z, and the forward vector is `(sin yaw, 0, cos yaw)`.

use glam::{Vec2, Vec3, vec2, vec3};
use serde::{Deserialize, Serialize};

use crate::data::Race;
use crate::dungeon;

/// Each zone spans `-WORLD_HALF_SIZE..WORLD_HALF_SIZE` around its center.
pub const WORLD_HALF_SIZE: f32 = 220.0;
/// Distance between zone centers along X.
pub const ZONE_SPACING: f32 = 1000.0;
/// The town around each zone's center is flat, safe and has no mobs.
pub const TOWN_RADIUS: f32 = 28.0;
/// Where players appear when they log in or release their spirit (local).
pub const GRAVEYARD: Vec2 = Vec2::new(0.0, -10.0);
/// Lakes fill everything below this height (in most zones).
pub const WATER_LEVEL: f32 = -4.6;
/// How far away other entities are sent to (and drawn by) a client.
pub const VIEW_DISTANCE: f32 = 110.0;

/// A starting area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Zone {
    /// Human: autumn hills at dusk.
    #[default]
    Amberfall,
    /// Orc: desert.
    Scorchsand,
    /// Elf: a regal forest at twilight.
    Silverbough,
    /// Goblin: a vast cave.
    Grubdeep,
    /// Gnome: snowy peaks.
    Frostcog,
    /// Undead: a dying forest.
    Witherwood,
}

impl Zone {
    pub const ALL: [Zone; 6] = [
        Zone::Amberfall,
        Zone::Scorchsand,
        Zone::Silverbough,
        Zone::Grubdeep,
        Zone::Frostcog,
        Zone::Witherwood,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Zone::Amberfall => "Amberfall Vale",
            Zone::Scorchsand => "Scorchsand Wastes",
            Zone::Silverbough => "Silverbough Glade",
            Zone::Grubdeep => "Grubdeep Caverns",
            Zone::Frostcog => "Frostcog Peaks",
            Zone::Witherwood => "Witherwood",
        }
    }

    pub fn town_name(self) -> &'static str {
        match self {
            Zone::Amberfall => "Hearthmere",
            Zone::Scorchsand => "Kragmaw Hold",
            Zone::Silverbough => "Aelthas",
            Zone::Grubdeep => "Rustpocket",
            Zone::Frostcog => "Gearhaven",
            Zone::Witherwood => "Gravenhold",
        }
    }

    /// Whose starting area this is.
    pub fn race(self) -> Race {
        match self {
            Zone::Amberfall => Race::Human,
            Zone::Scorchsand => Race::Orc,
            Zone::Silverbough => Race::Elf,
            Zone::Grubdeep => Race::Goblin,
            Zone::Frostcog => Race::Gnome,
            Zone::Witherwood => Race::Undead,
        }
    }

    /// The trader in this zone's town.
    pub fn merchant_name(self) -> &'static str {
        match self {
            Zone::Amberfall => "Tobin Hale",
            Zone::Scorchsand => "Joe",
            Zone::Silverbough => "Elarion",
            Zone::Grubdeep => "Migwick",
            Zone::Frostcog => "Nimble Cogsworth",
            Zone::Witherwood => "Mortimer Graves",
        }
    }

    pub fn subtitle(self) -> String {
        format!("{} starting area", self.race().name())
    }

    pub fn center(self) -> Vec2 {
        vec2(self.index() as f32 * ZONE_SPACING, 0.0)
    }

    /// The zone a world position is in (or nearest to).
    pub fn at(p: Vec3) -> Zone {
        let i = (p.x / ZONE_SPACING).round().clamp(0.0, 5.0) as usize;
        Zone::ALL[i]
    }

    /// How the shared layout is turned for this zone: quarter turns, and
    /// whether it's mirrored first.
    fn variant(self) -> (u8, bool) {
        match self {
            Zone::Amberfall => (0, false),
            Zone::Scorchsand => (1, false),
            Zone::Silverbough => (2, false),
            Zone::Grubdeep => (3, false),
            Zone::Frostcog => (0, true),
            Zone::Witherwood => (2, true),
        }
    }

    /// Local layout coordinates to a world position on the XZ plane.
    pub fn to_world(self, local: Vec2) -> Vec2 {
        let (turns, mirror) = self.variant();
        let l = if mirror {
            vec2(-local.x, local.y)
        } else {
            local
        };
        let a = turns as f32 * std::f32::consts::FRAC_PI_2;
        let (s, c) = a.sin_cos();
        self.center() + vec2(l.x * c + l.y * s, l.y * c - l.x * s)
    }

    /// World XZ position to local layout coordinates.
    pub fn to_local(self, world: Vec2) -> Vec2 {
        let (turns, mirror) = self.variant();
        let w = world - self.center();
        let a = -(turns as f32) * std::f32::consts::FRAC_PI_2;
        let (s, c) = a.sin_cos();
        let l = vec2(w.x * c + w.y * s, w.y * c - w.x * s);
        if mirror { vec2(-l.x, l.y) } else { l }
    }

    /// A facing in the local layout, as a world yaw.
    pub fn yaw_to_world(self, local_yaw: f32) -> f32 {
        let (turns, mirror) = self.variant();
        let y = if mirror { -local_yaw } else { local_yaw };
        y + turns as f32 * std::f32::consts::FRAC_PI_2
    }

    /// The point on the ground at a local position.
    pub fn ground_local(self, local: Vec2) -> Vec3 {
        let w = self.to_world(local);
        ground(w.x, w.y)
    }

    /// Where players of this zone appear.
    pub fn graveyard(self) -> Vec3 {
        self.ground_local(GRAVEYARD)
    }

    pub fn water_level(self) -> f32 {
        match self {
            // Only a few oases in the desert.
            Zone::Scorchsand => -5.8,
            _ => WATER_LEVEL,
        }
    }

    /// Whether a world position is in this zone's safe town.
    pub fn in_town(self, p: Vec3) -> bool {
        vec2(p.x, p.z).distance(self.center()) < TOWN_RADIUS
    }
}

impl Race {
    /// The starting area this race begins in.
    pub fn zone(self) -> Zone {
        match self {
            Race::Human => Zone::Amberfall,
            Race::Orc => Zone::Scorchsand,
            Race::Elf => Zone::Silverbough,
            Race::Goblin => Zone::Grubdeep,
            Race::Gnome => Zone::Frostcog,
            Race::Undead => Zone::Witherwood,
        }
    }
}

/// Whether a position is in any town.
pub fn in_town(p: Vec3) -> bool {
    Zone::at(p).in_town(p)
}

/// A pseudo-random value in `0..1` for a lattice point.
fn lattice(i: i32, j: i32, seed: u32) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x8DA6_B343)
        ^ (j as u32).wrapping_mul(0xD816_3841)
        ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Smooth value noise in `0..1`: random heights on a lattice, eased in
/// between. Unlike sums of sine waves it has no regular rows or columns.
pub fn value_noise(x: f32, z: f32, seed: u32) -> f32 {
    let (fx, fz) = (x.floor(), z.floor());
    let (i, j) = (fx as i32, fz as i32);
    let (tx, tz) = (x - fx, z - fz);
    let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let a = lattice(i, j, seed);
    let b = lattice(i + 1, j, seed);
    let c = lattice(i, j + 1, seed);
    let d = lattice(i + 1, j + 1, seed);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    top + (bottom - top) * sz
}

/// Several octaves of value noise, each turned a little so no direction
/// lines up. Returns `0..1`.
pub fn fbm(x: f32, z: f32, octaves: u32, seed: u32) -> f32 {
    let (mut x, mut z) = (x, z);
    let (mut sum, mut amp, mut total) = (0.0, 0.5, 0.0);
    for o in 0..octaves {
        sum += value_noise(x, z, seed.wrapping_add(o)) * amp;
        total += amp;
        // Rotate by about 37 degrees and double the frequency.
        let (nx, nz) = (x * 0.8 - z * 0.6, x * 0.6 + z * 0.8);
        x = nx * 2.03 + 17.1;
        z = nz * 2.03 - 9.7;
        amp *= 0.5;
    }
    sum / total
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Height of the ground at world `(x, z)`.
pub fn terrain_height(x: f32, z: f32) -> f32 {
    if dungeon::instance_at(vec3(x, 0.0, z)).is_some() {
        // The Sunken Vault has a flat stone floor.
        return 0.0;
    }
    let zone = Zone::at(vec3(x, 0.0, z));
    let l = zone.to_local(vec2(x, z));
    let mut h = crate::layout::terrain(zone).height(l.x, l.y);
    if zone == Zone::Scorchsand {
        // Wind-blown ripples on the dunes.
        let d = l.length();
        h += ((l.x * 0.35 + l.y * 0.12).sin() * 0.25)
            * smoothstep(TOWN_RADIUS, TOWN_RADIUS + 20.0, d);
    }
    h
}

/// The point on the ground under `(x, z)`.
pub fn ground(x: f32, z: f32) -> Vec3 {
    vec3(x, terrain_height(x, z), z)
}

/// Keeps a position inside its zone (or copy of the Sunken Vault) and not
/// below the ground.
pub fn clamp_to_world(pos: Vec3) -> Vec3 {
    if dungeon::instance_at(pos).is_some() {
        return dungeon::clamp(pos);
    }
    let zone = Zone::at(pos);
    let c = zone.center();
    let limit = WORLD_HALF_SIZE - 2.0;
    let x = pos.x.clamp(c.x - limit, c.x + limit);
    let z = pos.z.clamp(c.y - limit, c.y + limit);
    vec3(x, pos.y.max(terrain_height(x, z)), z)
}

/// A region you can't walk or leap out of: a starting area, or one copy of
/// the Sunken Vault.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Zone(Zone),
    Dungeon(u32),
}

impl Place {
    pub fn at(p: Vec3) -> Place {
        match dungeon::instance_at(p) {
            Some(i) => Place::Dungeon(i),
            None => Place::Zone(Zone::at(p)),
        }
    }

    /// Where the dead come back to life.
    pub fn graveyard(self) -> Vec3 {
        match self {
            Place::Zone(z) => z.graveyard(),
            Place::Dungeon(i) => dungeon::to_world(i, dungeon::ENTRANCE),
        }
    }
}

pub fn forward(yaw: f32) -> Vec3 {
    vec3(yaw.sin(), 0.0, yaw.cos())
}

/// The yaw that faces from `from` towards `to`.
pub fn yaw_towards(from: Vec3, to: Vec3) -> f32 {
    (to.x - from.x).atan2(to.z - from.z)
}

/// Distance on the XZ plane.
pub fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// Whether something at `from` looking along `yaw` has `to` within the
/// forward 180 degree arc. Things right on top of you always count.
pub fn is_facing(from: Vec3, yaw: f32, to: Vec3) -> bool {
    let dir = vec3(to.x - from.x, 0.0, to.z - from.z);
    if dir.length_squared() < 0.25 {
        return true;
    }
    forward(yaw).dot(dir.normalize()) >= -0.05
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn towns_are_flat() {
        for zone in Zone::ALL {
            for (x, z) in [
                (0.0, 0.0),
                (10.0, -15.0),
                (-20.0, 5.0),
                (GRAVEYARD.x, GRAVEYARD.y),
            ] {
                assert_eq!(zone.ground_local(vec2(x, z)).y, 0.0, "{zone:?}");
            }
        }
    }

    #[test]
    fn zone_transforms_round_trip() {
        for zone in Zone::ALL {
            for p in [vec2(10.0, -3.0), vec2(-150.0, 90.0)] {
                let w = zone.to_world(p);
                assert_eq!(Zone::at(vec3(w.x, 0.0, w.y)), zone);
                assert!(zone.to_local(w).distance(p) < 1e-3, "{zone:?}");
                // Heights follow the shared layout.
                assert!(
                    (terrain_height(w.x, w.y) - crate::layout::terrain(zone).height(p.x, p.y))
                        .abs()
                        < 0.3
                );
            }
            // Facing turns with the layout.
            let dir = zone.to_world(vec2(0.0, 1.0)) - zone.center();
            let f = forward(zone.yaw_to_world(0.0));
            assert!(dir.distance(vec2(f.x, f.z)) < 1e-4, "{zone:?}");
        }
        // The human zone is the original map.
        assert_eq!(Zone::Amberfall.to_world(vec2(5.0, 7.0)), vec2(5.0, 7.0));
    }

    #[test]
    fn lakes_are_small() {
        let mut wet = 0;
        let n = 100;
        for i in 0..n {
            for j in 0..n {
                let x = -WORLD_HALF_SIZE + 2.0 * WORLD_HALF_SIZE * i as f32 / n as f32;
                let z = -WORLD_HALF_SIZE + 2.0 * WORLD_HALF_SIZE * j as f32 / n as f32;
                if terrain_height(x, z) < WATER_LEVEL {
                    wet += 1;
                }
            }
        }
        assert!(
            wet < n * n / 10,
            "{wet} of {} samples are under water",
            n * n
        );
    }

    #[test]
    fn edges_are_raised() {
        assert!(terrain_height(WORLD_HALF_SIZE, 0.0) > 12.0);
    }

    #[test]
    fn facing() {
        let origin = Vec3::ZERO;
        assert!(is_facing(origin, 0.0, vec3(0.0, 0.0, 5.0)));
        assert!(!is_facing(origin, 0.0, vec3(0.0, 0.0, -5.0)));
        let yaw = yaw_towards(origin, vec3(3.0, 0.0, -4.0));
        assert!(forward(yaw).distance(vec3(0.6, 0.0, -0.8)) < 1e-5);
    }

    #[test]
    fn clamping_keeps_you_in_your_zone() {
        let p = clamp_to_world(vec3(400.0, -50.0, 0.0));
        assert!(p.x < WORLD_HALF_SIZE);
        assert_eq!(p.y, terrain_height(p.x, p.z));
        let q = clamp_to_world(vec3(ZONE_SPACING * 2.0 + 300.0, 0.0, -400.0));
        assert_eq!(Zone::at(q), Zone::Silverbough);
        assert!(q.x < ZONE_SPACING * 2.0 + WORLD_HALF_SIZE);
        // In the vault, you stay in your own copy, on its floor.
        let inside = dungeon::to_world(5, vec2(240.0, 30.0));
        let p = clamp_to_world(inside + vec3(0.0, -3.0, 0.0));
        assert_eq!(Place::at(p), Place::Dungeon(5));
        assert_eq!(p.y, 0.0);
        assert!(p.x < dungeon::center(5).x + dungeon::HALF_SIZE);
        assert_eq!(terrain_height(p.x, p.z), 0.0);
        assert_eq!(
            Place::at(Zone::Frostcog.graveyard()),
            Place::Zone(Zone::Frostcog)
        );
    }
}
