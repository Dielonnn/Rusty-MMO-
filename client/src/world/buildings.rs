//! Houses and each town's centerpiece.

use macroquad::prelude::*;
use shared::world::*;

use super::props::lamp_color;
use super::*;
use crate::models::{BONE, GOLD, WOOD};

pub(super) fn house(
    b: &mut Batch,
    zone: Zone,
    center: Vec3,
    yaw: f32,
    v: u8,
    chimneys: &mut Vec<Vec3>,
    lamps: &mut Vec<Glow>,
) {
    let f = forward(yaw);
    let r = vec3(-f.z, 0.0, f.x);
    let lamp = lamp_color(zone);
    match zone {
        Zone::Scorchsand | Zone::Blightscar => {
            // A round clay hut with a hide roof, bone spikes and a banner.
            let clay = [c(0.72, 0.45, 0.3), c(0.66, 0.4, 0.28), c(0.76, 0.5, 0.34)][v as usize % 3];
            b.cone_ref(center, Vec3::Y * 2.8, f, 2.8, 2.6, 12, clay);
            b.cone_ref(
                center + Vec3::Y * 2.7,
                Vec3::Y * 2.4,
                f,
                3.3,
                0.0,
                12,
                c(0.5, 0.36, 0.24),
            );
            for k in 0..6 {
                let a = k as f32 * 1.047 + 0.5;
                let base = center + vec3(a.cos(), 0.0, a.sin()) * 3.0 + Vec3::Y * 3.1;
                b.cone(
                    base,
                    vec3(a.cos() * 0.6, 0.7, a.sin() * 0.6),
                    0.1,
                    0.0,
                    5,
                    BONE,
                );
            }
            let front = center + f * 2.75;
            b.block(
                front + Vec3::Y * 1.0,
                vec3(0.6, 1.0, 0.05),
                yaw,
                c(0.25, 0.15, 0.1),
            );
            b.block(
                front + r * 1.3 + Vec3::Y * 2.0 + f * 0.1,
                vec3(0.3, 0.6, 0.02),
                yaw,
                c(0.75, 0.2, 0.12),
            );
            lamps.push(Glow {
                pos: front - r * 1.2 + Vec3::Y * 2.2 + f * 0.3,
                color: lamp,
            });
        }
        Zone::Silverbough => {
            // A slender white tower-house with a blue spire.
            let marble = c(0.9, 0.9, 0.94);
            b.cone_ref(center, Vec3::Y * 5.5, f, 2.4, 2.1, 10, marble);
            b.cone_ref(center + Vec3::Y * 5.5, Vec3::Y * 0.3, f, 2.6, 2.6, 10, GOLD);
            b.cone_ref(
                center + Vec3::Y * 5.8,
                Vec3::Y * 3.6,
                f,
                2.6,
                0.0,
                10,
                c(0.25, 0.45, 0.75),
            );
            b.glow_sphere(center + Vec3::Y * 9.5, 0.2, c(0.75, 0.9, 1.0));
            let front = center + f * 2.3;
            b.lit(|b| {
                b.block(
                    front + Vec3::Y * 1.1,
                    vec3(0.55, 1.1, 0.05),
                    yaw,
                    c(0.55, 0.75, 1.0),
                );
                for k in 0..3 {
                    let a = yaw + (k as f32 - 1.0) * 1.2;
                    b.block(
                        center + forward(a) * 2.25 + Vec3::Y * 3.8,
                        vec3(0.25, 0.5, 0.05),
                        a,
                        lamp,
                    );
                }
            });
        }
        Zone::Grubdeep => {
            // A ramshackle shack of planks and scrap with a smoking pipe.
            let plank =
                [c(0.45, 0.33, 0.22), c(0.4, 0.3, 0.22), c(0.5, 0.38, 0.25)][v as usize % 3];
            b.block(center + Vec3::Y * 1.7, vec3(3.0, 1.7, 2.4), yaw, plank);
            for k in 0..5 {
                let at = center + r * (-2.4 + k as f32 * 1.2) + Vec3::Y * 1.7 + f * 2.42;
                b.block(at, vec3(0.05, 1.7, 0.02), yaw, dark(plank, 0.8));
            }
            // A sloped tin roof.
            let tin = c(0.55, 0.52, 0.48);
            let low = center + Vec3::Y * 3.4 + f * 2.8;
            let high = center + Vec3::Y * 4.4 - f * 2.8;
            b.quad(
                [low - r * 3.3, low + r * 3.3, high + r * 3.3, high - r * 3.3],
                (Vec3::Y + f * 0.18).normalize(),
                tin,
            );
            for k in 0..7 {
                let x = -3.0 + k as f32;
                b.beam(
                    low + r * x + Vec3::Y * 0.03,
                    high + r * x + Vec3::Y * 0.03,
                    0.04,
                    Vec3::Y,
                    dark(tin, 0.8),
                );
            }
            let pipe = center + r * 2.0 - f * 1.0;
            b.cylinder(
                pipe + Vec3::Y * 3.5,
                Vec3::Y * 2.5,
                0.25,
                8,
                c(0.35, 0.33, 0.32),
            );
            chimneys.push(pipe + Vec3::Y * 6.0);
            let front = center + f * 2.42;
            b.block(
                front + Vec3::Y * 1.0,
                vec3(0.6, 1.0, 0.04),
                yaw,
                c(0.3, 0.22, 0.15),
            );
            b.lit(|b| {
                b.block(
                    front + r * 1.8 + Vec3::Y * 2.1,
                    vec3(0.45, 0.35, 0.04),
                    yaw,
                    lamp,
                )
            });
            lamps.push(Glow {
                pos: front - r * 1.4 + Vec3::Y * 2.6 + f * 0.3,
                color: lamp,
            });
        }
        Zone::Frostcog => {
            // A round gnome cottage with a snowy dome, a big gear and a chimney.
            let wall = [c(0.85, 0.3, 0.25), c(0.95, 0.75, 0.3), c(0.35, 0.55, 0.8)][v as usize % 3];
            b.cone_ref(center, Vec3::Y * 2.6, f, 2.8, 2.8, 12, wall);
            b.ellipsoid_axes(
                center + Vec3::Y * 2.6,
                [r * 3.0, Vec3::Y * 2.2, f * 3.0],
                c(0.95, 0.97, 1.0),
            );
            let gear = center - r * 2.85 + Vec3::Y * 1.6;
            let brass = c(0.7, 0.6, 0.3);
            b.cone_ref(gear, -r * 0.2, Vec3::Y, 1.1, 1.1, 10, brass);
            for k in 0..8 {
                let a = k as f32 * 0.785;
                let d = Vec3::Y * a.sin() + f * a.cos();
                b.beam(gear - r * 0.1, gear - r * 0.1 + d * 1.35, 0.12, r, brass);
            }
            let chim = center + r * 1.2 - f * 0.8;
            b.cylinder(
                chim + Vec3::Y * 3.5,
                Vec3::Y * 2.2,
                0.3,
                8,
                c(0.5, 0.45, 0.42),
            );
            chimneys.push(chim + Vec3::Y * 5.7);
            let front = center + f * 2.75;
            b.block(
                front + Vec3::Y * 0.9,
                vec3(0.55, 0.9, 0.06),
                yaw,
                c(0.4, 0.26, 0.15),
            );
            b.lit(|b| {
                for sx in [-1.0, 1.0] {
                    b.block(
                        front + r * sx * 1.5 - f * 0.25 + Vec3::Y * 1.6,
                        vec3(0.35, 0.35, 0.05),
                        yaw,
                        lamp,
                    );
                }
            });
            lamps.push(Glow {
                pos: front + r * 1.0 + Vec3::Y * 2.3 + f * 0.3,
                color: lamp,
            });
        }
        Zone::Witherwood => {
            // A dark stone house with a steep roof and eerie windows.
            let stone =
                [c(0.36, 0.34, 0.36), c(0.32, 0.3, 0.33), c(0.4, 0.37, 0.38)][v as usize % 3];
            b.block(center + Vec3::Y * 2.0, vec3(3.0, 2.0, 2.4), yaw, stone);
            let base_y = 4.0;
            let top = center + Vec3::Y * (base_y + 3.2);
            let corner = |sx: f32, sz: f32| center + r * sx * 3.3 + f * sz * 2.7 + Vec3::Y * base_y;
            let ridge = |sx: f32| top + r * sx * 3.3;
            let roof = c(0.18, 0.14, 0.2);
            for sz in [-1.0, 1.0] {
                let n = (f * sz * 3.2 + Vec3::Y * 2.7).normalize();
                b.quad(
                    [corner(-1.0, sz), corner(1.0, sz), ridge(1.0), ridge(-1.0)],
                    n,
                    roof,
                );
            }
            for sx in [-1.0, 1.0] {
                b.triangle(corner(sx, -1.0), corner(sx, 1.0), ridge(sx), stone);
            }
            let chim = center + r * 2.0 - f * 0.6;
            b.block(
                chim + Vec3::Y * (base_y + 1.8),
                vec3(0.35, 1.8, 0.35),
                yaw,
                stone,
            );
            chimneys.push(chim + Vec3::Y * (base_y + 3.6));
            let front = center + f * 2.42;
            b.block(
                front + Vec3::Y * 1.1,
                vec3(0.6, 1.1, 0.04),
                yaw,
                c(0.15, 0.1, 0.1),
            );
            b.lit(|b| {
                for sx in [-1.0, 1.0] {
                    b.block(
                        front + r * sx * 1.9 + Vec3::Y * 2.4,
                        vec3(0.35, 0.5, 0.04),
                        yaw,
                        c(0.65, 0.45, 0.9),
                    );
                }
            });
            lamps.push(Glow {
                pos: front + r * 1.0 + Vec3::Y * 2.6 + f * 0.3,
                color: lamp,
            });
        }
        Zone::Amberfall | Zone::Sunfold => {
            let walls = [c(0.9, 0.84, 0.7), c(0.86, 0.78, 0.66), c(0.93, 0.88, 0.78)];
            let roofs = [c(0.62, 0.22, 0.15), c(0.3, 0.33, 0.45), c(0.48, 0.3, 0.18)];
            let i = v as usize % 3;
            timber_house(b, center, yaw, walls[i], roofs[i], chimneys, lamps, lamp);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn timber_house(
    b: &mut Batch,
    center: Vec3,
    yaw: f32,
    wall: Color,
    roof: Color,
    chimneys: &mut Vec<Vec3>,
    lamps: &mut Vec<Glow>,
    lamp: Color,
) {
    let half = vec3(3.0, 1.8, 2.4);
    let f = forward(yaw);
    let r = vec3(-f.z, 0.0, f.x);
    b.block(
        center + Vec3::Y * 0.25,
        vec3(half.x + 0.15, 0.25, half.z + 0.15),
        yaw,
        c(0.5, 0.48, 0.45),
    );
    b.block(center + Vec3::Y * half.y, half, yaw, wall);
    let beam = c(0.32, 0.2, 0.11);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let p = center + r * sx * half.x + f * sz * half.z + Vec3::Y * half.y;
        b.block(p, vec3(0.15, half.y, 0.15), yaw, beam);
    }
    b.block(
        center + Vec3::Y * 1.9 + f * (half.z + 0.03),
        vec3(half.x, 0.08, 0.04),
        yaw,
        beam,
    );
    let eave = 0.45;
    let top = center + Vec3::Y * (half.y * 2.0 + 1.9);
    let base_y = half.y * 2.0;
    let corner = |sx: f32, sz: f32| {
        center + r * sx * (half.x + eave) + f * sz * (half.z + eave) + Vec3::Y * base_y
    };
    let ridge = |sx: f32| top + r * sx * (half.x + eave);
    for sz in [-1.0, 1.0] {
        let n = (f * sz * 1.9 + Vec3::Y * (half.z + eave)).normalize();
        b.quad(
            [corner(-1.0, sz), corner(1.0, sz), ridge(1.0), ridge(-1.0)],
            n,
            roof,
        );
        for k in 1..4 {
            let t = k as f32 / 4.0;
            let a = corner(-1.0, sz).lerp(ridge(-1.0), t) + n * 0.03;
            let cc = corner(1.0, sz).lerp(ridge(1.0), t) + n * 0.03;
            let d = (ridge(-1.0) - corner(-1.0, sz)).normalize() * 0.06;
            b.quad([a, cc, cc + d, a + d], n, dark(roof, 0.8));
        }
    }
    for sx in [-1.0, 1.0] {
        b.triangle(corner(sx, -1.0), corner(sx, 1.0), ridge(sx), wall);
    }
    b.block(
        top,
        vec3(half.x + eave + 0.1, 0.08, 0.12),
        yaw,
        dark(roof, 0.7),
    );
    let chim = center + r * (half.x - 0.8) - f * 0.6;
    b.block(
        chim + Vec3::Y * (base_y + 1.6),
        vec3(0.35, 1.6, 0.35),
        yaw,
        c(0.48, 0.4, 0.36),
    );
    chimneys.push(chim + Vec3::Y * (base_y + 3.3));
    let front = center + f * (half.z + 0.02);
    b.block(
        front + Vec3::Y * 1.05,
        vec3(0.65, 1.05, 0.04),
        yaw,
        c(0.36, 0.22, 0.12),
    );
    b.block(
        front + Vec3::Y * 1.05 + r * 0.4 + f * 0.04,
        vec3(0.06, 0.06, 0.03),
        yaw,
        c(0.8, 0.65, 0.25),
    );
    for sx in [-1.0, 1.0] {
        let wp = front + r * sx * 1.9 + Vec3::Y * 2.2;
        b.block(wp, vec3(0.5, 0.45, 0.04), yaw, beam);
        b.lit(|b| b.block(wp + f * 0.02, vec3(0.4, 0.36, 0.04), yaw, c(1.0, 0.78, 0.4)));
        b.block(wp + f * 0.05, vec3(0.03, 0.36, 0.03), yaw, beam);
        for so in [-1.0, 1.0] {
            b.block(
                wp + r * so * 0.62 + f * 0.04,
                vec3(0.13, 0.45, 0.03),
                yaw,
                c(0.3, 0.45, 0.35),
            );
        }
    }
    let lamp_pos = front + r * 1.0 + Vec3::Y * 2.3 + f * 0.3;
    b.block(lamp_pos + Vec3::Y * 0.3, vec3(0.03, 0.2, 0.2), yaw, beam);
    lamps.push(Glow {
        pos: lamp_pos,
        color: lamp,
    });
}

pub(super) fn centerpiece(
    b: &mut Batch,
    zone: Zone,
    p: Vec3,
    fires: &mut Vec<Glow>,
    lamps: &mut Vec<Glow>,
) {
    match zone {
        Zone::Amberfall | Zone::Sunfold => {
            // A fountain.
            let stone = c(0.62, 0.6, 0.57);
            b.cylinder(p, Vec3::Y * 0.7, 3.0, 20, stone);
            b.cylinder(
                p + Vec3::Y * 0.7,
                Vec3::Y * 0.12,
                3.15,
                20,
                dark(stone, 0.85),
            );
            b.cylinder(
                p + Vec3::Y * 0.72,
                Vec3::Y * 0.02,
                2.7,
                20,
                c(0.3, 0.42, 0.6),
            );
            b.cylinder(p, Vec3::Y * 2.2, 0.35, 10, stone);
            b.cylinder(p + Vec3::Y * 2.2, Vec3::Y * 0.25, 1.0, 14, stone);
            b.sphere(p + Vec3::Y * 2.75, 0.35, c(0.55, 0.7, 0.9));
        }
        Zone::Scorchsand | Zone::Blightscar => {
            // A great fire pit ringed with stones, beside a tusked totem.
            for k in 0..14 {
                let a = k as f32 * 0.449;
                b.sphere(
                    p + vec3(a.cos() * 2.8, 0.3, a.sin() * 2.8),
                    0.5,
                    c(0.55, 0.4, 0.3),
                );
            }
            b.cylinder(p, Vec3::Y * 0.2, 2.6, 16, c(0.25, 0.18, 0.15));
            fires.push(Glow {
                pos: p + Vec3::Y * 0.2,
                color: c(1.0, 0.45, 0.1),
            });
            for a in [0.0f32, 2.1, 4.2] {
                b.block(p + Vec3::Y * 0.4, vec3(1.6, 0.18, 0.18), a, WOOD);
            }
            let pole = p + vec3(0.0, 0.0, 3.6);
            b.cylinder(pole, Vec3::Y * 6.0, 0.25, 8, WOOD);
            for sx in [-1.0, 1.0] {
                b.cone(
                    pole + vec3(0.2 * sx, 5.0, 0.0),
                    vec3(1.0 * sx, 1.4, 0.0),
                    0.15,
                    0.0,
                    6,
                    BONE,
                );
            }
            b.sphere(pole + Vec3::Y * 6.2, 0.5, BONE);
        }
        Zone::Silverbough => {
            // A moonwell: a glowing pool under a white arch.
            let marble = c(0.9, 0.9, 0.94);
            b.cylinder(p, Vec3::Y * 0.7, 3.0, 20, marble);
            b.lit(|b| {
                b.cylinder(
                    p + Vec3::Y * 0.72,
                    Vec3::Y * 0.02,
                    2.7,
                    20,
                    c(0.55, 0.8, 1.0),
                )
            });
            for sx in [-1.0, 1.0] {
                b.cylinder(
                    p + vec3(2.6 * sx, 0.0, -2.0),
                    Vec3::Y * 5.0,
                    0.3,
                    10,
                    marble,
                );
            }
            b.beam(
                p + vec3(-2.9, 5.0, -2.0),
                p + vec3(2.9, 5.0, -2.0),
                0.3,
                Vec3::Y,
                marble,
            );
            lamps.push(Glow {
                pos: p + Vec3::Y * 2.0,
                color: Color::new(0.6, 0.85, 1.0, 0.6),
            });
        }
        Zone::Grubdeep => {
            // A huge glowing crystal on a machine base.
            b.cylinder(p, Vec3::Y * 1.0, 3.0, 12, c(0.4, 0.38, 0.35));
            for k in 0..6 {
                let a = k as f32 * 1.047;
                b.cylinder(
                    p + vec3(a.cos() * 2.6, 1.0, a.sin() * 2.6),
                    Vec3::Y * 0.8,
                    0.2,
                    6,
                    c(0.6, 0.45, 0.2),
                );
            }
            b.lit(|b| {
                b.cone(
                    p + Vec3::Y * 1.0,
                    Vec3::Y * 5.0,
                    1.2,
                    0.0,
                    6,
                    c(0.45, 1.0, 0.6),
                );
                b.cone(
                    p + vec3(1.0, 1.0, 0.5),
                    vec3(0.6, 3.0, 0.3),
                    0.6,
                    0.0,
                    6,
                    c(0.45, 1.0, 0.6),
                );
            });
            lamps.push(Glow {
                pos: p + Vec3::Y * 3.0,
                color: Color::new(0.45, 1.0, 0.6, 0.6),
            });
        }
        Zone::Frostcog => {
            // A clockwork tower with a great gear.
            b.cylinder(p, Vec3::Y * 6.0, 1.2, 10, c(0.65, 0.55, 0.35));
            b.cone(
                p + Vec3::Y * 6.0,
                Vec3::Y * 2.0,
                1.5,
                0.0,
                10,
                c(0.8, 0.25, 0.2),
            );
            let gear = p + Vec3::Y * 4.0 + Vec3::Z * 1.3;
            let brass = c(0.75, 0.65, 0.3);
            b.cone_ref(gear, Vec3::Z * 0.25, Vec3::Y, 1.4, 1.4, 12, brass);
            for k in 0..10 {
                let a = k as f32 * 0.628;
                b.beam(
                    gear,
                    gear + vec3(a.cos(), a.sin(), 0.0) * 1.7,
                    0.15,
                    Vec3::Z,
                    brass,
                );
            }
            b.glow_sphere(gear + Vec3::Z * 0.3, 0.3, c(1.0, 0.85, 0.45));
        }
        Zone::Witherwood => {
            // A black obelisk with a green flame.
            b.cylinder(p, Vec3::Y * 0.6, 2.8, 8, c(0.22, 0.2, 0.24));
            b.cone_ref(
                p + Vec3::Y * 0.6,
                Vec3::Y * 7.0,
                Vec3::X,
                0.9,
                0.3,
                4,
                c(0.12, 0.1, 0.14),
            );
            fires.push(Glow {
                pos: p + Vec3::Y * 7.5,
                color: c(0.4, 1.0, 0.3),
            });
        }
    }
}
