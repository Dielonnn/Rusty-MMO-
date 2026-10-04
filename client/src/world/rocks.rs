//! Rounded, lumpy boulders with their light and shade painted into their
//! colors: darker in the hollows and underneath, lighter on the bumps, and
//! moss or snow on top.

use macroquad::prelude::*;

use crate::gfx::{Batch, mix};

/// Smooth 3D noise in `0..1`.
fn noise3(p: Vec3, seed: u32) -> f32 {
    let lattice = |x: i32, y: i32, z: i32| {
        let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
            ^ (y as u32).wrapping_mul(0xD816_3841)
            ^ (z as u32).wrapping_mul(0xCB1A_B31F)
            ^ seed.wrapping_mul(0x9E37_79B9);
        h ^= h >> 13;
        h = h.wrapping_mul(0x5BD1_E995);
        h ^= h >> 15;
        (h & 0xFFFF) as f32 / 65535.0
    };
    let f = p.floor();
    let (i, j, k) = (f.x as i32, f.y as i32, f.z as i32);
    let t = p - f;
    let s = t * t * (Vec3::splat(3.0) - 2.0 * t);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let face = |dk: i32| {
        let a = lerp(lattice(i, j, k + dk), lattice(i + 1, j, k + dk), s.x);
        let b = lerp(
            lattice(i, j + 1, k + dk),
            lattice(i + 1, j + 1, k + dk),
            s.x,
        );
        lerp(a, b, s.y)
    };
    lerp(face(0), face(1), s.z)
}

/// How far out the boulder's surface is in direction `d`, about 1.
fn bulge(d: Vec3, seed: u32) -> f32 {
    1.0 + (noise3(d * 1.6, seed) - 0.5) * 0.45 + (noise3(d * 4.2, seed ^ 0x77) - 0.5) * 0.14
}

/// What the boulder's surface looks like.
pub(super) struct Stone {
    pub color: Color,
    /// Moss, lichen or snow on the upward faces.
    pub top: Option<Color>,
    /// Detail: rings around and up the boulder.
    pub slices: usize,
    pub stacks: usize,
}

/// A boulder centered at `center`, sunk into the ground at its foot, with
/// half-sizes `radii`, turned by `yaw`.
pub(super) fn boulder(b: &mut Batch, center: Vec3, radii: Vec3, yaw: f32, seed: u32, s: &Stone) {
    let (slices, stacks) = (s.slices, s.stacks);
    let (sn, cs) = yaw.sin_cos();
    // The lowest the bottom goes before it flattens (into the ground).
    let floor = -0.45;
    let point = |phi: f32, theta: f32| {
        let d = vec3(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
        let r = bulge(d, seed);
        let mut l = d * r;
        l.y = l.y.max(floor);
        let l = l * radii;
        (
            center + vec3(l.x * cs - l.z * sn, l.y, l.x * sn + l.z * cs),
            r,
        )
    };
    let base = b.begin_shape((slices + 1) * (stacks + 1), slices * stacks * 6);
    for j in 0..=stacks {
        let phi = std::f32::consts::PI * j as f32 / stacks as f32;
        for i in 0..=slices {
            let theta = std::f32::consts::TAU * i as f32 / slices as f32;
            let (p, r) = point(phi, theta);
            // The normal, from the surface's slope around this point.
            let e = 0.05;
            let along = point((phi + e).min(std::f32::consts::PI), theta).0
                - point((phi - e).max(0.0), theta).0;
            let around = point(phi, theta + e).0 - point(phi, theta - e).0;
            let mut n = around.cross(along);
            let outward = p - center;
            if n.length_squared() < 1e-8 {
                n = outward;
            } else if n.dot(outward) < 0.0 {
                n = -n;
            }
            let n = n.normalize_or_zero();
            // Painted light and shade: hollows and the underside dark,
            // bumps light.
            let y = (p.y - center.y) / radii.y.max(0.01);
            let occlusion = 0.62 + 0.38 * ((y + 0.45) / 1.2).clamp(0.0, 1.0);
            let bump = 0.85 + (r - 1.0) * 0.9;
            let mut col = Color::new(
                s.color.r * occlusion * bump,
                s.color.g * occlusion * bump,
                s.color.b * occlusion * bump,
                1.0,
            );
            if let Some(top) = s.top {
                let patchy = noise3(p * 0.9, seed ^ 0x1234) * 0.35;
                let cover = ((n.y - 0.55 + patchy) * 4.0).clamp(0.0, 1.0);
                col = mix(col, top, cover);
            }
            b.vertex_uv(p, Vec2::ZERO, n, col);
        }
    }
    let row = slices as u16 + 1;
    for j in 0..stacks as u16 {
        for i in 0..slices as u16 {
            let a = base + j * row + i;
            let c = a + row;
            b.index(&[a, c, a + 1, a + 1, c, c + 1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boulders_are_lumpy_but_whole() {
        let mut b = Batch::recording();
        let s = Stone {
            color: WHITE,
            top: None,
            slices: 12,
            stacks: 8,
        };
        boulder(&mut b, Vec3::ZERO, Vec3::splat(2.0), 0.4, 7, &s);
        let meshes = b.finish();
        let v = &meshes[0].vertices;
        let dist: Vec<f32> = v.iter().map(|v| v.position.length()).collect();
        let (lo, hi) = dist
            .iter()
            .fold((f32::MAX, 0.0f32), |(lo, hi), &d| (lo.min(d), hi.max(d)));
        // Rounded and roughly the size asked for, with some lumps...
        assert!(hi < 3.0 && hi > 1.8, "{hi}");
        assert!(hi - lo > 0.2);
        // ...flat underneath, where it sits in the ground...
        assert!(v.iter().all(|v| v.position.y >= -0.45 * 2.0 * 1.0001));
        // ...and every normal points outward.
        for v in v {
            let n = v.normal.truncate();
            assert!((n.length() - 1.0).abs() < 1e-3);
            assert!(n.dot(v.position) > -0.2, "{n} at {}", v.position);
        }
    }
}
