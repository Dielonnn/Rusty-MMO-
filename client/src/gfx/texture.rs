//! Textures: decoding image files (PNG, JPEG...) that ship inside the game,
//! the painted grain the shader lays over every lit surface, and the noise
//! the world's painted textures are generated from.

use macroquad::miniquad::{FilterMode as MqFilter, MipmapFilterMode, TextureWrap};
use macroquad::prelude::*;

/// Decodes an image file's bytes (usually from `include_bytes!`) into a
/// texture that tiles, with mipmaps so it stays smooth in the distance.
pub fn load(bytes: &[u8]) -> Result<Texture2D, String> {
    let image = Image::from_file_with_format(bytes, None).map_err(|e| e.to_string())?;
    Ok(tiling(Texture2D::from_image(&image)))
}

/// Makes a texture repeat and gives it mipmaps.
pub fn tiling(texture: Texture2D) -> Texture2D {
    let id = texture.raw_miniquad_id();
    // SAFETY: only changes this texture's sampling settings, between draws.
    let ctx = unsafe { get_internal_gl() }.quad_context;
    ctx.texture_set_wrap(id, TextureWrap::Repeat, TextureWrap::Repeat);
    ctx.texture_generate_mipmaps(id);
    ctx.texture_set_min_filter(id, MqFilter::Linear, MipmapFilterMode::Linear);
    ctx.texture_set_mag_filter(id, MqFilter::Linear);
    texture
}

const DETAIL_SIZE: usize = 128;

/// A soft, seamless grain like brushwork, mid-gray on average.
pub fn detail() -> Texture2D {
    let pixels = detail_pixels(DETAIL_SIZE);
    let pixels: Vec<[u8; 4]> = pixels
        .into_iter()
        .map(|v| {
            let g = byte(v);
            [g, g, g, 255]
        })
        .collect();
    from_pixels(DETAIL_SIZE, &pixels)
}

/// Turns `size` by `size` RGBA pixels into a texture that tiles.
pub fn from_pixels(size: usize, pixels: &[[u8; 4]]) -> Texture2D {
    tiling(Texture2D::from_rgba8(
        size as u16,
        size as u16,
        pixels.as_flattened(),
    ))
}

/// Turns a `width` by `height` picture made of separate cells (an atlas)
/// into a texture. Its edges don't repeat, but it still has mipmaps.
pub fn atlas(width: usize, height: usize, pixels: &[[u8; 4]]) -> Texture2D {
    let texture = Texture2D::from_rgba8(width as u16, height as u16, pixels.as_flattened());
    let id = texture.raw_miniquad_id();
    // SAFETY: only changes this texture's sampling settings, between draws.
    let ctx = unsafe { get_internal_gl() }.quad_context;
    ctx.texture_set_wrap(id, TextureWrap::Clamp, TextureWrap::Clamp);
    ctx.texture_generate_mipmaps(id);
    ctx.texture_set_min_filter(id, MqFilter::Linear, MipmapFilterMode::Linear);
    ctx.texture_set_mag_filter(id, MqFilter::Linear);
    texture
}

/// A `0..1` channel value as a byte.
pub fn byte(v: f32) -> u8 {
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

/// A pseudo-random value in `0..1` for a lattice point.
pub fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d)
        ^ (y as u32).wrapping_mul(0x1656_67b1)
        ^ seed.wrapping_mul(0x9e37_79b9);
    h = (h ^ (h >> 15)).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}

/// Smooth noise in `0..1` over the unit square, on a lattice of `cells` by
/// `cells` that wraps around, so a texture made from it tiles.
pub fn tile_noise(u: f32, v: f32, cells: i32, seed: u32) -> f32 {
    let (x, y) = (u * cells as f32, v * cells as f32);
    let (x0, y0) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let at = |i: i32, j: i32| hash(i.rem_euclid(cells), j.rem_euclid(cells), seed);
    let top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * sx;
    let bottom = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * sx;
    top + (bottom - top) * sy
}

/// Cellular noise over the unit square that wraps around: distances (in
/// cells) to the nearest and second nearest of one random point per cell,
/// and a random value for the nearest point's cell.
pub fn tile_cells(u: f32, v: f32, cells: i32, seed: u32) -> (f32, f32, f32) {
    let (x, y) = (u * cells as f32, v * cells as f32);
    let (cx, cy) = (x.floor() as i32, y.floor() as i32);
    let (mut f1, mut f2, mut id) = (9.0f32, 9.0f32, 0.0);
    for j in -1..=1 {
        for i in -1..=1 {
            let (gx, gy) = (cx + i, cy + j);
            let (wx, wy) = (gx.rem_euclid(cells), gy.rem_euclid(cells));
            let px = gx as f32 + hash(wx, wy, seed);
            let py = gy as f32 + hash(wx, wy, seed ^ 0x5bd1);
            let d = ((px - x).powi(2) + (py - y).powi(2)).sqrt();
            if d < f1 {
                f2 = f1;
                f1 = d;
                id = hash(wx, wy, seed ^ 0xa11e);
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    (f1, f2, id)
}

/// Brightness in `0..1` for each pixel of a `size`-wide square that tiles
/// seamlessly: a few octaves of smooth noise, stretched a little sideways
/// so it reads as strokes rather than speckle.
fn detail_pixels(size: usize) -> Vec<f32> {
    let octave = |u: f32, v: f32, cells: i32| tile_noise(u, v, cells, 0);
    let mut out = Vec::with_capacity(size * size);
    for j in 0..size {
        for i in 0..size {
            let (u, v) = (i as f32 / size as f32, j as f32 / size as f32);
            let n = octave(u, v, 4) * 0.4
                + octave(u, v, 8) * 0.3
                + octave(u, v + octave(u, v, 4) * 0.1, 16) * 0.2
                + octave(u, v, 32) * 0.1;
            // Stretch the contrast around the middle.
            out.push(((n - 0.5) * 1.8 + 0.5).clamp(0.0, 1.0));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grain_tiles_and_averages_mid_gray() {
        let size = 64;
        let p = detail_pixels(size);
        let mean = p.iter().sum::<f32>() / p.len() as f32;
        assert!((mean - 0.5).abs() < 0.1, "mean {mean}");
        // Opposite edges match closely, so it repeats without a seam.
        for k in 0..size {
            let (left, right) = (p[k * size], p[k * size + size - 1]);
            let (top, bottom) = (p[k], p[(size - 1) * size + k]);
            assert!((left - right).abs() < 0.2, "row {k}");
            assert!((top - bottom).abs() < 0.2, "column {k}");
        }
    }

    #[test]
    fn noise_wraps_around() {
        for k in 0..20 {
            let t = k as f32 / 20.0;
            assert!((tile_noise(0.0, t, 8, 3) - tile_noise(1.0, t, 8, 3)).abs() < 1e-4);
            assert!((tile_noise(t, 0.0, 8, 3) - tile_noise(t, 1.0, 8, 3)).abs() < 1e-4);
            let (a, b) = (tile_cells(0.0, t, 6, 1), tile_cells(1.0, t, 6, 1));
            assert!((a.0 - b.0).abs() < 1e-3 && (a.2 - b.2).abs() < 1e-6);
        }
    }
}
