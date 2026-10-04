//! Foliage made of painted, cut-out cards: tree canopies, bushes, grass,
//! ferns and flowers. Cards are drawn into their own batch, which draws
//! with the foliage atlas (see `paint::foliage`).

use macroquad::prelude::*;
use shared::props::Scatter;
use shared::world::terrain_height;

use super::paint::Card;
use crate::gfx::{Batch, dark};

/// A card centered at `center`, spanning `right` and `up` either side of
/// it. Each corner's normal points away from `light_from`, so many cards
/// around one point shade together like a single round shape.
fn card(
    cards: &mut Batch,
    center: Vec3,
    right: Vec3,
    up: Vec3,
    light_from: Vec3,
    kind: Card,
    color: Color,
) {
    let corners = [
        center - right - up,
        center + right - up,
        center + right + up,
        center - right + up,
    ];
    let base = cards.begin_shape(4, 6);
    for (p, uv) in corners.into_iter().zip(kind.uvs()) {
        cards.vertex_uv(p, uv, p - light_from, color);
    }
    cards.index(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// A card standing on the ground at `foot`, `width` wide and `height` tall,
/// facing along `facing`, lit as if it were the ground (so it blends in).
fn standing(
    cards: &mut Batch,
    foot: Vec3,
    facing: f32,
    width: f32,
    height: f32,
    kind: Card,
    color: Color,
) {
    let right = vec3(facing.cos(), 0.0, facing.sin()) * width * 0.5;
    let up = Vec3::Y * height * 0.5;
    // Lit from far below, so every corner faces (almost) straight up.
    card(
        cards,
        foot + up - Vec3::Y * 0.04,
        right,
        up,
        foot - Vec3::Y * 1000.0,
        kind,
        color,
    );
}

/// Two cards crossed at right angles, standing on the ground.
pub(super) fn tuft(
    cards: &mut Batch,
    rng: &mut Scatter,
    foot: Vec3,
    size: f32,
    kind: Card,
    color: Color,
) {
    let facing = rng.range(0.0, std::f32::consts::PI);
    let width = size * rng.range(0.85, 1.15);
    let height = size * rng.range(0.7, 1.0);
    for k in 0..2 {
        standing(
            cards,
            foot,
            facing + k as f32 * std::f32::consts::FRAC_PI_2,
            width,
            height,
            kind,
            color,
        );
    }
}

/// A leafy canopy: a dark core, so it can't be seen through, wrapped in
/// leaf cards at all angles, shaded as one round clump.
pub(super) fn canopy(
    b: &mut Batch,
    cards: &mut Batch,
    rng: &mut Scatter,
    center: Vec3,
    radii: Vec3,
    color: Color,
    count: usize,
) {
    b.ellipsoid(center, radii * 0.72, dark(color, 0.6));
    for k in 0..count {
        // Spread evenly over the clump (a spiral down a sphere).
        let t = (k as f32 + 0.5) / count as f32;
        let y = 1.0 - 2.0 * t;
        let ring = (1.0 - y * y).sqrt();
        let a = k as f32 * 2.399_963 + rng.range(-0.3, 0.3);
        let d = vec3(a.cos() * ring, y, a.sin() * ring);
        let at = center + d * radii * rng.range(0.7, 0.9);
        // Each card faces a random way, tipped a little off upright.
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        let right = vec3(yaw.cos(), rng.range(-0.3, 0.3), yaw.sin()).normalize();
        let up = (Vec3::Y + d * 0.4 + right.cross(Vec3::Y) * rng.range(-0.4, 0.4))
            .normalize()
            .reject_from(right)
            .normalize();
        let size = radii.x.max(radii.z) * rng.range(0.6, 0.8);
        let tone = rng.range(0.9, 1.1);
        card(
            cards,
            at,
            right * size,
            up * size,
            center - Vec3::Y * radii.y * 0.3,
            Card::Leaves,
            dark(color, tone),
        );
    }
}

/// Fern fronds fanning out of the ground around `foot`.
pub(super) fn ferns(cards: &mut Batch, rng: &mut Scatter, foot: Vec3, size: f32, color: Color) {
    for k in 0..3 {
        let facing = k as f32 * 1.05 + rng.range(0.0, 0.5);
        standing(cards, foot, facing, size * 1.3, size, Card::Fern, color);
    }
}

/// A bush: a rounded clump of leaves sitting on the ground, with a few
/// fern fronds at its foot.
pub(super) fn bush(
    b: &mut Batch,
    cards: &mut Batch,
    rng: &mut Scatter,
    foot: Vec3,
    size: f32,
    color: Color,
) {
    canopy(
        b,
        cards,
        rng,
        foot + Vec3::Y * 0.55 * size,
        vec3(0.85, 0.6, 0.85) * size,
        color,
        12,
    );
    ferns(cards, rng, foot, size * 0.9, dark(color, 0.9));
}

/// Leafy, drooping fringes around a pine's tiers, which soften its cones
/// into boughs. `tiers` is each tier's height above `foot`, radius and
/// height.
pub(super) fn bough_fringe(
    cards: &mut Batch,
    rng: &mut Scatter,
    foot: Vec3,
    tiers: &[(f32, f32, f32)],
    color: Color,
) {
    for &(y, r, h) in tiers {
        let count = (r * 6.0) as usize + 4;
        for k in 0..count {
            let a = std::f32::consts::TAU * k as f32 / count as f32 + rng.range(-0.2, 0.2);
            let out = vec3(a.cos(), 0.0, a.sin());
            // Hanging from the cone's edge, leaning out and down.
            let at = foot + Vec3::Y * (y + h * 0.18) + out * r * 0.92;
            let right = vec3(-a.sin(), 0.0, a.cos()) * r * 0.45;
            let up = (Vec3::Y * 0.8 - out * 0.6).normalize() * h * 0.32;
            card(
                cards,
                at,
                right,
                up,
                foot + Vec3::Y * (y - h),
                Card::Fern,
                dark(color, rng.range(0.9, 1.15)),
            );
        }
    }
}

/// A few flower heads standing in a grass tuft.
pub(super) fn flowers(
    cards: &mut Batch,
    rng: &mut Scatter,
    foot: Vec3,
    grass: Color,
    bloom: Color,
) {
    tuft(cards, rng, foot, 0.7, Card::Grass, grass);
    for _ in 0..2 {
        let o = vec3(rng.range(-0.25, 0.25), 0.0, rng.range(-0.25, 0.25));
        let at = foot + o;
        let at = vec3(at.x, terrain_height(at.x, at.z) + 0.25, at.z);
        standing(
            cards,
            at,
            rng.range(0.0, std::f32::consts::PI),
            0.35,
            0.35,
            Card::Blossoms,
            bloom,
        );
    }
}
