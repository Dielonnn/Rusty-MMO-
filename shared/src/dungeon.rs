//! The dungeons: halls of stone (or basalt, or ice) under the world, and the
//! waystones that carry players between the towns and into them.
//!
//! There are three: the Sunken Vault, the Cinderforge (a volcano held by
//! orcs) and Frosthowl Cavern (snow wolves and yetis). Every party (or
//! player on their own) who enters one gets a copy of their own, an
//! "instance". Each dungeon's copies sit side by side in a row far from the
//! starting areas (the vault's at `ROW_Z`, the others `ROW_GAP` further
//! on), `SPACING` apart, so nothing in one can see or reach anything in
//! another. An instance's index says both which dungeon it is and which
//! copy (`of`, `center`). Everything about a dungeon's shape is in
//! "dungeon" coordinates, with the entrance at the origin; `center` says
//! where an instance's origin really is.
//!
//! The halls are rectangles (`Dungeon::halls`) on flat ground, walled in
//! wherever open floor meets solid rock. Both sides build the same walls,
//! so the client draws what the server's mobs bump into.

use std::sync::OnceLock;

use glam::{Vec2, Vec3, vec2, vec3};
use serde::{Deserialize, Serialize};

use crate::data::MobKind;
use crate::props::{Colliders, Prop, PropKind};

/// Everything at or beyond this Z is inside some instance of a dungeon.
pub const ROW_Z: f32 = 6000.0;
/// Distance between neighbouring dungeons' rows along Z.
pub const ROW_GAP: f32 = 1000.0;
/// Distance between neighbouring instances along X.
pub const SPACING: f32 = 500.0;
/// How many copies of each dungeon can be open at once.
pub const MAX_INSTANCES: u32 = 64;
/// How far from its entrance an instance reaches, in every direction.
pub const HALF_SIZE: f32 = 200.0;
/// How tall the walls are; the ceiling sits on top of them.
pub const WALL_HEIGHT: f32 = 7.0;
/// How thick the walls are.
pub const WALL_THICKNESS: f32 = 1.0;

/// Where players arrive (and come back after dying), facing in. Every
/// dungeon starts with the same small room around the origin.
pub const ENTRANCE: Vec2 = Vec2::new(0.0, -2.0);
/// The waystone that takes you back out.
pub const EXIT_STONE: Vec2 = Vec2::new(0.0, -6.0);
/// Where the dungeon's quest giver stands, beside the way in.
pub const QUEST_GIVER: Vec2 = Vec2::new(4.0, -3.0);

/// An instance nobody is in is closed (and its mobs are gone) after this
/// many seconds.
pub const EMPTY_RESET: f32 = 300.0;

/// Which dungeon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DungeonId {
    /// Drowned cultists and Morvane the Sunken King. Level 8.
    SunkenVault,
    /// A volcano's orc war camp and Warlord Gorrak Ashfist. Level 15.
    Cinderforge,
    /// Snow wolves, yetis and Hrimja the Frostmother. Level 20.
    Frosthowl,
}

impl DungeonId {
    /// In row order, lowest level first.
    pub const ALL: [DungeonId; 3] = [
        DungeonId::SunkenVault,
        DungeonId::Cinderforge,
        DungeonId::Frosthowl,
    ];

    /// Its row, counting from the vault's.
    pub fn row(self) -> u32 {
        self as u32
    }

    pub fn get(self) -> &'static Dungeon {
        match self {
            DungeonId::SunkenVault => &SUNKEN_VAULT,
            DungeonId::Cinderforge => &CINDERFORGE,
            DungeonId::Frosthowl => &FROSTHOWL,
        }
    }

    /// The instance index of this dungeon's `copy`.
    pub fn instance(self, copy: u32) -> u32 {
        self.row() * MAX_INSTANCES + copy
    }
}

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

/// A group of mobs that fight together: kind, levels and how many, around
/// a spot in the dungeon.
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

/// Adds one decoration: kind, x, z, yaw, size, variant.
type Place<'a> = &'a mut dyn FnMut(PropKind, f32, f32, f32, f32, u8);

/// One dungeon: its rooms, its mobs and its scenery.
pub struct Dungeon {
    pub id: DungeonId,
    pub name: &'static str,
    /// You must be this level to enter.
    pub min_level: u8,
    /// The rooms and the corridors between them, in the order you go
    /// through them. Corridors have empty names. The last is the boss's.
    pub halls: &'static [Hall],
    /// Every mob in a fresh instance. Pull them one pack at a time.
    pub packs: &'static [Pack],
    /// The teleporter that opens in the boss's room once the boss is dead.
    /// It works like the waystone at the entrance.
    pub portal: Vec2,
    /// Barrels, pillars, braziers and the like (not the walls).
    decor: fn(Place),
    walls: OnceLock<Vec<Wall>>,
    props: OnceLock<Vec<Prop>>,
    colliders: OnceLock<Colliders>,
}

use MobKind::*;

pub static SUNKEN_VAULT: Dungeon = Dungeon {
    id: DungeonId::SunkenVault,
    name: "The Sunken Vault",
    min_level: 8,
    halls: &[
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
    ],
    packs: &[
        // Kennels
        pack(-6.0, 32.0, 3.0, &[(VaultHound, (10, 10), 2)]),
        pack(6.0, 42.0, 3.0, &[(VaultHound, (10, 10), 2)]),
        pack(
            0.0,
            44.0,
            2.0,
            &[(DrownedEnforcer, (10, 10), 1), (DrownedAdept, (10, 10), 1)],
        ),
        // Crawler Nest
        pack(36.0, 30.0, 4.0, &[(VaultCrawler, (10, 11), 3)]),
        pack(48.0, 42.0, 4.0, &[(VaultCrawler, (10, 11), 3)]),
        // Warden's Hall
        pack(
            38.0,
            78.0,
            2.0,
            &[(DrownedEnforcer, (10, 11), 1), (DrownedAdept, (10, 11), 1)],
        ),
        pack(42.0, 90.0, 1.0, &[(StoneWarden, (11, 11), 1)]),
        // Drowned Chapel
        pack(
            -10.0,
            80.0,
            3.0,
            &[(DrownedEnforcer, (11, 11), 2), (DrownedAdept, (11, 11), 1)],
        ),
        pack(-16.0, 92.0, 2.0, &[(DrownedAdept, (11, 11), 2)]),
        // Throne of the Deep
        pack(-10.0, 142.0, 1.0, &[(SunkenKing, (11, 11), 1)]),
    ],
    portal: Vec2::new(-10.0, 128.0),
    decor: vault_decor,
    walls: OnceLock::new(),
    props: OnceLock::new(),
    colliders: OnceLock::new(),
};

fn vault_decor(add: Place) {
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
    ring(add, PropKind::Pillar, vec2(-10.0, 134.0), 13.0, WALL_HEIGHT);
    for k in 0..6 {
        let a = k as f32 * 1.047;
        let p = vec2(-10.0, 140.0) + vec2(a.cos(), a.sin()) * 5.0;
        add(PropKind::RuneTile, p.x, p.y, a, 1.0, 0);
    }
    // Braziers in the chapel.
    for x in [-18.0, -2.0] {
        add(PropKind::Campfire, x, 76.0, 0.0, 1.0, 0);
    }
}

/// Eight of something in a ring around a boss's room, turned half a step
/// so none stands in the doorway.
fn ring(add: Place, kind: PropKind, center: Vec2, radius: f32, size: f32) {
    for k in 0..8 {
        let a = (k as f32 + 0.5) * std::f32::consts::TAU / 8.0;
        let p = center + vec2(a.cos(), a.sin()) * radius;
        add(kind, p.x, p.y, a, size, k as u8);
    }
}

pub static CINDERFORGE: Dungeon = Dungeon {
    id: DungeonId::Cinderforge,
    name: "The Cinderforge",
    min_level: 15,
    halls: &[
        hall("Scorched Gate", 0.0, 0.0, 8.0, 8.0),
        hall("", 0.0, 16.0, 3.0, 8.0),
        hall("War Camp", 0.0, 38.0, 14.0, 14.0),
        hall("", -24.0, 38.0, 10.0, 3.0),
        hall("Slag Pits", -48.0, 38.0, 14.0, 12.0),
        hall("", -48.0, 62.0, 3.0, 12.0),
        hall("Forge of Embers", -48.0, 88.0, 14.0, 14.0),
        hall("", -22.0, 88.0, 12.0, 3.0),
        hall("Ashen Barracks", 2.0, 88.0, 12.0, 12.0),
        hall("", 2.0, 110.0, 3.0, 10.0),
        hall("Caldera Throne", 2.0, 138.0, 18.0, 18.0),
    ],
    packs: &[
        // War Camp
        pack(-7.0, 32.0, 3.0, &[(AshHound, (15, 15), 2)]),
        pack(7.0, 44.0, 3.0, &[(CinderGrunt, (15, 15), 2)]),
        pack(
            -5.0,
            46.0,
            2.0,
            &[(CinderGrunt, (15, 15), 1), (CinderFirecaller, (15, 15), 1)],
        ),
        // Slag Pits
        pack(-54.0, 32.0, 4.0, &[(AshHound, (15, 16), 3)]),
        pack(
            -42.0,
            44.0,
            3.0,
            &[(CinderGrunt, (15, 16), 2), (CinderFirecaller, (15, 16), 1)],
        ),
        // Forge of Embers
        pack(
            -54.0,
            80.0,
            2.0,
            &[(CinderGrunt, (16, 16), 1), (CinderFirecaller, (16, 16), 1)],
        ),
        pack(-46.0, 94.0, 1.0, &[(MoltenColossus, (16, 16), 1)]),
        // Ashen Barracks
        pack(
            2.0,
            84.0,
            3.0,
            &[(CinderGrunt, (16, 16), 2), (CinderFirecaller, (16, 16), 1)],
        ),
        pack(-4.0, 95.0, 2.0, &[(CinderFirecaller, (16, 16), 2)]),
        // Caldera Throne
        pack(2.0, 146.0, 1.0, &[(GorrakAshfist, (16, 16), 1)]),
    ],
    portal: Vec2::new(2.0, 132.0),
    decor: cinderforge_decor,
    walls: OnceLock::new(),
    props: OnceLock::new(),
    colliders: OnceLock::new(),
};

fn cinderforge_decor(add: Place) {
    // Supplies piled at the gate.
    for (x, z) in [(-6.0, -5.5), (6.5, 5.5)] {
        add(PropKind::Barrel, x, z, 0.0, 1.0, 0);
    }
    add(PropKind::Crate, -6.0, 5.5, 0.4, 0.55, 0);
    // The war camp: tents around a cooking fire.
    add(PropKind::Tent, 10.0, 29.0, -0.6, 1.0, 0);
    add(PropKind::Tent, -10.0, 49.0, 2.5, 1.0, 1);
    add(PropKind::Campfire, 5.0, 34.0, 0.0, 1.0, 0);
    // Slag heaps and old bones in the pits.
    for (i, (x, z)) in [(-60.0, 46.0), (-36.0, 28.0), (-60.0, 28.0)]
        .into_iter()
        .enumerate()
    {
        add(PropKind::Stalagmite, x, z, i as f32, 1.4, i as u8);
    }
    for (i, (x, z)) in [(-50.0, 46.0), (-38.0, 34.0), (6.0, 80.0), (-8.0, 84.0)]
        .into_iter()
        .enumerate()
    {
        add(PropKind::Bones, x, z, i as f32 * 1.3, 1.0, i as u8);
    }
    // The forge: fires in every corner, and stacked ingots and ore.
    for x in [-59.0, -37.0] {
        for z in [77.0, 99.0] {
            add(PropKind::Campfire, x, z, 0.0, 1.0, 0);
        }
    }
    add(PropKind::Crate, -40.0, 84.0, 0.2, 0.6, 0);
    add(PropKind::Barrel, -41.0, 82.0, 0.0, 1.0, 0);
    // Bunks in the barracks.
    add(PropKind::Crate, 11.0, 79.0, 0.0, 0.55, 0);
    add(PropKind::Crate, 11.0, 97.0, 0.0, 0.55, 0);
    // Basalt pillars and braziers around the warlord's throne.
    ring(add, PropKind::Pillar, vec2(2.0, 138.0), 13.0, WALL_HEIGHT);
    for x in [-6.0, 10.0] {
        add(PropKind::Campfire, x, 150.0, 0.0, 1.0, 0);
    }
}

pub static FROSTHOWL: Dungeon = Dungeon {
    id: DungeonId::Frosthowl,
    name: "Frosthowl Cavern",
    min_level: 20,
    halls: &[
        hall("Icefall Entry", 0.0, 0.0, 8.0, 8.0),
        hall("", 0.0, 18.0, 3.0, 10.0),
        hall("Howling Den", 0.0, 40.0, 12.0, 12.0),
        hall("", 22.0, 40.0, 10.0, 3.0),
        hall("Frozen Falls", 46.0, 40.0, 14.0, 14.0),
        hall("", 46.0, 66.0, 3.0, 12.0),
        hall("Yeti Hollow", 46.0, 92.0, 14.0, 14.0),
        hall("", 20.0, 92.0, 12.0, 3.0),
        hall("Glacier Gallery", -4.0, 92.0, 12.0, 12.0),
        hall("", -4.0, 114.0, 3.0, 10.0),
        hall("Frostmother's Throne", -4.0, 142.0, 18.0, 18.0),
    ],
    packs: &[
        // Howling Den
        pack(-6.0, 36.0, 3.0, &[(FrostfangWolf, (20, 20), 2)]),
        pack(6.0, 46.0, 3.0, &[(FrostfangWolf, (20, 20), 3)]),
        // Frozen Falls
        pack(40.0, 33.0, 2.0, &[(CavernYeti, (20, 20), 1)]),
        pack(52.0, 47.0, 3.0, &[(FrostfangWolf, (20, 21), 3)]),
        // Yeti Hollow
        pack(
            40.0,
            86.0,
            3.0,
            &[(CavernYeti, (21, 21), 1), (FrostfangWolf, (20, 21), 2)],
        ),
        pack(48.0, 100.0, 1.0, &[(ElderYeti, (21, 21), 1)]),
        // Glacier Gallery
        pack(-4.0, 86.0, 3.0, &[(FrostfangWolf, (21, 21), 3)]),
        pack(-9.0, 99.0, 2.0, &[(CavernYeti, (21, 21), 1)]),
        // Frostmother's Throne
        pack(-4.0, 150.0, 1.0, &[(Hrimja, (21, 21), 1)]),
    ],
    portal: Vec2::new(-4.0, 136.0),
    decor: frosthowl_decor,
    walls: OnceLock::new(),
    props: OnceLock::new(),
    colliders: OnceLock::new(),
};

fn frosthowl_decor(add: Place) {
    // Icicles and ice crystals along the walls.
    for (i, (x, z)) in [
        (-6.5, 6.5),
        (6.5, 6.0),
        (-10.5, 50.5),
        (10.5, 30.0),
        (58.0, 28.0),
        (34.0, 52.0),
        (58.0, 104.0),
        (34.0, 80.0),
        (-14.5, 82.0),
        (6.5, 102.0),
    ]
    .into_iter()
    .enumerate()
    {
        let kind = if i % 2 == 0 {
            PropKind::Crystal
        } else {
            PropKind::Stalagmite
        };
        // Odd variants are the ice-blue crystals.
        add(kind, x, z, i as f32 * 0.9, 1.3, 1);
    }
    // Gnawed bones in the dens.
    for (i, (x, z)) in [
        (-8.0, 44.0),
        (4.0, 32.0),
        (52.0, 36.0),
        (40.0, 98.0),
        (54.0, 88.0),
    ]
    .into_iter()
    .enumerate()
    {
        add(PropKind::Bones, x, z, i as f32 * 1.3, 1.0, i as u8);
    }
    // Boulders fallen at the falls.
    add(PropKind::Rock, 57.0, 40.0, 0.3, 1.2, 0);
    add(PropKind::Rock, 36.0, 42.0, 1.1, 0.9, 1);
    // A ring of great ice crystals around the Frostmother.
    ring(
        &mut |kind, x, z, yaw, size, _| add(kind, x, z, yaw, size, 1),
        PropKind::Crystal,
        vec2(-4.0, 142.0),
        13.0,
        2.2,
    );
}

/// Which dungeon instance `index` is a copy of.
pub fn of(index: u32) -> &'static Dungeon {
    let row = (index / MAX_INSTANCES).min(DungeonId::ALL.len() as u32 - 1);
    DungeonId::ALL[row as usize].get()
}

/// Where instance `index`'s entrance really is, on the XZ plane.
pub fn center(index: u32) -> Vec2 {
    let (row, copy) = (index / MAX_INSTANCES, index % MAX_INSTANCES);
    vec2(copy as f32 * SPACING, ROW_Z + row as f32 * ROW_GAP)
}

/// Which instance of a dungeon a world position is in, if any.
pub fn instance_at(p: Vec3) -> Option<u32> {
    (p.z > ROW_Z - HALF_SIZE * 2.0).then(|| {
        let row = ((p.z - ROW_Z) / ROW_GAP)
            .round()
            .clamp(0.0, (DungeonId::ALL.len() - 1) as f32) as u32;
        let copy = (p.x / SPACING)
            .round()
            .clamp(0.0, (MAX_INSTANCES - 1) as f32) as u32;
        row * MAX_INSTANCES + copy
    })
}

/// The dungeon a world position is in, if any.
pub fn at(p: Vec3) -> Option<&'static Dungeon> {
    instance_at(p).map(of)
}

/// Dungeon coordinates in instance `index` to a world position on the
/// floor.
pub fn to_world(index: u32, local: Vec2) -> Vec3 {
    let c = center(index) + local;
    vec3(c.x, 0.0, c.y)
}

/// A world position (in any instance) to dungeon coordinates.
pub fn to_local(p: Vec3) -> Vec2 {
    let c = center(instance_at(p).unwrap_or(0));
    vec2(p.x - c.x, p.z - c.y)
}

impl Dungeon {
    /// Whether a spot in dungeon coordinates is open floor.
    pub fn open(&self, local: Vec2) -> bool {
        self.halls.iter().any(|h| {
            let d = (local - h.center).abs();
            d.x <= h.half.x && d.y <= h.half.y
        })
    }

    /// The named room a spot is in, if any.
    pub fn room_at(&self, local: Vec2) -> Option<&'static str> {
        self.halls
            .iter()
            .filter(|h| !h.name.is_empty())
            .find(|h| {
                let d = (local - h.center).abs();
                d.x < h.half.x && d.y < h.half.y
            })
            .map(|h| h.name)
    }

    /// The corners of the box around every hall, in whole meters.
    fn bounds(&self) -> (i32, i32, i32, i32) {
        let mut b = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for h in self.halls {
            let lo = h.center - h.half;
            let hi = h.center + h.half;
            b.0 = b.0.min(lo.x.floor() as i32 - 1);
            b.1 = b.1.min(lo.y.floor() as i32 - 1);
            b.2 = b.2.max(hi.x.ceil() as i32 + 1);
            b.3 = b.3.max(hi.y.ceil() as i32 + 1);
        }
        b
    }

    /// Every wall, found on a one meter grid wherever open floor meets
    /// rock, with neighbouring pieces merged into long runs.
    pub fn walls(&self) -> &[Wall] {
        self.walls.get_or_init(|| {
            let (x0, z0, x1, z1) = self.bounds();
            let cell = |i: i32, j: i32| self.open(vec2(i as f32 + 0.5, j as f32 + 0.5));
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

    /// Wall torches: where each one hangs (dungeon coordinates) and which
    /// way it faces.
    pub fn torches(&self) -> Vec<(Vec3, Vec2)> {
        let mut out = Vec::new();
        for w in self.walls() {
            let n = (w.length() / 9.0).floor() as i32;
            for k in 0..n {
                let t = (k as f32 + 0.5) / n as f32;
                let p = w.from.lerp(w.to, t) + w.inward * 0.35;
                out.push((vec3(p.x, 2.8, p.y), w.inward));
            }
        }
        out
    }

    /// Everything solid or decorative in the dungeon, in dungeon
    /// coordinates (instance 0's world position minus its center).
    fn local_props(&self) -> &[Prop] {
        self.props.get_or_init(|| {
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
            for w in self.walls() {
                // Thick blocks behind the line, a little longer than the run
                // so the corners close.
                let mid = (w.from + w.to) * 0.5 - w.inward * WALL_THICKNESS * 0.5;
                let along = (w.to - w.from).normalize();
                let yaw = along.x.atan2(along.y);
                let half = w.length() * 0.5 + WALL_THICKNESS * 0.5;
                add(PropKind::VaultWall, mid.x, mid.y, yaw, half, 0);
            }
            add(PropKind::Waystone, EXIT_STONE.x, EXIT_STONE.y, 0.0, 1.0, 0);
            (self.decor)(&mut add);
            props
        })
    }

    /// What you bump into, in dungeon coordinates.
    fn colliders(&self) -> &Colliders {
        self.colliders
            .get_or_init(|| Colliders::new(self.local_props()))
    }

    /// The boss: the one in the last pack.
    pub fn boss(&self) -> MobKind {
        self.packs.last().unwrap().mobs[0].0
    }
}

/// One straight piece of wall: its ends (dungeon coordinates, on the line
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

/// Everything in instance `index`, in world coordinates.
pub fn props(index: u32) -> Vec<Prop> {
    let c = center(index);
    of(index)
        .local_props()
        .iter()
        .map(|p| Prop {
            pos: p.pos + vec3(c.x, 0.0, c.y),
            ..*p
        })
        .collect()
}

/// Pushes a circle at a world position (in any instance) out of the walls.
pub fn resolve(pos: Vec3, radius: f32) -> Vec3 {
    let index = instance_at(pos).unwrap_or(0);
    let c = center(index);
    let offset = vec3(c.x, 0.0, c.y);
    of(index).colliders().resolve(pos - offset, radius) + offset
}

/// Whether a circle at a world position overlaps a wall or anything solid.
pub fn blocked(pos: Vec3, radius: f32) -> bool {
    let index = instance_at(pos).unwrap_or(0);
    let c = center(index);
    of(index)
        .colliders()
        .blocked(pos - vec3(c.x, 0.0, c.y), radius)
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
        for d in DungeonId::ALL {
            for copy in [0, 1, 17, MAX_INSTANCES - 1] {
                let i = d.instance(copy);
                let p = to_world(i, vec2(30.0, 140.0));
                assert_eq!(instance_at(p), Some(i));
                assert_eq!(of(i).id, d);
                assert!(to_local(p).distance(vec2(30.0, 140.0)) < 1e-3);
                // The far corners stay in the same instance.
                for corner in [vec2(-190.0, -190.0), vec2(190.0, 190.0)] {
                    assert_eq!(instance_at(to_world(i, corner)), Some(i));
                }
            }
        }
        // Nothing in one instance is within sight of the next, in its row
        // or the next row.
        const { assert!(SPACING - 2.0 * HALF_SIZE > crate::world::VIEW_DISTANCE * 0.5) };
        const { assert!(ROW_GAP - 2.0 * HALF_SIZE > crate::world::VIEW_DISTANCE * 0.5) };
        for zone in crate::world::Zone::ALL {
            assert_eq!(instance_at(zone.graveyard()), None);
        }
    }

    #[test]
    fn halls_join_up_and_fit() {
        for d in DungeonId::ALL.map(DungeonId::get) {
            // Walk from each hall's center to the next one's: every step is
            // on open floor, through the corridor between them.
            for pair in d.halls.windows(2) {
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
                        assert!(
                            d.open(p),
                            "{}: {p} between {} and {}",
                            d.name,
                            pair[0].name,
                            pair[1].name
                        );
                    }
                }
            }
            for h in d.halls {
                let far = (h.center.abs() + h.half).max_element();
                assert!(far < HALF_SIZE - 10.0, "{} reaches too far", h.name);
            }
            // Every dungeon starts with the same little room.
            assert_eq!(d.halls[0].center, Vec2::ZERO);
            assert_eq!(d.halls[0].half, vec2(8.0, 8.0));
        }
    }

    #[test]
    fn walls_close_in_the_floor() {
        let r = 0.45;
        for id in DungeonId::ALL {
            let d = id.get();
            let i = id.instance(3);
            // The middle of every hall is clear, and a point just outside
            // the first room is solid.
            for h in d.halls {
                assert!(!blocked(to_world(i, h.center), r), "{}", h.name);
            }
            assert!(blocked(to_world(i, vec2(0.0, -8.3)), r));
            assert!(blocked(to_world(i, vec2(-8.3, 0.0)), r));
            // You can't walk through a wall: pushing from inside a wall puts
            // you on the floor side or in the rock, never across.
            let out = resolve(to_world(i, vec2(-7.9, 0.0)), r);
            assert!(to_local(out).x > -8.0 + r - 0.05);
            assert!(d.walls().len() > 20);
            assert!(!d.torches().is_empty());
        }
        // A wall of one dungeon isn't in another.
        assert!(blocked(to_world(0, vec2(-12.3, 36.0)), r));
        assert!(!blocked(
            to_world(DungeonId::Cinderforge.instance(0), vec2(-12.3, 36.0)),
            r
        ));
    }

    #[test]
    fn entrance_packs_and_stone_are_on_open_floor() {
        for d in DungeonId::ALL.map(DungeonId::get) {
            let i = d.id.instance(0);
            assert!(d.open(ENTRANCE) && d.open(EXIT_STONE) && d.open(QUEST_GIVER));
            assert!(d.open(d.portal) && !blocked(to_world(i, d.portal), 0.8));
            assert!(!blocked(to_world(i, ENTRANCE), 0.45));
            assert!(!blocked(to_world(i, QUEST_GIVER), 0.45));
            for p in d.packs {
                assert!(d.open(p.center), "{}: pack at {}", d.name, p.center);
                for k in 0..8 {
                    let a = k as f32 * 0.785;
                    let q = p.center + vec2(a.cos(), a.sin()) * p.radius;
                    assert!(
                        d.open(q),
                        "{}: pack at {} spills into rock",
                        d.name,
                        p.center
                    );
                    assert!(
                        !blocked(to_world(i, q), 0.5),
                        "{}: pack at {} hits something",
                        d.name,
                        p.center
                    );
                }
                // Nobody waits right at the door.
                assert!(p.center.distance(ENTRANCE) > 25.0);
            }
            // One elite partway through, and the boss at the end, in the
            // last room.
            let elites: Vec<MobKind> = d
                .packs
                .iter()
                .flat_map(|p| p.mobs.iter().map(|m| m.0))
                .filter(|k| k.template().elite)
                .collect();
            assert_eq!(elites.len(), 2, "{}", d.name);
            assert!(elites[1].template().boss && !elites[0].template().boss);
            assert_eq!(d.boss(), elites[1]);
            let last = d.halls.last().unwrap();
            assert_eq!(d.room_at(d.packs.last().unwrap().center), Some(last.name));
            assert_eq!(d.room_at(d.portal), Some(last.name));
            // Every mob is a dungeon mob.
            for p in d.packs {
                for &(kind, levels, _) in p.mobs {
                    assert!(MobKind::DUNGEON.contains(&kind), "{kind:?}");
                    assert!(levels.0 >= d.min_level && levels.0 <= levels.1);
                }
            }
        }
        assert_eq!(
            DungeonId::ALL.map(|d| d.get().boss()),
            [MobKind::SunkenKing, MobKind::GorrakAshfist, MobKind::Hrimja]
        );
    }
}
