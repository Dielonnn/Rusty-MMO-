//! What makes each starting area's map its own: the shape of its hills, how
//! its town is laid out, and where its fields, camps and elite ruins are.
//!
//! Everything here is in the zone's local coordinates (town at the origin).
//! The server spawns mobs at the camp sites and the client and collision
//! code place scenery around them, so both always agree.

use std::sync::OnceLock;

use glam::{Vec2, vec2};

use crate::props::Scatter;
use crate::world::*;

/// The hills of one zone: four overlapping waves, with valleys flattened by
/// `valleys` (lower means fewer, shallower lakes) and mountains at the edge.
#[derive(Clone, Copy, Debug)]
pub struct Terrain {
    pub freqs: [f32; 4],
    pub amps: [f32; 4],
    pub phase: Vec2,
    pub valleys: f32,
    pub edge: f32,
    /// Extra sharp ridges (snowy peaks, cave walls).
    pub ridges: f32,
}

/// Who lives at a camp site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteKind {
    /// Beasts roaming the open: no scenery.
    Beasts,
    /// A camp of humanoids, with tents and a campfire.
    Camp,
    /// The elite's ruins, ringed with pillars.
    Ruins,
}

/// A group of mobs: their role in `MobKind::for_zone` (0 hunter, 1 grazer,
/// 2 fighter, 3 caster, 4 elite), levels and how many.
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub role: usize,
    pub levels: (u8, u8),
    pub count: usize,
    /// Offset from the site's center, and how far they wander.
    pub offset: Vec2,
    pub radius: f32,
}

#[derive(Clone, Debug)]
pub struct Site {
    pub kind: SiteKind,
    pub center: Vec2,
    pub spawns: Vec<Spawn>,
}

/// A house in town: where it stands and which way its door faces.
#[derive(Clone, Copy, Debug)]
pub struct HouseSpot {
    pub pos: Vec2,
    pub yaw: f32,
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub terrain: Terrain,
    pub houses: Vec<HouseSpot>,
    pub fields: Vec<Vec2>,
    pub sites: Vec<Site>,
    /// Where the town's well stands.
    pub well: Vec2,
}

impl Layout {
    pub fn ruins(&self) -> Vec2 {
        self.sites
            .iter()
            .find(|s| s.kind == SiteKind::Ruins)
            .map_or(vec2(-150.0, -150.0), |s| s.center)
    }

    pub fn camps(&self) -> impl Iterator<Item = Vec2> + '_ {
        self.sites
            .iter()
            .filter(|s| s.kind == SiteKind::Camp)
            .map(|s| s.center)
    }
}

impl Zone {
    /// This zone's map.
    pub fn layout(self) -> &'static Layout {
        static LAYOUTS: OnceLock<Vec<Layout>> = OnceLock::new();
        &LAYOUTS.get_or_init(|| Zone::ALL.iter().map(|z| build(*z)).collect())[self.index()]
    }
}

pub fn terrain(zone: Zone) -> Terrain {
    match zone {
        // The original map.
        Zone::Amberfall => Terrain {
            freqs: [0.031, 0.027, 0.071, 0.013],
            amps: [3.5, 3.0, 1.1, 6.0],
            phase: Vec2::ZERO,
            valleys: 0.45,
            edge: 22.0,
            ridges: 0.0,
        },
        // Long, low dunes.
        Zone::Scorchsand => Terrain {
            freqs: [0.022, 0.017, 0.05, 0.009],
            amps: [4.0, 3.5, 1.4, 7.0],
            phase: vec2(40.0, -70.0),
            valleys: 0.3,
            edge: 18.0,
            ridges: 0.0,
        },
        // Gentle glades and wide lakes.
        Zone::Silverbough => Terrain {
            freqs: [0.028, 0.034, 0.06, 0.011],
            amps: [3.0, 2.6, 1.0, 5.5],
            phase: vec2(-90.0, 35.0),
            valleys: 0.6,
            edge: 26.0,
            ridges: 0.0,
        },
        // Rugged cave floors, walled in.
        Zone::Grubdeep => Terrain {
            freqs: [0.045, 0.04, 0.09, 0.017],
            amps: [3.0, 3.0, 1.6, 4.0],
            phase: vec2(13.0, 57.0),
            valleys: 0.5,
            edge: 32.0,
            ridges: 3.0,
        },
        // Tall, sharp peaks.
        Zone::Frostcog => Terrain {
            freqs: [0.024, 0.03, 0.08, 0.012],
            amps: [4.5, 4.0, 1.2, 7.0],
            phase: vec2(-33.0, -120.0),
            valleys: 0.35,
            edge: 30.0,
            ridges: 4.0,
        },
        // Low, boggy ground with many pools.
        Zone::Witherwood => Terrain {
            freqs: [0.035, 0.025, 0.065, 0.015],
            amps: [2.6, 2.4, 1.0, 5.0],
            phase: vec2(77.0, 11.0),
            valleys: 0.8,
            edge: 20.0,
            ridges: 0.0,
        },
    }
}

impl Terrain {
    /// Height at a local position.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let d = (x * x + z * z).sqrt();
        let outside_town = smoothstep(TOWN_RADIUS, TOWN_RADIUS + 30.0, d);
        let (px, pz) = (x + self.phase.x, z + self.phase.y);
        let [f0, f1, f2, f3] = self.freqs;
        let [a0, a1, a2, a3] = self.amps;
        let mut hills = (px * f0).sin() * a0
            + (pz * f1).cos() * a1
            + ((px + pz) * f2).sin() * a2
            + ((px - 0.6 * pz) * f3).sin() * a3;
        if hills < 0.0 {
            hills *= self.valleys;
        }
        if self.ridges > 0.0 {
            // Sharp crests where two waves meet.
            let r = 1.0 - ((px * 0.043).sin() * (pz * 0.037).cos()).abs();
            hills += r.powi(6) * self.ridges * 2.5;
        }
        let edge = smoothstep(
            WORLD_HALF_SIZE - 30.0,
            WORLD_HALF_SIZE,
            x.abs().max(z.abs()),
        );
        hills * outside_town + edge * self.edge
    }
}

/// Houses in a ring around the square.
fn ring(radius: f32, degrees: &[f32]) -> Vec<HouseSpot> {
    degrees
        .iter()
        .map(|deg| {
            let a = deg.to_radians();
            let pos = vec2(a.cos(), a.sin()) * radius;
            HouseSpot {
                pos,
                yaw: (-pos.x).atan2(-pos.y),
            }
        })
        .collect()
}

/// Houses at given spots, facing the square.
fn facing_center(spots: &[(f32, f32)]) -> Vec<HouseSpot> {
    spots
        .iter()
        .map(|&(x, z)| HouseSpot {
            pos: vec2(x, z),
            yaw: (-x).atan2(-z),
        })
        .collect()
}

fn houses(zone: Zone) -> Vec<HouseSpot> {
    match zone {
        Zone::Amberfall => ring(
            20.0,
            &[28.0, 62.0, 118.0, 152.0, 208.0, 242.0, 298.0, 332.0],
        ),
        // A wide ring of huts.
        Zone::Scorchsand => ring(
            22.0,
            &[20.0, 68.0, 112.0, 160.0, 205.0, 245.0, 295.0, 338.0],
        ),
        // Towers in a crescent, near and far.
        Zone::Silverbough => {
            let mut h = ring(18.5, &[40.0, 140.0, 220.0, 320.0]);
            h.extend(ring(23.5, &[70.0, 110.0, 250.0, 290.0]));
            h
        }
        // Two rows of shacks along a street.
        Zone::Grubdeep => {
            let mut h = Vec::new();
            for z in [-17.0f32, 17.0] {
                for x in [-18.0f32, -9.0, 9.0, 18.0] {
                    h.push(HouseSpot {
                        pos: vec2(x, z),
                        yaw: if z > 0.0 { std::f32::consts::PI } else { 0.0 },
                    });
                }
            }
            h
        }
        // A tight cluster of cottages.
        Zone::Frostcog => facing_center(&[
            (14.0, 15.0),
            (-14.0, 15.0),
            (14.0, -15.0),
            (-14.5, -16.0),
            (22.0, 7.5),
            (-22.0, 7.5),
            (22.0, -8.0),
            (-22.0, -8.5),
        ]),
        // A crooked lane of houses to the south, a few strays to the north.
        Zone::Witherwood => {
            let mut h = ring(20.0, &[195.0, 222.0, 250.0, 290.0, 318.0, 345.0]);
            h.extend(ring(23.0, &[60.0, 120.0]));
            h
        }
    }
}

/// The base camp sites, in the order levels rise.
fn base_sites() -> Vec<Site> {
    let spawn = |role, levels, count, radius| Spawn {
        role,
        levels,
        count,
        offset: Vec2::ZERO,
        radius,
    };
    let beasts = |x: f32, z: f32, role, levels, count| Site {
        kind: SiteKind::Beasts,
        center: vec2(x, z),
        spawns: vec![spawn(
            role,
            levels,
            count,
            if role == 0 { 14.0 } else { 16.0 },
        )],
    };
    let camp = |x: f32, z: f32, levels, fighters, casters| {
        let mut spawns = vec![spawn(2, levels, fighters, 14.0)];
        if casters > 0 {
            spawns.push(Spawn {
                offset: vec2(3.0, 3.0),
                ..spawn(3, levels, casters, 10.0)
            });
        }
        Site {
            kind: SiteKind::Camp,
            center: vec2(x, z),
            spawns,
        }
    };
    vec![
        beasts(50.0, 25.0, 0, (1, 2), 5),
        beasts(-45.0, 40.0, 1, (1, 3), 6),
        beasts(15.0, -60.0, 0, (2, 3), 5),
        beasts(-85.0, -35.0, 1, (3, 5), 6),
        camp(92.0, -72.0, (4, 5), 4, 2),
        beasts(20.0, 110.0, 0, (5, 6), 6),
        camp(122.0, 92.0, (6, 8), 5, 3),
        beasts(155.0, -20.0, 0, (7, 8), 6),
        camp(-120.0, -120.0, (8, 9), 5, 0),
        Site {
            kind: SiteKind::Ruins,
            center: vec2(-150.0, -150.0),
            spawns: vec![spawn(4, (10, 10), 1, 4.0)],
        },
    ]
}

/// Whether a spot (and the ground around it) is dry, not too steep and
/// inside the mountains.
fn usable(t: &Terrain, water: f32, p: Vec2, radius: f32) -> bool {
    if p.x.abs().max(p.y.abs()) > WORLD_HALF_SIZE - 40.0 {
        return false;
    }
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for k in 0..9 {
        let q = if k == 0 {
            p
        } else {
            let a = k as f32 * std::f32::consts::TAU / 8.0;
            p + vec2(a.cos(), a.sin()) * radius
        };
        let h = t.height(q.x, q.y);
        lo = lo.min(h);
        hi = hi.max(h);
    }
    lo > water + 0.6 && hi < 13.0 && hi - lo < radius * 0.8
}

/// Finds a usable spot near `want`, turning around the town and then
/// moving in or out a little.
fn settle(t: &Terrain, water: f32, want: Vec2, radius: f32, taken: &[Vec2]) -> Vec2 {
    let d = want.length();
    let a = want.y.atan2(want.x);
    let clear = |p: Vec2| taken.iter().all(|q| q.distance(p) > 34.0);
    for step in 0..92 {
        let turn = (step as f32 / 2.0).ceil() * if step % 2 == 0 { 1.0 } else { -1.0 } * 0.07;
        for scale in [1.0, 0.9, 1.1, 0.8] {
            let r = (d * scale).max(TOWN_RADIUS + 25.0);
            let p = vec2((a + turn).cos(), (a + turn).sin()) * r;
            if usable(t, water, p, radius) && clear(p) {
                return p;
            }
        }
    }
    want
}

fn build(zone: Zone) -> Layout {
    let t = terrain(zone);
    let water = zone.water_level();
    let mut rng = Scatter(0x51ED + (zone.index() as u32).wrapping_mul(0x9E37));
    // Each zone turns its sites around the town by its own amount, and
    // stretches them in or out a little.
    let spin = if zone == Zone::Amberfall {
        0.0
    } else {
        rng.range(0.6, 5.6)
    };
    let mut taken: Vec<Vec2> = Vec::new();
    let mut sites = base_sites();
    for site in &mut sites {
        let a = site.center.y.atan2(site.center.x) + spin + rng.range(-0.25, 0.25);
        let d = site.center.length() * rng.range(0.92, 1.08);
        let want = if zone == Zone::Amberfall {
            site.center
        } else {
            vec2(a.cos(), a.sin()) * d
        };
        let radius = if site.kind == SiteKind::Ruins {
            13.0
        } else {
            12.0
        };
        site.center = settle(&t, water, want, radius, &taken);
        taken.push(site.center);
    }
    let mut fields = Vec::new();
    for base in [vec2(38.0, -30.0), vec2(-36.0, -32.0)] {
        let a = base.y.atan2(base.x) + spin * 0.5;
        let want = if zone == Zone::Amberfall {
            base
        } else {
            vec2(a.cos(), a.sin()) * base.length() * rng.range(1.0, 1.15)
        };
        let f = settle(&t, water, want, 12.0, &taken);
        fields.push(f);
        taken.push(f);
    }
    let well = match zone {
        Zone::Amberfall => vec2(-7.0, -2.0),
        Zone::Scorchsand => vec2(-6.0, 9.0),
        Zone::Silverbough => vec2(7.5, -6.0),
        Zone::Grubdeep => vec2(-7.0, 9.5),
        Zone::Frostcog => vec2(8.0, -7.0),
        Zone::Witherwood => vec2(7.0, -4.0),
    };
    Layout {
        terrain: t,
        houses: houses(zone),
        fields,
        sites,
        well,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zone_lays_out_its_own_map() {
        let amber = Zone::Amberfall.layout();
        // The human map keeps its original camps.
        assert_eq!(amber.sites[0].center, vec2(50.0, 25.0));
        for zone in Zone::ALL {
            let l = zone.layout();
            assert_eq!(l.sites.len(), 10);
            for s in &l.sites {
                assert!(
                    usable(&l.terrain, zone.water_level(), s.center, 12.0),
                    "{zone:?} site {:?} at {}",
                    s.kind,
                    s.center
                );
            }
            for f in &l.fields {
                assert!(
                    usable(&l.terrain, zone.water_level(), *f, 12.0),
                    "{zone:?} field {f}"
                );
            }
            for h in &l.houses {
                assert!(h.pos.length() < TOWN_RADIUS, "{zone:?} house outside town");
            }
            if zone != Zone::Amberfall {
                let moved = l
                    .sites
                    .iter()
                    .zip(&amber.sites)
                    .filter(|(a, b)| a.center.distance(b.center) > 20.0)
                    .count();
                assert!(moved >= 6, "{zone:?} looks too much like Amberfall");
            }
        }
    }
}
