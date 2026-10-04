//! Where every building, tree and rock stands, and what you bump into.
//!
//! Placement is deterministic, so the client draws the same scenery the
//! collision checks use. Props are laid out in each zone's local coordinates
//! (see `world` and `layout`) and returned in world coordinates.

use std::collections::HashMap;

use glam::{Vec2, Vec3, vec2};

use crate::layout::SiteKind;
use crate::world::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    // Town
    House,
    Stall,
    /// The town's centerpiece: a fountain, totem, moonwell...
    Centerpiece,
    Grave,
    Lamp,
    Barrel,
    Crate,
    Well,
    Cart,
    /// Points the way out of town.
    Signpost,
    /// A tall pole with a flag in the zone's colors.
    Banner,
    // Wilds
    /// The zone's signature tree.
    Tree,
    Conifer,
    DeadTree,
    Rock,
    Shrub,
    Mushrooms,
    Cactus,
    Mesa,
    Stalagmite,
    Crystal,
    Tombstone,
    Bones,
    Flowers,
    /// A fallen log.
    Log,
    // Farms
    /// `variant` 0 or 1 picks which of the two fields it grows in.
    Crop,
    HayBale,
    Scarecrow,
    /// `size` is half its length.
    Fence,
    // Camps
    Tent,
    Stake,
    Campfire,
    /// `size` is its height.
    Pillar,
    RuneTile,
}

#[derive(Clone, Copy, Debug)]
pub struct Prop {
    pub kind: PropKind,
    /// World position, on the ground.
    pub pos: Vec3,
    pub yaw: f32,
    pub size: f32,
    /// Varies looks between props of the same kind.
    pub variant: u8,
}

/// A footprint on the ground: a circle, or a box with half extents along the
/// prop's right and forward directions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Circle(f32),
    Box(f32, f32),
}

impl Prop {
    /// What you bump into, if anything.
    pub fn footprint(&self) -> Option<Shape> {
        use PropKind::*;
        let s = self.size;
        Some(match self.kind {
            House => Shape::Box(3.2, 2.6),
            Stall => Shape::Box(1.45, 0.65),
            Centerpiece => Shape::Circle(3.2),
            Grave => Shape::Box(0.15, 0.5),
            Lamp | Scarecrow => Shape::Circle(0.22),
            Barrel => Shape::Circle(0.45),
            Crate => Shape::Circle(s * 1.1),
            Tree => Shape::Circle(0.35 * s),
            Conifer | DeadTree => Shape::Circle(0.3 * s),
            Rock => Shape::Circle(1.0 * s),
            Cactus => Shape::Circle(0.45 * s),
            Mesa => Shape::Circle(3.5 * s),
            Stalagmite => Shape::Circle(0.6 * s),
            Crystal => Shape::Circle(0.5 * s),
            Tombstone => Shape::Box(0.4, 0.15),
            HayBale => Shape::Circle(0.7),
            Fence => Shape::Box(0.1, s),
            Tent => Shape::Circle(2.0),
            Stake => Shape::Circle(0.25),
            Campfire => Shape::Circle(1.0),
            Pillar => Shape::Circle(0.85),
            Well => Shape::Circle(1.3),
            Cart => Shape::Box(0.9, 1.6),
            Signpost | Banner => Shape::Circle(0.2),
            Log => Shape::Box(0.4, 1.6 * s),
            Shrub | Mushrooms | Bones | Crop | RuneTile | Flowers => return None,
        })
    }
}

/// Where the merchant stands in every town (local coordinates).
pub const MERCHANT_SPOT: Vec2 = Vec2::new(6.4, 4.8);
/// Where the quest giver stands.
pub const QUEST_SPOT: Vec2 = Vec2::new(-5.0, -4.5);

/// A tiny deterministic random number generator for placing scenery.
pub struct Scatter(pub u32);

impl Scatter {
    pub fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 & 0xFFFFFF) as f32 / 0x1000000 as f32
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}

/// Places in a zone that should stay clear of random scenery (local
/// coordinates).
pub fn near_landmark(zone: Zone, x: f32, z: f32) -> bool {
    let p = vec2(x, z);
    let d = p.length();
    let l = zone.layout();
    d < TOWN_RADIUS + 8.0
        || x.abs() < 5.0 && d < 90.0
        || z.abs() < 5.0 && d < 90.0
        || l.sites.iter().any(|s| match s.kind {
            SiteKind::Camp => s.center.distance(p) < 14.0,
            SiteKind::Ruins => s.center.distance(p) < 16.0,
            SiteKind::Beasts => false,
        })
        || l.fields.iter().any(|f| f.distance(p) < 15.0)
}

struct Builder {
    zone: Zone,
    props: Vec<Prop>,
}

impl Builder {
    fn add(&mut self, kind: PropKind, local: Vec2, local_yaw: f32, size: f32, variant: u8) {
        self.props.push(Prop {
            kind,
            pos: self.zone.ground_local(local),
            yaw: self.zone.yaw_to_world(local_yaw),
            size,
            variant,
        });
    }

    fn fence(&mut self, from: Vec2, to: Vec2) {
        let mid = (from + to) * 0.5;
        let yaw = (to.x - from.x).atan2(to.y - from.y);
        self.add(PropKind::Fence, mid, yaw, from.distance(to) * 0.5, 0);
    }
}

/// Every prop in a zone.
pub fn props(zone: Zone) -> Vec<Prop> {
    use PropKind::*;
    let mut b = Builder {
        zone,
        props: Vec::new(),
    };

    let layout = zone.layout();

    // The town.
    for (i, h) in layout.houses.iter().enumerate() {
        b.add(House, h.pos, h.yaw, 1.0, i as u8);
    }
    b.add(Centerpiece, Vec2::ZERO, 0.0, 1.0, 0);
    b.add(Well, layout.well, 0.3, 1.0, 0);
    b.add(Cart, vec2(-10.5, 9.5), 0.9, 1.0, 0);
    // Signposts and banners where the roads leave town.
    for (k, (x, z)) in [(0.0, 31.0), (31.0, 0.0), (0.0, -31.0), (-31.0, 0.0)]
        .into_iter()
        .enumerate()
    {
        let p = vec2(x, z);
        let side = vec2(-z, x).normalize() * 4.0;
        b.add(Signpost, p + side * 0.9, (-x).atan2(-z), 1.0, k as u8);
        b.add(Banner, p - side, 0.0, 1.0, k as u8);
    }
    for (i, p) in [vec2(8.0, 6.0), vec2(-8.0, 6.5)].into_iter().enumerate() {
        b.add(Stall, p, (-p.x).atan2(-p.y), 1.0, i as u8);
    }
    for (x, z) in [
        (11.0, 3.0),
        (11.6, 4.0),
        (-11.0, 4.5),
        (5.0, 12.0),
        (-5.0, 12.5),
    ] {
        b.add(Barrel, vec2(x, z), 0.0, 1.0, 0);
    }
    for (x, z, s) in [(12.0, 6.0, 0.5), (12.3, 6.1, 0.35), (-12.0, 2.8, 0.45)] {
        b.add(Crate, vec2(x, z), 0.3, s, 0);
    }
    for row in 0..2 {
        for i in 0..3 {
            let x = -9.0 - i as f32 * 2.2;
            let z = GRAVEYARD.y + 4.0 - row as f32 * 3.0;
            b.add(Grave, vec2(x, z), 0.0, 1.0, (row * 3 + i) as u8);
        }
    }
    for (x, z) in [
        (4.0, 26.0),
        (-4.0, -26.0),
        (26.0, -4.0),
        (-26.0, 4.0),
        (4.0, -26.0),
        (-4.0, 26.0),
        (26.0, 4.0),
        (-26.0, -4.0),
    ] {
        b.add(Lamp, vec2(x, z), 0.0, 1.0, 0);
    }

    // Two fields with crops, hay, a scarecrow and fences.
    let mut rng = Scatter(0xFA12 + (zone.index() as u32).wrapping_mul(7919));
    for (fi, &f) in layout.fields.iter().enumerate() {
        for i in 0..9 {
            for j in 0..9 {
                if rng.unit() < 0.45 {
                    continue;
                }
                let p = f + vec2(
                    -8.0 + i as f32 * 2.0 + rng.range(-0.3, 0.3),
                    -8.0 + j as f32 * 2.0 + rng.range(-0.3, 0.3),
                );
                if (i + j) % 7 == 0 {
                    b.add(HayBale, p, rng.range(0.0, 3.0), 1.0, fi as u8);
                } else {
                    b.add(
                        Crop,
                        p,
                        rng.range(0.0, std::f32::consts::TAU),
                        rng.range(0.7, 1.2),
                        fi as u8,
                    );
                }
            }
        }
        b.add(Scarecrow, f + vec2(1.0, 1.0), 0.4, 1.0, fi as u8);
        let r = 11.5;
        let corners = [vec2(-r, -r), vec2(r, -r), vec2(r, r), vec2(-r, r)];
        for k in 0..4 {
            let (a, c) = (f + corners[k], f + corners[(k + 1) % 4]);
            // Leave a gate on the side facing town.
            if (a + c).length() < (f * 2.0).length() {
                let mid = a.lerp(c, 0.5);
                b.fence(a, a.lerp(mid, 0.8));
                b.fence(c.lerp(mid, 0.8), c);
            } else {
                b.fence(a, c);
            }
        }
    }

    // Camps and the elite's ruins.
    for c in layout.camps() {
        for i in 0..4 {
            let a = i as f32 * 1.6 + 0.4;
            b.add(Tent, c + vec2(a.cos(), a.sin()) * 7.0, a, 1.0, i as u8);
        }
        for k in 0..14 {
            let a = k as f32 * 0.22 + 3.4;
            b.add(Stake, c + vec2(a.cos(), a.sin()) * 12.0, a, 1.0, 0);
        }
        b.add(Campfire, c, 0.0, 1.0, 0);
    }
    let ruins = layout.ruins();
    for i in 0..9 {
        let a = i as f32 * std::f32::consts::TAU / 9.0;
        let height = [6.0, 2.5, 5.0, 1.5, 6.5, 3.5, 4.0, 2.0, 5.5][i];
        b.add(
            Pillar,
            ruins + vec2(a.cos(), a.sin()) * 11.0,
            a,
            height,
            i as u8,
        );
    }
    for k in 0..6 {
        let a = k as f32 * 1.047;
        b.add(RuneTile, ruins + vec2(a.cos(), a.sin()) * 5.0, a, 1.0, 0);
    }

    // The wilds: each zone has its own mix.
    let mix: &[(PropKind, f32)] = match zone {
        Zone::Amberfall => &[
            (Conifer, 0.28),
            (Tree, 0.42),
            (DeadTree, 0.08),
            (Rock, 0.12),
            (Shrub, 0.04),
            (Mushrooms, 0.03),
            (Flowers, 0.02),
            (Log, 0.02),
        ],
        Zone::Scorchsand => &[
            (Cactus, 0.35),
            (Rock, 0.25),
            (Mesa, 0.05),
            (DeadTree, 0.13),
            (Bones, 0.12),
            (Shrub, 0.1),
        ],
        Zone::Silverbough => &[
            (Tree, 0.5),
            (Conifer, 0.1),
            (Rock, 0.1),
            (Shrub, 0.1),
            (Mushrooms, 0.1),
            (Flowers, 0.08),
            (Log, 0.02),
        ],
        Zone::Grubdeep => &[
            (Stalagmite, 0.4),
            (Crystal, 0.2),
            (Rock, 0.2),
            (Mushrooms, 0.2),
        ],
        Zone::Frostcog => &[
            (Conifer, 0.55),
            (Rock, 0.23),
            (DeadTree, 0.08),
            (Shrub, 0.1),
            (Log, 0.04),
        ],
        Zone::Witherwood => &[
            (DeadTree, 0.42),
            (Tombstone, 0.15),
            (Rock, 0.08),
            (Log, 0.05),
            (Shrub, 0.1),
            (Mushrooms, 0.1),
            (Bones, 0.1),
        ],
    };
    let attempts = if zone == Zone::Silverbough {
        1500
    } else {
        1100
    };
    let water = zone.water_level();
    let mut rng = Scatter(0x9E3779B9 ^ (zone.index() as u32).wrapping_mul(0x85EBCA6B));
    for _ in 0..attempts {
        let x = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let z = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let roll = rng.unit();
        let size = rng.range(0.8, 1.4);
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        let variant = (rng.unit() * 8.0) as u8;
        let h = layout.terrain.height(x, z);
        if near_landmark(zone, x, z) || !(water + 0.4..=16.0).contains(&h) {
            continue;
        }
        let mut acc = 0.0;
        let Some(kind) = mix.iter().find_map(|(k, w)| {
            acc += w;
            (roll < acc).then_some(*k)
        }) else {
            continue;
        };
        // Clump trees into woods: skip most of them in open meadows.
        let tree = matches!(kind, Tree | Conifer | DeadTree);
        let forest = (fbm(x * 0.018, z * 0.018, 2, 11 + zone.index() as u32) - 0.5) * 2.0;
        if tree && forest < -0.1 && rng.unit() < 0.8 {
            continue;
        }
        b.add(kind, vec2(x, z), yaw, size, variant);
    }
    b.props
}

/// A zone's footprints, bucketed for quick lookups.
pub struct Colliders {
    shapes: Vec<(Vec2, f32, Shape)>,
    cells: HashMap<(i32, i32), Vec<usize>>,
}

const CELL: f32 = 8.0;

fn cell_of(p: Vec2) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

impl Colliders {
    pub fn new(props: &[Prop]) -> Self {
        let mut shapes = Vec::new();
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for p in props {
            let Some(shape) = p.footprint() else { continue };
            let center = vec2(p.pos.x, p.pos.z);
            let reach = match shape {
                Shape::Circle(r) => r,
                Shape::Box(x, z) => x.hypot(z),
            };
            let i = shapes.len();
            shapes.push((center, p.yaw, shape));
            let (x0, z0) = cell_of(center - Vec2::splat(reach));
            let (x1, z1) = cell_of(center + Vec2::splat(reach));
            for cx in x0..=x1 {
                for cz in z0..=z1 {
                    cells.entry((cx, cz)).or_default().push(i);
                }
            }
        }
        Self { shapes, cells }
    }

    pub fn for_zone(zone: Zone) -> Self {
        Self::new(&props(zone))
    }

    /// Pushes a circle of `radius` at `pos` out of anything solid.
    pub fn resolve(&self, pos: Vec3, radius: f32) -> Vec3 {
        let mut p = vec2(pos.x, pos.z);
        for _ in 0..3 {
            let Some(list) = self.cells.get(&cell_of(p)) else {
                break;
            };
            let mut moved = false;
            for &i in list {
                let (center, yaw, shape) = self.shapes[i];
                if let Some(push) = push_out(p, radius, center, yaw, shape) {
                    p += push;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        Vec3::new(p.x, pos.y, p.y)
    }

    /// Whether a circle at `pos` overlaps anything solid.
    pub fn blocked(&self, pos: Vec3, radius: f32) -> bool {
        let p = vec2(pos.x, pos.z);
        self.cells.get(&cell_of(p)).is_some_and(|list| {
            list.iter().any(|&i| {
                let (center, yaw, shape) = self.shapes[i];
                push_out(p, radius, center, yaw, shape).is_some()
            })
        })
    }
}

/// How far to move a circle so it no longer overlaps a shape.
fn push_out(p: Vec2, radius: f32, center: Vec2, yaw: f32, shape: Shape) -> Option<Vec2> {
    let d = p - center;
    match shape {
        Shape::Circle(r) => {
            let len = d.length();
            let min = r + radius;
            if len >= min {
                return None;
            }
            let dir = if len > 1e-4 { d / len } else { vec2(1.0, 0.0) };
            Some(dir * (min - len))
        }
        Shape::Box(hx, hz) => {
            let f = vec2(yaw.sin(), yaw.cos());
            let r = vec2(-f.y, f.x);
            let local = vec2(d.dot(r), d.dot(f));
            let closest = vec2(local.x.clamp(-hx, hx), local.y.clamp(-hz, hz));
            let diff = local - closest;
            let push_local = if diff.length_squared() > 1e-8 {
                let len = diff.length();
                if len >= radius {
                    return None;
                }
                diff / len * (radius - len)
            } else {
                // Inside the box: leave through the nearest side.
                let to_x = hx - local.x.abs() + radius;
                let to_z = hz - local.y.abs() + radius;
                if to_x < to_z {
                    vec2(to_x * local.x.signum(), 0.0)
                } else {
                    vec2(0.0, to_z * local.y.signum())
                }
            };
            Some(r * push_local.x + f * push_local.y)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zone_has_a_town_and_wilds() {
        for zone in Zone::ALL {
            let props = props(zone);
            assert!(props.iter().filter(|p| p.kind == PropKind::House).count() >= 6);
            assert!(props.len() > 400, "{zone:?} has only {} props", props.len());
            assert!(props.iter().all(|p| Zone::at(p.pos) == zone));
        }
    }

    #[test]
    fn houses_are_solid() {
        let zone = Zone::Scorchsand;
        let colliders = Colliders::for_zone(zone);
        let house = props(zone)
            .into_iter()
            .find(|p| p.kind == PropKind::House)
            .unwrap();
        assert!(colliders.blocked(house.pos, 0.4));
        let out = colliders.resolve(house.pos, 0.4);
        assert!(!colliders.blocked(out, 0.39));
        // The middle of the town square (beside the centerpiece) is open.
        let open = zone.ground_local(vec2(0.0, -6.0));
        assert_eq!(colliders.resolve(open, 0.4), open);
    }

    #[test]
    fn boxes_push_out_sideways() {
        let push = push_out(vec2(0.0, 1.9), 0.4, Vec2::ZERO, 0.0, Shape::Box(3.0, 2.0)).unwrap();
        assert!(push.y > 0.0 && push.x.abs() < 1e-5);
        assert!(push_out(vec2(0.0, 2.5), 0.4, Vec2::ZERO, 0.0, Shape::Box(3.0, 2.0)).is_none());
        // Turned a quarter: the long side now runs along Z.
        let push = push_out(
            vec2(0.0, 2.8),
            0.4,
            Vec2::ZERO,
            std::f32::consts::FRAC_PI_2,
            Shape::Box(3.0, 2.0),
        )
        .unwrap();
        assert!(push.y > 0.0);
    }

    #[test]
    fn the_spawn_point_and_merchant_are_clear() {
        for zone in Zone::ALL {
            let colliders = Colliders::for_zone(zone);
            assert!(
                !colliders.blocked(zone.graveyard(), 0.4),
                "{zone:?} graveyard"
            );
            assert!(
                !colliders.blocked(zone.ground_local(MERCHANT_SPOT), 0.4),
                "{zone:?} merchant"
            );
            assert!(
                !colliders.blocked(zone.ground_local(QUEST_SPOT), 0.4),
                "{zone:?} quest giver"
            );
        }
    }

    #[test]
    fn houses_stand_clear_of_the_rest_of_town() {
        for zone in Zone::ALL {
            let all = props(zone);
            let reach = |p: &Prop| match p.footprint() {
                Some(Shape::Circle(r)) => r,
                Some(Shape::Box(x, z)) => x.min(z),
                None => 0.0,
            };
            for h in all.iter().filter(|p| p.kind == PropKind::House) {
                for o in &all {
                    if std::ptr::eq(o, h) || o.footprint().is_none() {
                        continue;
                    }
                    let d = vec2(o.pos.x - h.pos.x, o.pos.z - h.pos.z).length();
                    assert!(
                        d > 2.6 + reach(o),
                        "{zone:?}: a {:?} is inside a house",
                        o.kind
                    );
                }
            }
        }
    }
}
