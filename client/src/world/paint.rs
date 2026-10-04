//! The world's painted textures, all generated here rather than loaded from
//! files: each zone's ground, the foliage cards trees and grass are made
//! of, and the clouds.
//!
//! Like hand-painted MMO textures, they're mostly light and shade (the
//! color comes from the zone's palette, which tints them), with the light
//! painted in: leaves are brighter at the top, grass blades at their tips,
//! pebbles on the side facing the sun.

use macroquad::prelude::*;
use shared::world::Zone;

use crate::gfx::GroundPaint;
use crate::gfx::texture::{atlas, byte, from_pixels, hash, tile_cells, tile_noise};

const GROUND_SIZE: usize = 256;

/// What covers the ground between the dirt and rock.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Top {
    Grass,
    Sand,
    Snow,
    Cave,
}

fn top(zone: Zone) -> Top {
    match zone {
        Zone::Scorchsand => Top::Sand,
        Zone::Frostcog => Top::Snow,
        Zone::Grubdeep => Top::Cave,
        _ => Top::Grass,
    }
}

/// A zone's painted ground texture and how its top layer is tinted.
pub(super) fn ground(zone: Zone) -> GroundPaint {
    let top = top(zone);
    let (shadow, light) = match top {
        // Cool, slightly blue shadows and warm, yellowed highlights.
        Top::Grass => (vec3(0.78, 0.9, 0.9), vec3(1.18, 1.1, 0.84)),
        Top::Sand => (vec3(0.86, 0.82, 0.84), vec3(1.1, 1.08, 1.0)),
        Top::Snow => (vec3(0.9, 0.94, 1.03), vec3(1.05, 1.04, 1.0)),
        Top::Cave => (vec3(0.78, 0.8, 0.9), vec3(1.16, 1.08, 0.98)),
    };
    GroundPaint {
        texture: from_pixels(GROUND_SIZE, &ground_pixels(top, GROUND_SIZE)),
        shadow,
        light,
    }
}

fn ground_pixels(top: Top, size: usize) -> Vec<[u8; 4]> {
    let mut out = Vec::with_capacity(size * size);
    for j in 0..size {
        for i in 0..size {
            let (u, v) = (i as f32 / size as f32, j as f32 / size as f32);
            let n = |cells, seed| tile_noise(u, v, cells, seed);
            let r = match top {
                Top::Grass => {
                    // Soft clumps, with brush strokes running two ways.
                    let clumps = n(6, 1) * 0.5 + n(12, 2) * 0.3 + n(24, 3) * 0.2;
                    let along = tile_noise(u * 3.0, v, 96, 4);
                    let across = tile_noise(u, v * 3.0, 96, 5);
                    let strokes = along + (across - along) * n(4, 6);
                    let tufts = (n(32, 7) - 0.55).max(0.0) * 1.6;
                    0.5 + (clumps - 0.5) * 1.5 + (strokes - 0.5) * 0.55 + tufts
                        - (1.0 - tile_noise(u, v, 64, 8)).powi(6) * 0.4
                }
                Top::Sand => {
                    // Wind ripples that wander, over soft drifts.
                    let wander = n(4, 1) * 1.6 + n(8, 2) * 0.4;
                    let ripple = (std::f32::consts::TAU * (v * 14.0 + wander)).sin();
                    let drift = n(6, 3);
                    0.5 + ripple * 0.17 + (drift - 0.5) * 0.6 + (n(64, 4) - 0.5) * 0.15
                }
                Top::Snow => {
                    // Soft, low drifts and the odd glint.
                    let drift = n(4, 1) * 0.6 + n(10, 2) * 0.4;
                    let glint = if hash(i as i32, j as i32, 9) > 0.995 {
                        0.25
                    } else {
                        0.0
                    };
                    0.5 + (drift - 0.5) * 0.5 + glint
                }
                Top::Cave => {
                    // Packed earth with grit and scattered stones.
                    let (f1, _, id) = tile_cells(u, v, 20, 3);
                    let stone = if f1 < 0.28 { 0.12 + id * 0.15 } else { 0.0 };
                    0.42 + (n(8, 1) - 0.5) * 0.8 + (n(48, 2) - 0.5) * 0.25 + stone
                }
            };
            out.push([
                byte(r),
                byte(dirt(u, v)),
                byte(rock(u, v)),
                byte(0.5 + (n(2, 11) * 0.6 + n(5, 12) * 0.4 - 0.5) * 1.6),
            ]);
        }
    }
    out
}

/// Packed dirt: mottled, with pebbles lit from the top left.
fn dirt(u: f32, v: f32) -> f32 {
    let mottle = tile_noise(u, v, 8, 21) * 0.6 + tile_noise(u, v, 24, 22) * 0.4;
    let mut d = 0.45 + (mottle - 0.5) * 0.7;
    let (f1, f2, id) = tile_cells(u, v, 24, 23);
    // Only some cells hold a pebble, and they vary in size, so they don't
    // line up in rows.
    let size = 0.12 + id * 0.16;
    if id > 0.55 && f1 < size {
        let lit = 1.0 - f1 / size;
        d += lit * 0.16 + (id - 0.7) * 0.15;
    } else if f2 - f1 < 0.05 {
        d -= 0.06;
    }
    d.clamp(0.0, 1.0)
}

/// Rock: flat facets of slightly different shades, split by dark cracks,
/// with faint layers.
fn rock(u: f32, v: f32) -> f32 {
    let (f1, f2, id) = tile_cells(u, v, 6, 31);
    let (g1, g2, _) = tile_cells(u, v, 17, 32);
    let crack = (1.0 - (f2 - f1) / 0.08).max(0.0) * 0.45 + (1.0 - (g2 - g1) / 0.05).max(0.0) * 0.2;
    let layers = (std::f32::consts::TAU * (v * 9.0 + tile_noise(u, v, 4, 33) * 1.5)).sin() * 0.06;
    let grain = (tile_noise(u, v, 40, 34) - 0.5) * 0.18;
    (0.38 + id * 0.3 + layers + grain - crack + (1.0 - f1).max(0.0) * 0.08).clamp(0.0, 1.0)
}

/// The cards of the foliage atlas, one per quarter of the texture.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Card {
    /// A clump of leaves, for tree canopies and bushes.
    Leaves,
    /// A tuft of grass blades growing from the bottom middle.
    Grass,
    /// Fern fronds fanning out from the bottom middle.
    Fern,
    /// A few small flower heads.
    Blossoms,
}

impl Card {
    /// The texture coordinates of the card's bottom-left, bottom-right,
    /// top-right and top-left corners.
    pub(super) fn uvs(self) -> [Vec2; 4] {
        let (x, y) = match self {
            Card::Leaves => (0.0, 0.0),
            Card::Grass => (0.5, 0.0),
            Card::Fern => (0.0, 0.5),
            Card::Blossoms => (0.5, 0.5),
        };
        let (x0, y0, x1, y1) = (x + 0.002, y + 0.002, x + 0.498, y + 0.498);
        [vec2(x0, y1), vec2(x1, y1), vec2(x1, y0), vec2(x0, y0)]
    }
}

const CELL: usize = 256;

/// A square of RGBA pixels being painted, `size` wide, in `0..1` values.
struct Canvas {
    size: usize,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(size: usize) -> Self {
        Self {
            size,
            px: vec![[0.0; 4]; size * size],
        }
    }

    /// Paints an ellipse centered at `(cx, cy)` (in `0..1` of the canvas)
    /// with half-lengths `a` along `angle` and `b` across it. `shade` gives
    /// the brightness at a point from its position along (`-1..1`) and
    /// across (`-1..1`) the ellipse.
    #[allow(clippy::too_many_arguments)]
    fn ellipse(
        &mut self,
        cx: f32,
        cy: f32,
        a: f32,
        b: f32,
        angle: f32,
        shade: impl Fn(f32, f32) -> f32,
    ) {
        let s = self.size as f32;
        let reach = a.max(b);
        let (x0, x1) = (
            ((cx - reach) * s).floor() as i32,
            ((cx + reach) * s).ceil() as i32,
        );
        let (y0, y1) = (
            ((cy - reach) * s).floor() as i32,
            ((cy + reach) * s).ceil() as i32,
        );
        let (sn, cs) = angle.sin_cos();
        for y in y0.max(0)..y1.min(self.size as i32) {
            for x in x0.max(0)..x1.min(self.size as i32) {
                let (dx, dy) = ((x as f32 + 0.5) / s - cx, (y as f32 + 0.5) / s - cy);
                let (along, across) = ((dx * cs + dy * sn) / a, (-dx * sn + dy * cs) / b);
                if along * along + across * across <= 1.0 {
                    let v = shade(along, across).clamp(0.0, 1.0);
                    self.px[y as usize * self.size + x as usize] = [v, v, v * 0.95, 1.0];
                }
            }
        }
    }

    /// Paints a tapering blade from `base` to `tip` (in `0..1` of the
    /// canvas), `width` wide at the base, bending sideways by `bend`.
    fn blade(&mut self, base: Vec2, tip: Vec2, width: f32, bend: f32, dark: f32, light: f32) {
        let steps = 40;
        let side = (tip - base).perp().normalize_or_zero();
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let p = base.lerp(tip, t) + side * bend * t * t;
            let w = width * (1.0 - t).max(0.08) * 0.5;
            let v = dark + (light - dark) * t;
            // Each step is a small round dab of paint along the blade.
            self.ellipse(p.x, p.y, w, w, 0.0, |_, across| v - across * 0.06);
        }
    }

    /// Paints a blossom: five rounded petals around a center.
    fn blossom(&mut self, cx: f32, cy: f32, r: f32, turn: f32) {
        for k in 0..5 {
            let a = turn + k as f32 * std::f32::consts::TAU / 5.0;
            let (px, py) = (cx + a.cos() * r * 0.55, cy + a.sin() * r * 0.55);
            self.ellipse(px, py, r * 0.55, r * 0.36, a, |along, _| {
                0.82 + (1.0 - along.abs()) * 0.18
            });
        }
        self.ellipse(cx, cy, r * 0.32, r * 0.32, 0.0, |along, across| {
            0.55 - (along + across) * 0.08
        });
    }

    /// Gives see-through pixels the average color of the painted ones, so
    /// cut-out edges don't fade to black in the distance.
    fn bleed(&mut self) {
        let painted: Vec<&[f32; 4]> = self.px.iter().filter(|p| p[3] > 0.5).collect();
        if painted.is_empty() {
            return;
        }
        let n = painted.len() as f32;
        let mean = [0, 1, 2].map(|c| painted.iter().map(|p| p[c]).sum::<f32>() / n);
        for p in &mut self.px {
            if p[3] < 0.5 {
                *p = [mean[0], mean[1], mean[2], 0.0];
            }
        }
    }
}

fn leaves(c: &mut Canvas, seed: u32) {
    let mut k = 0;
    let r = |k: &mut u32| {
        *k += 1;
        hash(*k as i32, 0, seed)
    };
    // Back leaves first, darker; front leaves last, lighter; brightest at
    // the top left, where the sun catches the clump.
    for layer in 0..3 {
        let count = [70, 60, 45][layer];
        let spread = [0.4, 0.36, 0.3][layer];
        for _ in 0..count {
            let a = r(&mut k) * std::f32::consts::TAU;
            let d = r(&mut k).sqrt() * spread;
            let (x, y) = (0.5 + a.cos() * d, 0.5 + a.sin() * d * 0.92);
            let tone = 0.5 + layer as f32 * 0.14 + (0.5 - y) * 0.45 + (0.5 - x) * 0.15;
            let tone = tone + (r(&mut k) - 0.5) * 0.12;
            let size = 0.045 + r(&mut k) * 0.03;
            let turn = r(&mut k) * std::f32::consts::TAU;
            c.ellipse(x, y, size, size * 0.5, turn, |along, across| {
                // A lighter edge on one side of the vein.
                tone + across * 0.1 - along.abs() * 0.06
            });
        }
    }
}

fn grass(c: &mut Canvas, seed: u32) {
    for k in 0..18 {
        let h = |i| hash(k, i, seed);
        let base = vec2(0.5 + (h(0) - 0.5) * 0.36, 0.99);
        let lean = (h(1) - 0.5) * 0.7 + (base.x - 0.5) * 0.9;
        let height = 0.55 + h(2) * 0.4;
        let tip = base + vec2(lean * height * 0.45, -height);
        let dark = 0.38 + h(3) * 0.12;
        c.blade(
            base,
            tip,
            0.05 + h(4) * 0.03,
            lean * 0.08,
            dark,
            0.92 + h(5) * 0.08,
        );
    }
}

fn fern(c: &mut Canvas, seed: u32) {
    for k in 0..7 {
        let h = |i| hash(k, i, seed);
        let a = std::f32::consts::PI * (0.12 + 0.76 * (k as f32 + 0.5) / 7.0) + (h(0) - 0.5) * 0.2;
        let base = vec2(0.5, 0.97);
        let len = 0.34 + h(1) * 0.1;
        let dir = vec2(-a.cos(), -a.sin());
        let droop = vec2(0.0, 0.12);
        // The stem, then leaflets along it, smaller towards the tip.
        c.blade(base, base + dir * len + droop, 0.02, 0.0, 0.35, 0.6);
        for s in 1..12 {
            let t = s as f32 / 12.0;
            let at = base + dir * len * t + droop * t * t;
            let size = 0.065 * (1.0 - t * 0.7);
            for side in [-1.0f32, 1.0] {
                let out = dir.perp() * side + dir * 0.5;
                let p = at + out.normalize() * size * 0.8;
                let tone = 0.5 + t * 0.35 + (0.5 - p.y) * 0.2;
                c.ellipse(
                    p.x,
                    p.y,
                    size,
                    size * 0.38,
                    out.y.atan2(out.x),
                    |along, _| tone + along * 0.06,
                );
            }
        }
    }
}

fn blossoms(c: &mut Canvas, seed: u32) {
    for k in 0..6 {
        let h = |i| hash(k, i, seed);
        let (x, y) = (0.22 + h(0) * 0.56, 0.25 + h(1) * 0.5);
        c.blossom(x, y, 0.09 + h(2) * 0.05, h(3) * 3.0);
    }
}

/// Paints one card of the foliage atlas, varied by a seed.
type Painter = fn(&mut Canvas, u32);

/// The foliage atlas: leaves, grass, fern and blossom cards, each a quarter
/// of the texture, in shades of gray for the zone's colors to tint.
pub(super) fn foliage() -> Texture2D {
    let (w, px) = foliage_pixels();
    atlas(w, w, &px)
}

fn foliage_pixels() -> (usize, Vec<[u8; 4]>) {
    let paint: [(Painter, usize, usize); 4] = [
        (leaves, 0, 0),
        (grass, 1, 0),
        (fern, 0, 1),
        (blossoms, 1, 1),
    ];
    let w = CELL * 2;
    let mut out = vec![[0u8; 4]; w * w];
    for (seed, (f, cx, cy)) in paint.into_iter().enumerate() {
        let mut c = Canvas::new(CELL);
        f(&mut c, seed as u32 + 1);
        c.bleed();
        for y in 0..CELL {
            for x in 0..CELL {
                let p = c.px[y * CELL + x];
                out[(cy * CELL + y) * w + cx * CELL + x] = p.map(byte);
            }
        }
    }
    (w, out)
}

/// How many cloud shapes the cloud atlas holds, side by side.
pub(super) const CLOUDS: usize = 4;
const CLOUD_W: usize = 256;
const CLOUD_H: usize = 128;

/// Billowing clouds, lit from above, with soft edges and flatter, shaded
/// bottoms: `CLOUDS` of them side by side.
pub(super) fn clouds() -> Texture2D {
    atlas(CLOUD_W * CLOUDS, CLOUD_H, &cloud_pixels())
}

fn cloud_pixels() -> Vec<[u8; 4]> {
    let w = CLOUD_W * CLOUDS;
    let mut out = vec![[0u8; 4]; w * CLOUD_H];
    for k in 0..CLOUDS {
        let h = |i: i32| hash(k as i32, i, 77);
        // Puffs along a flat base, biggest in the middle.
        let puffs: Vec<(f32, f32, f32)> = (0..9)
            .map(|i| {
                let t = (i as f32 + 0.5) / 9.0;
                let x = 0.14 + t * 0.72 + (h(i * 3) - 0.5) * 0.06;
                let mid = 1.0 - (t - 0.5).abs() * 2.0;
                let r = (0.06 + mid * 0.1) * (0.8 + h(i * 3 + 1) * 0.4);
                (x, 0.36 - r * 0.6 + (h(i * 3 + 2) - 0.5) * 0.03, r)
            })
            .collect();
        for y in 0..CLOUD_H {
            for x in 0..CLOUD_W {
                // Both in units of the cloud's width, so the puffs stay round.
                let (u, v) = (
                    (x as f32 + 0.5) / CLOUD_W as f32,
                    (y as f32 + 0.5) / CLOUD_W as f32,
                );
                let (mut density, mut lit) = (0.0f32, 0.0f32);
                for &(px, py, r) in &puffs {
                    let (dx, dy) = (u - px, v - py);
                    let d = (dx * dx + dy * dy).sqrt() / r;
                    let inside = (1.0 - d).max(0.0);
                    if inside > density {
                        density = inside;
                        // Each puff is lit on its upper side.
                        lit = (0.5 - dy / r * 0.5 - dx / r * 0.15).clamp(0.0, 1.0);
                    }
                }
                let wisp = tile_noise(u, v, 16, 78 + k as u32);
                let flat_bottom = 1.0 - ((v - 0.34) / 0.04).clamp(0.0, 1.0);
                let alpha = (density * 2.6 * (0.75 + wisp * 0.5) * flat_bottom).clamp(0.0, 1.0);
                let height = 1.0 - ((v - 0.1) / 0.26).clamp(0.0, 1.0);
                let tone = 0.62 + height * 0.18 + lit * 0.2;
                out[y * w + k * CLOUD_W + x] =
                    [byte(tone), byte(tone), byte(tone * 1.02), byte(alpha)];
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_layers_average_near_the_middle() {
        for top in [Top::Grass, Top::Sand, Top::Snow, Top::Cave] {
            let px = ground_pixels(top, 64);
            for ch in 0..4 {
                let mean = px.iter().map(|p| p[ch] as f32 / 255.0).sum::<f32>() / px.len() as f32;
                assert!((0.3..0.7).contains(&mean), "{top:?} channel {ch}: {mean}");
            }
        }
    }

    #[test]
    fn foliage_cards_are_cut_out_with_painted_middles() {
        let (w, px) = foliage_pixels();
        for (cx, cy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let cell: Vec<[u8; 4]> = (0..CELL)
                .flat_map(|y| (0..CELL).map(move |x| (x, y)))
                .map(|(x, y)| px[(cy * CELL + y) * w + cx * CELL + x])
                .collect();
            let solid = cell.iter().filter(|p| p[3] == 255).count() as f32 / cell.len() as f32;
            assert!((0.04..0.8).contains(&solid), "cell {cx},{cy}: {solid}");
            // The very edge is see-through, so neighbors never bleed in.
            for i in 0..CELL {
                assert_eq!(cell[i][3], 0, "top edge of {cx},{cy}");
                assert_eq!(cell[i * CELL][3], 0, "left edge of {cx},{cy}");
            }
            // See-through pixels carry the leaf color, not black.
            assert!(cell.iter().filter(|p| p[3] == 0).all(|p| p[0] > 60));
        }
    }

    #[test]
    fn clouds_are_soft_and_lighter_on_top() {
        let px = cloud_pixels();
        let w = CLOUD_W * CLOUDS;
        let x = CLOUD_W / 2;
        let column: Vec<[u8; 4]> = (0..CLOUD_H).map(|y| px[y * w + x]).collect();
        let solid: Vec<&[u8; 4]> = column.iter().filter(|p| p[3] > 200).collect();
        assert!(solid.len() > 10);
        assert!(solid.first().unwrap()[0] > solid.last().unwrap()[0]);
        assert_eq!(column[0][3], 0);
        assert_eq!(column[CLOUD_H - 1][3], 0);
    }
}
