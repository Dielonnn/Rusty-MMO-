//! The shape of the world: six starting areas (one per race), two
//! connecting zones between them, their terrain, bounds and a few geometry
//! helpers.
//!
//! The zones sit side by side along X, `ZONE_SPACING` apart. The waystones in
//! each town carry players between towns (see `dungeon`, which also places
//! the dungeons' copies far from them all), and mountain passes at the edges
//! of the map lead from three starting areas into the connecting zone
//! between them (see `Pass`). Each zone has its own layout (see `layout`), and is
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
    /// Level 13-20 highlands joining Amberfall, Silverbough and Frostcog.
    Sunfold,
    /// Level 13-20 badlands joining Witherwood, Scorchsand and Grubdeep.
    Blightscar,
}

impl Zone {
    pub const ALL: [Zone; 8] = [
        Zone::Amberfall,
        Zone::Scorchsand,
        Zone::Silverbough,
        Zone::Grubdeep,
        Zone::Frostcog,
        Zone::Witherwood,
        Zone::Sunfold,
        Zone::Blightscar,
    ];

    /// The zones between starting areas.
    pub const CONNECTING: [Zone; 2] = [Zone::Sunfold, Zone::Blightscar];

    /// Whether this is a zone between starting areas, not a race's own.
    pub fn connecting(self) -> bool {
        Self::CONNECTING.contains(&self)
    }

    /// The starting area whose scenery (trees, houses, rocks) this zone
    /// borrows. Starting areas are their own.
    pub fn style(self) -> Zone {
        match self {
            Zone::Sunfold => Zone::Amberfall,
            Zone::Blightscar => Zone::Scorchsand,
            z => z,
        }
    }

    /// The levels of the zone's mobs.
    pub fn levels(self) -> (u8, u8) {
        if self.connecting() { (13, 20) } else { (1, 10) }
    }

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
            Zone::Sunfold => "Sunfold Highlands",
            Zone::Blightscar => "Blightscar Badlands",
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
            Zone::Sunfold => "Three Banners",
            Zone::Blightscar => "Bonecross",
        }
    }

    /// Whose starting area this is. A connecting zone counts as its
    /// first neighbour's.
    pub fn race(self) -> Race {
        match self {
            Zone::Sunfold => Race::Human,
            Zone::Blightscar => Race::Undead,
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
            Zone::Sunfold => "Quartermaster Hollis",
            Zone::Blightscar => "Grizzle Rotfang",
        }
    }

    pub fn subtitle(self) -> String {
        match self {
            Zone::Sunfold => "Human, Elf and Gnome lands, level 13-20".into(),
            Zone::Blightscar => "Undead, Orc and Goblin lands, level 13-20".into(),
            _ => format!("{} starting area", self.race().name()),
        }
    }

    pub fn center(self) -> Vec2 {
        vec2(self.index() as f32 * ZONE_SPACING, 0.0)
    }

    /// The zone a world position is in (or nearest to).
    pub fn at(p: Vec3) -> Zone {
        let i = (p.x / ZONE_SPACING)
            .round()
            .clamp(0.0, (Zone::ALL.len() - 1) as f32) as usize;
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
            Zone::Sunfold => (1, true),
            Zone::Blightscar => (3, true),
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
            Zone::Scorchsand | Zone::Blightscar => -5.8,
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
            Race::Gnome | Race::Dwarf => Zone::Frostcog,
            Race::Undead => Zone::Witherwood,
        }
    }
}

/// A mountain pass at the edge of a zone, at the end of one of the roads
/// out of town: walk to its far end and you come out of the matching pass
/// in the zone it leads to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pass {
    pub zone: Zone,
    /// Which way the road runs from town, in local coordinates.
    pub dir: Vec2,
    pub to: Zone,
}

/// Half the width of a pass's road. The valley around it is wider.
pub const PASS_HALF_WIDTH: f32 = 6.0;
/// How far from town (along the road) the pass's gate stands.
pub const PASS_GATE: f32 = 192.0;
/// Walk this far down the pass and you cross into the next zone.
pub const PASS_END: f32 = 204.0;
/// Where travelers coming the other way arrive, facing into the zone.
pub const PASS_ARRIVAL: f32 = 182.0;

const fn pass(zone: Zone, x: f32, z: f32, to: Zone) -> Pass {
    Pass {
        zone,
        dir: Vec2::new(x, z),
        to,
    }
}

/// Every pass: one in each starting area it joins, three in each
/// connecting zone (one per neighbour).
pub const PASSES: [Pass; 12] = [
    pass(Zone::Amberfall, 0.0, 1.0, Zone::Sunfold),
    pass(Zone::Silverbough, 0.0, -1.0, Zone::Sunfold),
    pass(Zone::Frostcog, -1.0, 0.0, Zone::Sunfold),
    pass(Zone::Sunfold, 0.0, -1.0, Zone::Amberfall),
    pass(Zone::Sunfold, 1.0, 0.0, Zone::Silverbough),
    pass(Zone::Sunfold, -1.0, 0.0, Zone::Frostcog),
    pass(Zone::Witherwood, 1.0, 0.0, Zone::Blightscar),
    pass(Zone::Scorchsand, 0.0, 1.0, Zone::Blightscar),
    pass(Zone::Grubdeep, -1.0, 0.0, Zone::Blightscar),
    pass(Zone::Blightscar, 0.0, 1.0, Zone::Witherwood),
    pass(Zone::Blightscar, 1.0, 0.0, Zone::Scorchsand),
    pass(Zone::Blightscar, -1.0, 0.0, Zone::Grubdeep),
];

impl Zone {
    /// The passes out of this zone.
    pub fn passes(self) -> impl Iterator<Item = &'static Pass> {
        PASSES.iter().filter(move |p| p.zone == self)
    }
}

impl Pass {
    /// How far along the road out of town, and how far to the side of it,
    /// a local position is.
    pub fn along_across(&self, local: Vec2) -> (f32, f32) {
        (local.dot(self.dir), local.perp_dot(self.dir).abs())
    }

    /// How much of the pass's valley a local position is in: 1 on its
    /// road, 0 away from it.
    pub fn valley(&self, local: Vec2) -> f32 {
        let (along, across) = self.along_across(local);
        smoothstep(PASS_HALF_WIDTH + 12.0, PASS_HALF_WIDTH + 2.0, across)
            * smoothstep(60.0, 140.0, along)
    }

    /// A local position on the pass's road, `along` from town.
    pub fn spot(&self, along: f32) -> Vec2 {
        self.dir * along
    }

    /// The pass on the other side, that leads back here.
    pub fn other_end(&self) -> &'static Pass {
        self.to
            .passes()
            .find(|p| p.to == self.zone)
            .expect("every pass has a way back")
    }

    /// Where someone crossing this pass comes out, and which way they face.
    pub fn exit(&self) -> (Vec3, f32) {
        let other = self.other_end();
        let pos = other.zone.ground_local(other.spot(PASS_ARRIVAL));
        let toward = other.zone.ground_local(other.spot(PASS_ARRIVAL - 10.0));
        (pos, yaw_towards(pos, toward))
    }
}

/// The pass whose far end a world position has walked into, if any.
pub fn pass_at(p: Vec3) -> Option<&'static Pass> {
    if dungeon::instance_at(p).is_some() {
        return None;
    }
    let zone = Zone::at(p);
    let local = zone.to_local(vec2(p.x, p.z));
    zone.passes().find(|pass| {
        let (along, across) = pass.along_across(local);
        along >= PASS_END && across < PASS_HALF_WIDTH + 4.0
    })
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

    #[test]
    fn passes_are_open_roads_both_ways() {
        for zone in Zone::ALL {
            let n = zone.passes().count();
            assert_eq!(n, if zone.connecting() { 3 } else { 1 }, "{zone:?}");
            // `layout` hands each zone a slice of its own passes.
            assert_eq!(crate::layout::terrain(zone).passes.len(), n);
            assert!(
                crate::layout::terrain(zone)
                    .passes
                    .iter()
                    .all(|p| p.zone == zone)
            );
        }
        let colliders: Vec<_> = Zone::ALL
            .iter()
            .map(|z| crate::props::Colliders::for_zone(*z))
            .collect();
        for pass in &PASSES {
            assert_eq!(pass.other_end().other_end(), pass);
            assert!(pass.to.connecting() != pass.zone.connecting());
            // From the edge of town to the end, the road is dry, and once
            // it's out in the wilds it's open and never too steep.
            let mut last = pass.zone.ground_local(pass.spot(TOWN_RADIUS));
            let mut along = TOWN_RADIUS + 2.0;
            while along <= PASS_END + 2.0 {
                for side in [-PASS_HALF_WIDTH + 1.0, 0.0, PASS_HALF_WIDTH - 1.0] {
                    let local = pass.spot(along) + pass.dir.perp() * side;
                    let p = pass.zone.ground_local(local);
                    assert!(
                        p.y > pass.zone.water_level() + 0.3,
                        "{pass:?} wet at {along}"
                    );
                    // (Near town it's the old road, which can wind past a
                    // camp.)
                    assert!(
                        along < 90.0 || !colliders[pass.zone.index()].blocked(p, 0.4),
                        "{pass:?} blocked at {along} {side}"
                    );
                }
                let p = pass.zone.ground_local(pass.spot(along));
                assert!(
                    along < 90.0 || (p.y - last.y).abs() < 1.2,
                    "{pass:?} too steep at {along}: {} to {}",
                    last.y,
                    p.y
                );
                last = p;
                along += 2.0;
            }
            // Walls of mountains on both sides near the end.
            let wall = pass
                .zone
                .ground_local(pass.spot(PASS_END) + pass.dir.perp() * 30.0);
            assert!(wall.y > last.y + 8.0, "{pass:?} has no walls");
        }
    }
}
