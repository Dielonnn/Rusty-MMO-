//! Trees, rocks, lamps, crops, stalls and the other props in the wilds and towns.

use macroquad::prelude::*;
use shared::props::{Prop, PropKind, Scatter};

use super::buildings::{centerpiece, house};
use super::foliage::{bough_fringe, bush, canopy};
use super::rocks::{Stone, boulder};
use shared::world::*;

use super::*;
use crate::models::{BONE, GOLD, WOOD};

/// A banner's cloth: a grid of quads rippling along its length.
pub(super) fn flag(b: &mut Batch, f: &Flag, time: f32) {
    let out = forward(f.yaw);
    let side = vec3(-out.z, 0.0, out.x);
    let cols = 6;
    let len = 1.3;
    let drop = 1.6;
    let wave = |k: usize| {
        let t = k as f32 / cols as f32;
        side * ((time * 3.0 - t * 4.0 + f.top.x).sin() * 0.18 * t)
    };
    for k in 0..cols {
        let (t0, t1) = (k as f32 / cols as f32, (k + 1) as f32 / cols as f32);
        let a = f.top + out * len * t0 + wave(k);
        let e = f.top + out * len * t1 + wave(k + 1);
        // A swallowtail: the bottom edge rises at the tip.
        let d0 = drop * (1.0 - (t0 - 0.7).max(0.0));
        let d1 = drop * (1.0 - (t1 - 0.7).max(0.0));
        let n = side;
        let col = if k % 2 == 0 {
            f.color
        } else {
            dark(f.color, 0.9)
        };
        b.quad([a, e, e - Vec3::Y * d1, a - Vec3::Y * d0], n, col);
        b.quad([e, a, a - Vec3::Y * d0, e - Vec3::Y * d1], -n, col);
        if k == cols / 2 {
            b.lit(|b| b.sphere(a - Vec3::Y * drop * 0.45 + side * 0.02, 0.1, GOLD));
        }
    }
}

/// Color of a zone's lamps and fires.
pub(super) fn lamp_color(zone: Zone) -> Color {
    match zone {
        Zone::Amberfall | Zone::Frostcog => c(1.0, 0.85, 0.45),
        Zone::Scorchsand => c(1.0, 0.6, 0.2),
        Zone::Silverbough => c(0.7, 0.85, 1.0),
        Zone::Grubdeep => c(0.5, 1.0, 0.45),
        Zone::Witherwood => c(0.55, 1.0, 0.35),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_prop(
    b: &mut Batch,
    cards: &mut Batch,
    zone: Zone,
    p: &Prop,
    chimneys: &mut Vec<Vec3>,
    lamps: &mut Vec<Glow>,
    fires: &mut Vec<Glow>,
    flags: &mut Vec<Flag>,
) {
    let pos = p.pos;
    let yaw = p.yaw;
    let s = p.size;
    let v = p.variant;
    let fr = Frame::upright(pos, yaw, 1.0);
    let mut rng = Scatter((pos.x * 13.0 + pos.z * 7.0).abs() as u32 | 1);
    match p.kind {
        PropKind::House => house(b, zone, pos, yaw, v, chimneys, lamps),
        PropKind::Waystone => super::dungeon::waystone(b, pos, yaw, lamps),
        PropKind::VaultWall => super::dungeon::wall(b, zone, pos, yaw, s),
        PropKind::Stall => market_stall(b, zone, pos, yaw, v),
        PropKind::Centerpiece => centerpiece(b, zone, pos, fires, lamps),
        PropKind::Grave => {
            let grave = if zone == Zone::Witherwood {
                c(0.4, 0.4, 0.44)
            } else {
                c(0.58, 0.58, 0.6)
            };
            fr.cube(b, vec3(0.0, 0.6, 0.0), vec3(0.12, 0.6, 0.45), grave);
            fr.cube(b, vec3(0.0, 1.0, 0.0), vec3(0.13, 0.08, 0.25), grave);
            fr.cube(
                b,
                vec3(-0.9, 0.05, 0.0),
                vec3(0.8, 0.05, 0.45),
                c(0.35, 0.28, 0.2),
            );
        }
        PropKind::Lamp => {
            let post = if zone == Zone::Silverbough {
                c(0.85, 0.85, 0.9)
            } else {
                c(0.2, 0.2, 0.22)
            };
            b.cylinder(pos, Vec3::Y * 3.5, 0.12, 6, post);
            b.block(pos + Vec3::Y * 3.55, vec3(0.25, 0.05, 0.25), 0.0, post);
            lamps.push(Glow {
                pos: pos + Vec3::Y * 3.85,
                color: lamp_color(zone),
            });
        }
        PropKind::Barrel => {
            b.cylinder(pos, Vec3::Y * 1.0, 0.4, 10, WOOD);
            b.cylinder(
                pos + Vec3::Y * 0.3,
                Vec3::Y * 0.06,
                0.42,
                10,
                c(0.3, 0.3, 0.32),
            );
            b.cylinder(
                pos + Vec3::Y * 0.75,
                Vec3::Y * 0.06,
                0.42,
                10,
                c(0.3, 0.3, 0.32),
            );
        }
        PropKind::Crate => b.block(pos + Vec3::Y * s, Vec3::splat(s), yaw, WOOD),
        PropKind::Tree => match zone {
            Zone::Silverbough => regal_tree(b, cards, &mut rng, pos, s),
            _ => autumn_tree(b, cards, &mut rng, pos - Vec3::Y * 0.2, s),
        },
        PropKind::Conifer => {
            let (green, snowy) = match zone {
                Zone::Frostcog => (c(0.16, 0.3, 0.24), true),
                Zone::Silverbough => (c(0.4, 0.55, 0.55), false),
                _ => (c(0.14, 0.3, 0.2), false),
            };
            let p0 = pos - Vec3::Y * 0.2;
            b.cylinder(p0, Vec3::Y * 1.6 * s, 0.22 * s, 6, c(0.36, 0.25, 0.16));
            let tiers = [
                (1.0, 1.6, 2.2),
                (2.1, 1.3, 2.0),
                (3.2, 1.0, 1.8),
                (4.2, 0.7, 1.5),
            ];
            let scaled = tiers.map(|(y, r, h)| (y * s, r * s, h * s));
            let fringe = if snowy {
                mix(green, c(0.95, 0.97, 1.0), 0.45)
            } else {
                dark(green, 1.25)
            };
            bough_fringe(cards, &mut rng, p0, &scaled, fringe);
            for (y, r, hgt) in tiers {
                b.cone(
                    p0 + Vec3::Y * y * s,
                    Vec3::Y * hgt * s,
                    r * s,
                    0.0,
                    8,
                    green,
                );
                if snowy {
                    let cap = p0 + Vec3::Y * (y + hgt * 0.55) * s;
                    b.cone(
                        cap,
                        Vec3::Y * hgt * 0.45 * s,
                        r * 0.5 * s,
                        0.0,
                        8,
                        c(0.95, 0.97, 1.0),
                    );
                }
            }
        }
        PropKind::DeadTree => dead_tree(b, zone, &mut rng, pos - Vec3::Y * 0.2, s),
        PropKind::Rock => {
            let grey = match zone {
                Zone::Scorchsand => c(0.66, 0.42, 0.28),
                Zone::Grubdeep => c(0.3, 0.29, 0.32),
                Zone::Frostcog => c(0.5, 0.52, 0.56),
                Zone::Witherwood => c(0.34, 0.33, 0.36),
                _ => c(0.5, 0.48, 0.46),
            };
            let top = match zone {
                Zone::Frostcog => c(0.96, 0.97, 1.0),
                Zone::Scorchsand => dark(grey, 1.15),
                Zone::Grubdeep => c(0.22, 0.32, 0.36),
                _ => c(0.4, 0.45, 0.22),
            };
            let stone = Stone {
                color: dark(grey, 1.1),
                top: Some(top),
                slices: 14,
                stacks: 9,
            };
            let seed = v as u32 * 31 + (pos.x.abs() * 7.0) as u32;
            let big = vec3(1.05, 0.85, 0.85) * s;
            boulder(b, pos + Vec3::Y * big.y * 0.3, big, yaw, seed, &stone);
            let small = vec3(0.55, 0.45, 0.5) * s;
            let side = forward(yaw + 1.2) * 1.0 * s;
            boulder(
                b,
                pos + side + Vec3::Y * small.y * 0.25,
                small,
                yaw + 0.7,
                seed + 1,
                &stone,
            );
        }
        PropKind::Shrub => {
            let col = match zone {
                Zone::Scorchsand => c(0.55, 0.5, 0.3),
                Zone::Silverbough => c(0.25, 0.55, 0.4),
                Zone::Frostcog => c(0.95, 0.97, 1.0),
                Zone::Witherwood => c(0.3, 0.2, 0.3),
                Zone::Grubdeep => c(0.3, 0.35, 0.4),
                Zone::Amberfall => c(0.7, 0.35, 0.1),
            };
            if zone == Zone::Frostcog {
                // A bush buried in snow.
                b.ellipsoid(pos + Vec3::Y * 0.5 * s, vec3(0.8, 0.55, 0.8) * s, col);
            } else {
                bush(b, cards, &mut rng, pos, s, col);
            }
            if matches!(zone, Zone::Amberfall | Zone::Silverbough) {
                for k in 0..4 {
                    let a = k as f32 * 1.6;
                    b.sphere(
                        pos + vec3(a.cos() * 0.6, 0.75, a.sin() * 0.6) * s,
                        0.07 * s,
                        c(0.75, 0.1, 0.12),
                    );
                }
            }
        }
        PropKind::Mushrooms => {
            let (cap, glowing) = match zone {
                Zone::Silverbough => (c(0.45, 0.7, 1.0), true),
                Zone::Grubdeep => (
                    if v.is_multiple_of(2) {
                        c(0.45, 1.0, 0.55)
                    } else {
                        c(0.8, 0.45, 1.0)
                    },
                    true,
                ),
                Zone::Witherwood => (c(0.55, 0.65, 0.3), false),
                _ => (c(0.75, 0.18, 0.1), false),
            };
            let big = if zone == Zone::Grubdeep { 3.0 } else { 1.0 };
            for k in 0..3 {
                let o = vec3((k as f32 * 2.3).cos(), 0.0, (k as f32 * 2.3).sin()) * 0.35 * big;
                let sz = rng.range(0.6, 1.0) * big;
                b.cylinder(
                    pos + o,
                    Vec3::Y * 0.35 * sz,
                    0.05 * sz,
                    6,
                    c(0.9, 0.86, 0.78),
                );
                let at = pos + o + Vec3::Y * 0.38 * sz;
                if glowing {
                    b.lit(|b| b.ellipsoid(at, vec3(0.18, 0.08, 0.18) * sz, cap));
                } else {
                    b.ellipsoid(at, vec3(0.18, 0.08, 0.18) * sz, cap);
                }
            }
        }
        PropKind::Cactus => {
            let green = c(0.3, 0.52, 0.28);
            b.cone(pos, Vec3::Y * 3.2 * s, 0.35 * s, 0.3 * s, 8, green);
            b.sphere(pos + Vec3::Y * 3.2 * s, 0.3 * s, green);
            for sx in [-1.0, 1.0] {
                let side = forward(yaw + sx * 1.57);
                let arm = pos + Vec3::Y * (1.4 + sx * 0.3) * s + side * 0.3 * s;
                let out = arm + side * 0.5 * s;
                b.beam(arm, out, 0.17 * s, Vec3::Y, green);
                b.cone(
                    out - Vec3::Y * 0.1 * s,
                    Vec3::Y * 1.1 * s,
                    0.2 * s,
                    0.18 * s,
                    6,
                    green,
                );
                if v.is_multiple_of(3) {
                    b.sphere(out + Vec3::Y * 1.0 * s, 0.12 * s, c(0.95, 0.35, 0.5));
                }
            }
        }
        PropKind::Mesa => {
            let rock = c(0.7, 0.42, 0.28);
            b.cone(pos - Vec3::Y, Vec3::Y * 9.0 * s, 4.2 * s, 3.0 * s, 16, rock);
            b.cone(
                pos + Vec3::Y * 8.0 * s,
                Vec3::Y * 1.0 * s,
                3.1 * s,
                2.8 * s,
                16,
                dark(rock, 1.15),
            );
            for k in 0..3 {
                let r = (4.1 - k as f32 * 0.4) * s;
                b.cylinder(
                    pos + Vec3::Y * (2.0 + k as f32 * 2.5) * s,
                    Vec3::Y * 0.3,
                    r,
                    16,
                    dark(rock, 0.8),
                );
            }
            // Fallen boulders around its foot.
            let stone = Stone {
                color: dark(rock, 0.95),
                top: None,
                slices: 10,
                stacks: 7,
            };
            for k in 0..4 {
                let a = yaw + k as f32 * 1.7;
                let r = vec3(0.9, 0.7, 0.8) * s * (0.7 + (k % 2) as f32 * 0.5);
                let at = pos + forward(a) * 4.6 * s;
                let at = vec3(at.x, terrain_height(at.x, at.z) + r.y * 0.25, at.z);
                boulder(b, at, r, a, v as u32 * 7 + k, &stone);
            }
        }
        PropKind::Stalagmite => {
            let rock = c(0.33, 0.31, 0.34);
            b.cone(
                pos - Vec3::Y * 0.2,
                Vec3::Y * 4.0 * s,
                0.7 * s,
                0.0,
                12,
                rock,
            );
            b.cone(
                pos + forward(yaw) * 0.6 * s,
                Vec3::Y * 2.0 * s,
                0.4 * s,
                0.0,
                6,
                dark(rock, 1.1),
            );
        }
        PropKind::Crystal => {
            let col = if v.is_multiple_of(2) {
                c(0.55, 0.4, 1.0)
            } else {
                c(0.35, 0.85, 1.0)
            };
            b.lit(|b| {
                for k in 0..4 {
                    let tilt = forward(yaw + k as f32 * 1.6) * 0.4;
                    b.cone(
                        pos,
                        (Vec3::Y * 2.5 + tilt) * s * (1.0 - k as f32 * 0.15),
                        0.35 * s,
                        0.0,
                        5,
                        col,
                    );
                }
            });
            lamps.push(Glow {
                pos: pos + Vec3::Y * 1.5 * s,
                color: Color::new(col.r, col.g, col.b, 0.5),
            });
        }
        PropKind::Tombstone => {
            let stone = c(0.45, 0.45, 0.48);
            fr.cube(b, vec3(0.0, 0.55, 0.0), vec3(0.4, 0.55, 0.12), stone);
            fr.cylinder_dir(b, vec3(0.0, 1.1, -0.12), vec3(0.0, 0.0, 0.24), 0.4, stone);
            fr.cube(
                b,
                vec3(0.0, 0.03, 0.8),
                vec3(0.45, 0.04, 0.7),
                c(0.25, 0.22, 0.18),
            );
        }
        PropKind::Bones => {
            for k in 0..3 {
                let d = forward(yaw + k as f32 * 1.1) * 0.3;
                b.beam(
                    pos + d + Vec3::Y * 0.05,
                    pos - d + Vec3::Y * 0.05,
                    0.04,
                    Vec3::Y,
                    BONE,
                );
            }
            b.sphere(pos + forward(yaw) * 0.5 + Vec3::Y * 0.15, 0.17, BONE);
        }
        PropKind::Crop => crop(b, zone, pos, s, v),
        PropKind::HayBale => {
            let col = match zone {
                Zone::Frostcog => c(0.95, 0.97, 1.0),
                Zone::Grubdeep => c(0.4, 0.35, 0.3),
                _ => c(0.85, 0.7, 0.35),
            };
            let f = forward(yaw);
            b.cone_ref(
                pos + Vec3::Y * 0.6 - f * 0.6,
                f * 1.2,
                Vec3::Y,
                0.6,
                0.6,
                10,
                col,
            );
        }
        PropKind::Scarecrow => scarecrow(b, zone, pos),
        PropKind::Fence => {
            let col = match zone {
                Zone::Silverbough => c(0.9, 0.9, 0.92),
                Zone::Grubdeep => c(0.45, 0.42, 0.4),
                Zone::Witherwood => c(0.25, 0.22, 0.2),
                _ => c(0.45, 0.32, 0.2),
            };
            let f = forward(yaw);
            let posts = ((s * 2.0) / 2.5).ceil().max(1.0) as usize;
            for i in 0..=posts {
                let t = i as f32 / posts as f32 * 2.0 - 1.0;
                let at = pos + f * s * t;
                b.block(
                    ground(at.x, at.z) + Vec3::Y * 0.55,
                    vec3(0.08, 0.6, 0.08),
                    yaw,
                    col,
                );
            }
            let (a, e) = (pos - f * s, pos + f * s);
            for y in [0.45, 0.85] {
                b.beam(
                    ground(a.x, a.z) + Vec3::Y * y,
                    ground(e.x, e.z) + Vec3::Y * y,
                    0.045,
                    Vec3::Y,
                    col,
                );
            }
        }
        PropKind::Tent => {
            let canvas = match zone {
                Zone::Scorchsand => c(0.6, 0.38, 0.25),
                Zone::Silverbough => c(0.35, 0.5, 0.3),
                Zone::Grubdeep => c(0.35, 0.33, 0.35),
                Zone::Frostcog => c(0.7, 0.8, 0.9),
                Zone::Witherwood => c(0.18, 0.15, 0.2),
                Zone::Amberfall => c(0.58, 0.48, 0.33),
            };
            b.cone_ref(
                pos - Vec3::Y * 0.2,
                Vec3::Y * 3.0,
                forward(yaw),
                2.2,
                0.0,
                6,
                canvas,
            );
            b.cylinder(pos - Vec3::Y * 0.2, Vec3::Y * 3.6, 0.06, 5, WOOD);
        }
        PropKind::Stake => {
            let col = match zone {
                Zone::Witherwood => BONE,
                Zone::Frostcog => c(0.75, 0.88, 1.0),
                _ => WOOD,
            };
            b.cone(pos - Vec3::Y * 0.2, Vec3::Y * 2.6, 0.22, 0.0, 5, col);
        }
        PropKind::Campfire => {
            for i in 0..3 {
                b.block(
                    pos + Vec3::Y * 0.15,
                    vec3(0.9, 0.12, 0.12),
                    i as f32 * 2.1,
                    WOOD,
                );
            }
            for k in 0..8 {
                let a = k as f32 * 0.785;
                b.sphere(
                    pos + vec3(a.cos() * 1.1, 0.1, a.sin() * 1.1),
                    0.22,
                    c(0.45, 0.43, 0.42),
                );
            }
            let flame = match zone {
                Zone::Witherwood => c(0.4, 1.0, 0.3),
                Zone::Grubdeep => c(0.4, 0.7, 1.0),
                _ => c(1.0, 0.5, 0.1),
            };
            fires.push(Glow { pos, color: flame });
        }
        PropKind::Pillar => {
            let stone = match zone {
                Zone::Scorchsand => c(0.78, 0.62, 0.42),
                Zone::Silverbough => c(0.85, 0.85, 0.88),
                Zone::Grubdeep => c(0.32, 0.3, 0.36),
                Zone::Frostcog => c(0.7, 0.8, 0.9),
                Zone::Witherwood => c(0.3, 0.28, 0.3),
                Zone::Amberfall => c(0.52, 0.52, 0.5),
            };
            let p0 = pos - Vec3::Y * 0.3;
            b.cone_ref(p0, Vec3::Y * s, forward(yaw), 0.8, 0.8, 10, stone);
            b.cone_ref(
                p0 + Vec3::Y * s * 0.5,
                Vec3::Y * 0.3,
                forward(yaw),
                0.85,
                0.85,
                10,
                dark(stone, 0.8),
            );
            if s > 5.0 {
                b.block(p0 + Vec3::Y * (s + 0.25), vec3(1.1, 0.25, 1.1), yaw, stone);
            }
        }
        PropKind::RuneTile => {
            let col = match zone {
                Zone::Silverbough | Zone::Witherwood => c(0.5, 1.0, 0.4),
                Zone::Scorchsand => c(1.0, 0.7, 0.25),
                Zone::Grubdeep => c(0.7, 0.45, 1.0),
                _ => c(0.35, 0.85, 1.0),
            };
            b.lit(|b| b.block(pos + Vec3::Y * 0.06, vec3(0.5, 0.04, 0.12), yaw, col));
        }
        PropKind::Well => {
            let stone = match zone {
                Zone::Silverbough => c(0.88, 0.88, 0.92),
                Zone::Scorchsand => c(0.7, 0.5, 0.35),
                Zone::Frostcog => c(0.62, 0.64, 0.7),
                _ => c(0.5, 0.48, 0.46),
            };
            // A round stone wall with a roof on two posts and a bucket.
            b.cone_ref(pos, Vec3::Y * 0.9, forward(yaw), 1.2, 1.2, 14, stone);
            b.cone_ref(
                pos + Vec3::Y * 0.9,
                Vec3::Y * 0.12,
                forward(yaw),
                1.28,
                1.28,
                14,
                dark(stone, 0.8),
            );
            let water = theme(zone).water;
            b.cylinder(
                pos + Vec3::Y * 0.8,
                Vec3::Y * 0.12,
                1.0,
                14,
                Color::new(water.r, water.g, water.b, 1.0),
            );
            let r = vec3(-forward(yaw).z, 0.0, forward(yaw).x);
            for sx in [-1.0, 1.0] {
                b.block(
                    pos + r * sx * 1.05 + Vec3::Y * 1.6,
                    vec3(0.08, 1.6, 0.08),
                    yaw,
                    WOOD,
                );
            }
            b.beam(
                pos - r * 1.1 + Vec3::Y * 2.2,
                pos + r * 1.1 + Vec3::Y * 2.2,
                0.06,
                Vec3::Y,
                WOOD,
            );
            let roof = match zone {
                Zone::Amberfall => c(0.62, 0.22, 0.15),
                Zone::Frostcog => c(0.95, 0.97, 1.0),
                _ => dark(WOOD, 1.2),
            };
            let f = forward(yaw);
            for sz in [-1.0, 1.0] {
                let a = pos + Vec3::Y * 3.1;
                let e = pos + f * sz * 1.4 + Vec3::Y * 2.3;
                b.quad(
                    [a - r * 1.4, a + r * 1.4, e + r * 1.4, e - r * 1.4],
                    (Vec3::Y + f * sz).normalize(),
                    roof,
                );
            }
            b.beam(
                pos + Vec3::Y * 2.2,
                pos + Vec3::Y * 1.5,
                0.01,
                Vec3::X,
                c(0.6, 0.55, 0.45),
            );
            b.cone(pos + Vec3::Y * 1.2, Vec3::Y * 0.3, 0.16, 0.2, 8, WOOD);
        }
        PropKind::Cart => {
            let f = forward(yaw);
            let r = vec3(-f.z, 0.0, f.x);
            let wood = dark(WOOD, 1.15);
            b.block(pos + Vec3::Y * 0.85, vec3(0.8, 0.12, 1.4), yaw, wood);
            for sx in [-1.0, 1.0] {
                b.block(
                    pos + r * sx * 0.78 + Vec3::Y * 1.15,
                    vec3(0.04, 0.25, 1.4),
                    yaw,
                    wood,
                );
                // Wheels.
                let hub = pos + r * sx * 0.95 + Vec3::Y * 0.6 - f * 0.4;
                b.cone_ref(
                    hub - r * sx * 0.05,
                    r * sx * 0.1,
                    Vec3::Y,
                    0.6,
                    0.6,
                    12,
                    dark(WOOD, 0.8),
                );
                for k in 0..4 {
                    let a = k as f32 * 0.785;
                    let d = Vec3::Y * a.sin() + f * a.cos();
                    b.beam(
                        hub + r * sx * 0.06 - d * 0.55,
                        hub + r * sx * 0.06 + d * 0.55,
                        0.03,
                        r,
                        WOOD,
                    );
                }
            }
            // Shafts and a load of sacks.
            for sx in [-1.0, 1.0] {
                b.beam(
                    pos + r * sx * 0.4 + f * 1.3 + Vec3::Y * 0.8,
                    pos + r * sx * 0.4 + f * 2.6 + Vec3::Y * 0.3,
                    0.05,
                    Vec3::Y,
                    WOOD,
                );
            }
            for k in 0..3 {
                let at = pos
                    + Vec3::Y * 1.2
                    + f * (-0.7 + k as f32 * 0.6)
                    + r * ((k % 2) as f32 * 0.3 - 0.15);
                b.ellipsoid(at, vec3(0.35, 0.28, 0.3), c(0.78, 0.68, 0.48));
            }
        }
        PropKind::Signpost => {
            b.block(pos + Vec3::Y * 1.3, vec3(0.08, 1.3, 0.08), 0.0, WOOD);
            // Two arms pointing along the road.
            let f = forward(yaw);
            let r = vec3(-f.z, 0.0, f.x);
            for (k, sx) in [(0.0f32, 1.0f32), (1.0, -1.0)] {
                let at = pos + Vec3::Y * (2.1 - k * 0.45) + r * sx * 0.45;
                b.block(
                    at,
                    vec3(0.45, 0.13, 0.03),
                    yaw + std::f32::consts::FRAC_PI_2,
                    dark(WOOD, 1.25),
                );
                b.cone_ref(
                    at + r * sx * 0.45,
                    r * sx * 0.2,
                    Vec3::Y,
                    0.14,
                    0.0,
                    4,
                    dark(WOOD, 1.25),
                );
            }
            b.cone(
                pos + Vec3::Y * 2.6,
                Vec3::Y * 0.25,
                0.12,
                0.0,
                4,
                dark(WOOD, 0.8),
            );
        }
        PropKind::Banner => {
            let color = banner_color(zone);
            b.cylinder(pos, Vec3::Y * 5.0, 0.07, 6, c(0.25, 0.2, 0.18));
            b.sphere(pos + Vec3::Y * 5.05, 0.12, GOLD);
            b.beam(
                pos + Vec3::Y * 4.8,
                pos + Vec3::Y * 4.8 + forward(yaw + 0.6) * 1.3,
                0.035,
                Vec3::Y,
                c(0.25, 0.2, 0.18),
            );
            flags.push(Flag {
                top: pos + Vec3::Y * 4.8,
                yaw: yaw + 0.6,
                color,
            });
        }
        PropKind::Flowers => {
            let palette: [Color; 3] = match zone {
                Zone::Silverbough => [c(0.75, 0.8, 1.0), c(0.95, 0.95, 1.0), c(0.6, 0.5, 0.95)],
                _ => [c(0.95, 0.75, 0.2), c(0.85, 0.25, 0.2), c(0.9, 0.55, 0.85)],
            };
            for k in 0..9 {
                let a = k as f32 * 2.4 + rng.unit();
                let r = 0.2 + rng.unit() * 0.9;
                let at = pos + vec3(a.cos() * r, 0.0, a.sin() * r);
                let at = vec3(at.x, terrain_height(at.x, at.z), at.z);
                let h = 0.25 + rng.unit() * 0.25;
                b.cylinder(at, Vec3::Y * h, 0.015, 3, c(0.3, 0.55, 0.25));
                let col = palette[k % 3];
                if zone == Zone::Silverbough {
                    b.glow_sphere(at + Vec3::Y * h, 0.06, col);
                } else {
                    b.ellipsoid(at + Vec3::Y * h, vec3(0.08, 0.035, 0.08), col);
                }
            }
        }
        PropKind::Log => {
            let bark = match zone {
                Zone::Witherwood => c(0.18, 0.15, 0.14),
                Zone::Frostcog => c(0.36, 0.3, 0.26),
                _ => c(0.36, 0.25, 0.16),
            };
            let f = forward(yaw);
            let half = 1.6 * s;
            let a = pos - f * half + Vec3::Y * 0.35;
            b.cone_ref(a, f * half * 2.0, Vec3::Y, 0.38, 0.33, 8, bark);
            // Cut rings at both ends, moss or snow on top.
            b.cone_ref(
                a - f * 0.02,
                f * 0.02,
                Vec3::Y,
                0.32,
                0.32,
                8,
                c(0.75, 0.6, 0.4),
            );
            let top = match zone {
                Zone::Frostcog => c(0.96, 0.97, 1.0),
                Zone::Scorchsand => dark(bark, 1.2),
                _ => c(0.32, 0.45, 0.2),
            };
            b.cone_ref(
                a + Vec3::Y * 0.3 + f * half * 0.3,
                f * half * 1.0,
                Vec3::Y,
                0.18,
                0.15,
                6,
                top,
            );
            if zone != Zone::Scorchsand {
                b.cylinder(
                    pos + f * half * 0.3 + Vec3::Y * 0.6,
                    Vec3::Y * 0.12,
                    0.04,
                    5,
                    c(0.9, 0.86, 0.78),
                );
                b.ellipsoid(
                    pos + f * half * 0.3 + Vec3::Y * 0.74,
                    vec3(0.12, 0.05, 0.12),
                    c(0.75, 0.25, 0.12),
                );
            }
        }
    }
    // A soft shadow under things standing on the ground, so they sit on it.
    let shadow = match p.kind {
        PropKind::Tree => 2.6 * s,
        PropKind::Conifer => 2.2 * s,
        PropKind::DeadTree => 1.4 * s,
        PropKind::Rock => 1.5 * s,
        PropKind::Shrub => 1.1 * s,
        PropKind::Cactus | PropKind::Stalagmite => 1.0 * s,
        PropKind::Crate => 1.6 * s,
        PropKind::Barrel | PropKind::Lamp | PropKind::Scarecrow => 0.6,
        PropKind::HayBale => 1.1,
        PropKind::Well => 1.8,
        PropKind::Cart => 1.9,
        PropKind::Stall => 2.4,
        _ => 0.0,
    };
    b.blob_shadow(pos, shadow, 0.4);
}

/// The color of a zone's banners.
pub(super) fn banner_color(zone: Zone) -> Color {
    match zone {
        Zone::Amberfall => c(0.2, 0.32, 0.65),
        Zone::Scorchsand => c(0.75, 0.15, 0.1),
        Zone::Silverbough => c(0.25, 0.55, 0.45),
        Zone::Grubdeep => c(0.85, 0.6, 0.15),
        Zone::Frostcog => c(0.85, 0.2, 0.25),
        Zone::Witherwood => c(0.35, 0.15, 0.45),
    }
}

pub(super) fn autumn_tree(b: &mut Batch, cards: &mut Batch, rng: &mut Scatter, p: Vec3, size: f32) {
    const LEAVES: [Color; 5] = [
        c(0.9, 0.46, 0.12),
        c(0.75, 0.2, 0.1),
        c(0.93, 0.72, 0.2),
        c(0.82, 0.34, 0.1),
        c(0.6, 0.3, 0.12),
    ];
    let trunk = c(0.33, 0.22, 0.14);
    let leaves = LEAVES[(rng.unit() * 5.0) as usize % 5];
    let leaves2 = LEAVES[(rng.unit() * 5.0) as usize % 5];
    b.cone(p, Vec3::Y * 2.4 * size, 0.3 * size, 0.18 * size, 7, trunk);
    for k in 0..3 {
        let a = k as f32 * 2.1 + rng.unit();
        let dir = vec3(a.cos() * 0.8, 1.0, a.sin() * 0.8) * size;
        b.cone(
            p + Vec3::Y * 1.8 * size,
            dir,
            0.1 * size,
            0.04 * size,
            5,
            trunk,
        );
    }
    for (at, r, col, count) in [
        (vec3(0.0, 3.2, 0.0), vec3(1.5, 1.2, 1.5), leaves, 22),
        (vec3(0.9, 2.8, 0.4), Vec3::splat(0.95), leaves2, 10),
        (vec3(-0.8, 2.9, -0.5), Vec3::splat(1.0), leaves, 10),
        (vec3(0.1, 3.9, -0.2), Vec3::splat(0.85), leaves2, 9),
    ] {
        canopy(b, cards, rng, p + at * size, r * size, col, count);
    }
    for k in 0..6 {
        let a = k as f32 * 1.05 + rng.unit();
        let r = rng.range(0.6, 2.2) * size;
        let cc = p + vec3(a.cos() * r, 0.0, a.sin() * r);
        let cc = vec3(cc.x, terrain_height(cc.x, cc.z) + 0.05, cc.z);
        let u = vec3(a.cos(), 0.0, a.sin()) * 0.25;
        let v = vec3(-a.sin(), 0.0, a.cos()) * 0.16;
        b.quad(
            [cc - u, cc - v, cc + u, cc + v],
            Vec3::Y,
            if k % 2 == 0 { leaves } else { leaves2 },
        );
    }
}

/// The great silver-barked trees of the elven forest.
pub(super) fn regal_tree(b: &mut Batch, cards: &mut Batch, rng: &mut Scatter, p: Vec3, size: f32) {
    let bark = c(0.8, 0.8, 0.84);
    let s = size * 1.6;
    b.cone(
        p - Vec3::Y * 0.3,
        Vec3::Y * 6.0 * s,
        0.55 * s,
        0.3 * s,
        8,
        bark,
    );
    for k in 0..4 {
        let a = k as f32 * 1.57 + rng.unit();
        b.cone(
            p,
            vec3(a.cos(), -0.15, a.sin()) * 1.4 * s,
            0.25 * s,
            0.05 * s,
            5,
            bark,
        );
        let dir = vec3(a.cos() * 1.5, 1.0, a.sin() * 1.5) * s;
        b.cone(p + Vec3::Y * 4.5 * s, dir, 0.18 * s, 0.06 * s, 5, bark);
    }
    let greens = [c(0.25, 0.6, 0.45), c(0.35, 0.68, 0.4), c(0.85, 0.82, 0.45)];
    for k in 0..6 {
        let a = k as f32 * 1.05 + rng.unit();
        let r = if k == 0 { 0.0 } else { 1.6 };
        let col = greens[(rng.unit() * 3.0) as usize % 3];
        let at = p + vec3(a.cos() * r, 6.3 + (k % 2) as f32 * 0.8, a.sin() * r) * s;
        canopy(b, cards, rng, at, vec3(1.6, 1.0, 1.6) * s, col, 14);
    }
    // Glowing seed pods.
    for k in 0..3 {
        let a = k as f32 * 2.1 + rng.unit();
        b.glow_sphere(
            p + vec3(a.cos() * 1.8, 5.3, a.sin() * 1.8) * s,
            0.12 * s,
            c(0.75, 0.95, 1.0),
        );
    }
}

pub(super) fn dead_tree(b: &mut Batch, zone: Zone, rng: &mut Scatter, p: Vec3, size: f32) {
    let (trunk, moss) = match zone {
        Zone::Witherwood => (c(0.16, 0.13, 0.13), Some(c(0.35, 0.42, 0.28))),
        Zone::Scorchsand => (c(0.52, 0.42, 0.32), None),
        Zone::Frostcog => (c(0.4, 0.36, 0.34), None),
        _ => (c(0.3, 0.22, 0.17), None),
    };
    let tall = if zone == Zone::Witherwood { 1.4 } else { 1.0 };
    b.cone(
        p,
        vec3(0.3, 3.0 * tall, 0.1) * size,
        0.28 * size,
        0.1 * size,
        6,
        trunk,
    );
    for k in 0..5 {
        let a = k as f32 * 1.3 + rng.unit();
        let h = rng.range(1.4, 2.6) * size * tall;
        let dir = vec3(a.cos(), rng.range(0.4, 1.2), a.sin()) * rng.range(0.8, 1.6) * size;
        b.cone(p + Vec3::Y * h, dir, 0.08 * size, 0.02 * size, 4, trunk);
        if let Some(m) = moss {
            // Hanging moss.
            b.cone(
                p + Vec3::Y * h + dir,
                -Vec3::Y * 0.9 * size,
                0.08 * size,
                0.0,
                4,
                m,
            );
        }
    }
}

pub(super) fn crop(b: &mut Batch, zone: Zone, p: Vec3, s: f32, field: u8) {
    match (zone, field) {
        (Zone::Amberfall | Zone::Witherwood, 0) => {
            // Pumpkins (grey, glowing ones in the dying forest).
            let col = if zone == Zone::Witherwood {
                c(0.35, 0.3, 0.38)
            } else {
                c(0.95, 0.5, 0.1)
            };
            b.ellipsoid(p + Vec3::Y * 0.3 * s, vec3(0.45, 0.33, 0.45) * s, col);
            b.cylinder(
                p + Vec3::Y * 0.6 * s,
                Vec3::Y * 0.18,
                0.05,
                5,
                c(0.3, 0.4, 0.15),
            );
            if zone == Zone::Witherwood {
                b.glow_sphere(p + vec3(0.0, 0.35, 0.4) * s, 0.06, c(0.5, 1.0, 0.3));
            }
        }
        (Zone::Scorchsand, 0) => {
            b.ellipsoid(
                p + Vec3::Y * 0.3 * s,
                vec3(0.35, 0.3, 0.35) * s,
                c(0.65, 0.7, 0.25),
            );
            for k in 0..5 {
                let a = k as f32 * 1.26;
                let at = p + vec3(a.cos() * 0.25, 0.35, a.sin() * 0.25) * s;
                b.cone(
                    at,
                    vec3(a.cos(), 0.5, a.sin()) * 0.2 * s,
                    0.04,
                    0.0,
                    3,
                    c(0.85, 0.8, 0.5),
                );
            }
        }
        (Zone::Silverbough, 0) => {
            b.cylinder(p, Vec3::Y * 0.7 * s, 0.03, 4, c(0.3, 0.55, 0.3));
            b.lit(|b| {
                b.ellipsoid(
                    p + Vec3::Y * 0.75 * s,
                    vec3(0.2, 0.1, 0.2) * s,
                    c(0.92, 0.95, 1.0),
                )
            });
        }
        (Zone::Grubdeep, 0) => {
            b.cylinder(p, Vec3::Y * 0.5 * s, 0.06, 5, c(0.8, 0.78, 0.7));
            b.lit(|b| {
                b.ellipsoid(
                    p + Vec3::Y * 0.55 * s,
                    vec3(0.3, 0.12, 0.3) * s,
                    c(0.45, 1.0, 0.55),
                )
            });
        }
        (Zone::Frostcog, 0) => {
            b.ellipsoid(
                p + Vec3::Y * 0.25 * s,
                vec3(0.4, 0.28, 0.4) * s,
                c(0.55, 0.75, 0.5),
            );
            b.ellipsoid(
                p + Vec3::Y * 0.45 * s,
                vec3(0.25, 0.08, 0.25) * s,
                c(0.95, 0.97, 1.0),
            );
        }
        _ => {
            // Sheaves of grain (or moss, or snowy stubble).
            let col = match zone {
                Zone::Scorchsand => c(0.85, 0.72, 0.42),
                Zone::Silverbough => c(0.75, 0.8, 0.55),
                Zone::Grubdeep => c(0.3, 0.5, 0.45),
                Zone::Frostcog => c(0.8, 0.75, 0.6),
                Zone::Witherwood => c(0.42, 0.38, 0.3),
                Zone::Amberfall => c(0.88, 0.72, 0.36),
            };
            b.cone(p, Vec3::Y * 1.3 * s, 0.35, 0.15, 7, col);
            b.cone(
                p + Vec3::Y * 1.3 * s,
                Vec3::Y * 0.4,
                0.3,
                0.0,
                7,
                dark(col, 0.9),
            );
        }
    }
}

pub(super) fn scarecrow(b: &mut Batch, zone: Zone, p: Vec3) {
    match zone {
        Zone::Frostcog => {
            // A snowman.
            let snow = c(0.96, 0.97, 1.0);
            b.sphere(p + Vec3::Y * 0.6, 0.65, snow);
            b.sphere(p + Vec3::Y * 1.45, 0.45, snow);
            b.sphere(p + Vec3::Y * 2.05, 0.32, snow);
            b.cone(
                p + vec3(0.0, 2.05, 0.25),
                vec3(0.0, 0.0, 0.35),
                0.06,
                0.0,
                6,
                c(0.95, 0.5, 0.1),
            );
            b.cylinder(
                p + Vec3::Y * 2.3,
                Vec3::Y * 0.35,
                0.25,
                8,
                c(0.15, 0.15, 0.18),
            );
        }
        Zone::Scorchsand => {
            // A skull totem.
            b.cylinder(p, Vec3::Y * 2.8, 0.15, 6, WOOD);
            b.sphere(p + Vec3::Y * 2.9, 0.3, BONE);
            for sx in [-1.0, 1.0] {
                b.cone(
                    p + vec3(0.2 * sx, 3.0, 0.0),
                    vec3(0.4 * sx, 0.4, 0.0),
                    0.06,
                    0.0,
                    5,
                    BONE,
                );
            }
            b.block(
                p + Vec3::Y * 2.0,
                vec3(0.5, 0.25, 0.05),
                0.0,
                c(0.7, 0.25, 0.15),
            );
        }
        Zone::Grubdeep => {
            // A scrap-metal robot.
            let metal = c(0.5, 0.48, 0.45);
            b.block(p + Vec3::Y * 1.0, vec3(0.4, 0.5, 0.3), 0.3, metal);
            b.block(
                p + Vec3::Y * 1.75,
                vec3(0.25, 0.25, 0.25),
                0.3,
                dark(metal, 1.1),
            );
            b.glow_sphere(
                p + Vec3::Y * 1.8 + forward(0.3) * 0.26,
                0.08,
                c(1.0, 0.3, 0.2),
            );
            b.cylinder(p, Vec3::Y * 0.5, 0.1, 6, metal);
        }
        Zone::Silverbough => {
            // A marble statue on a plinth.
            let marble = c(0.88, 0.88, 0.92);
            b.block(p + Vec3::Y * 0.4, vec3(0.6, 0.4, 0.6), 0.0, marble);
            b.cone(p + Vec3::Y * 0.8, Vec3::Y * 1.6, 0.35, 0.2, 8, marble);
            b.sphere(p + Vec3::Y * 2.6, 0.22, marble);
        }
        Zone::Amberfall | Zone::Witherwood => {
            let wood = c(0.45, 0.32, 0.2);
            b.cylinder(p, Vec3::Y * 2.6, 0.08, 6, wood);
            b.block(p + Vec3::Y * 1.9, vec3(0.9, 0.06, 0.06), 0.4, wood);
            b.block(
                p + Vec3::Y * 1.75,
                vec3(0.38, 0.4, 0.18),
                0.4,
                c(0.45, 0.25, 0.4),
            );
            let head = if zone == Zone::Witherwood {
                BONE
            } else {
                c(0.9, 0.78, 0.5)
            };
            b.sphere(p + Vec3::Y * 2.45, 0.28, head);
            b.cone(
                p + Vec3::Y * 2.6,
                Vec3::Y * 0.5,
                0.45,
                0.0,
                10,
                c(0.72, 0.6, 0.3),
            );
        }
    }
}

pub(super) fn market_stall(b: &mut Batch, zone: Zone, p: Vec3, yaw: f32, v: u8) {
    let wood = c(0.45, 0.31, 0.19);
    let f = forward(yaw);
    let r = vec3(-f.z, 0.0, f.x);
    let awning = match zone {
        Zone::Scorchsand => c(0.75, 0.3, 0.15),
        Zone::Silverbough => c(0.3, 0.45, 0.75),
        Zone::Grubdeep => c(0.55, 0.45, 0.2),
        Zone::Frostcog => c(0.8, 0.2, 0.2),
        Zone::Witherwood => c(0.35, 0.2, 0.4),
        Zone::Amberfall if v.is_multiple_of(2) => c(0.75, 0.25, 0.15),
        Zone::Amberfall => c(0.85, 0.6, 0.15),
    };
    b.block(p + Vec3::Y * 0.5, vec3(1.4, 0.5, 0.6), yaw, wood);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        b.block(
            p + r * sx * 1.35 + f * sz * 0.55 + Vec3::Y * 1.2,
            vec3(0.06, 1.2, 0.06),
            yaw,
            wood,
        );
    }
    for k in 0..6 {
        let x = -1.5 + k as f32 * 0.5 + 0.25;
        let col = if k % 2 == 0 {
            awning
        } else {
            c(0.92, 0.88, 0.8)
        };
        let a = p + r * (x - 0.25) + Vec3::Y * 2.45 - f * 0.7;
        let bb = p + r * (x + 0.25) + Vec3::Y * 2.45 - f * 0.7;
        let cc = p + r * (x + 0.25) + Vec3::Y * 2.1 + f * 0.9;
        let d = p + r * (x - 0.25) + Vec3::Y * 2.1 + f * 0.9;
        b.quad([a, bb, cc, d], (Vec3::Y + f * 0.3).normalize(), col);
    }
    // Wares: potions and produce.
    for k in 0..4 {
        let col = if k % 2 == 0 {
            c(0.85, 0.15, 0.15)
        } else {
            c(0.2, 0.35, 0.95)
        };
        let at = p + r * (-0.9 + k as f32 * 0.6) + Vec3::Y * 1.1;
        b.cylinder(at, Vec3::Y * 0.25, 0.1, 6, col);
        b.cylinder(
            at + Vec3::Y * 0.25,
            Vec3::Y * 0.1,
            0.04,
            5,
            c(0.85, 0.85, 0.8),
        );
    }
    for k in 0..5 {
        b.sphere(
            p + r * (-1.0 + k as f32 * 0.5) + f * 0.35 + Vec3::Y * 1.05,
            0.1,
            c(0.8, 0.15, 0.1),
        );
    }
}
