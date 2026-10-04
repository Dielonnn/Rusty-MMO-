//! Textures: decoding image files (PNG, JPEG...) that ship inside the game,
//! and the painted grain the shader lays over every lit surface.

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
    let mut bytes = Vec::with_capacity(pixels.len() * 4);
    for v in pixels {
        let g = (v * 255.0).round().clamp(0.0, 255.0) as u8;
        bytes.extend_from_slice(&[g, g, g, 255]);
    }
    tiling(Texture2D::from_rgba8(
        DETAIL_SIZE as u16,
        DETAIL_SIZE as u16,
        &bytes,
    ))
}

/// Brightness in `0..1` for each pixel of a `size`-wide square that tiles
/// seamlessly: a few octaves of smooth noise, stretched a little sideways
/// so it reads as strokes rather than speckle.
fn detail_pixels(size: usize) -> Vec<f32> {
    let hash = |x: i32, y: i32| {
        let mut h = (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1);
        h = (h ^ (h >> 15)).wrapping_mul(0x85eb_ca6b);
        h ^= h >> 13;
        (h & 0xffff) as f32 / 65535.0
    };
    // Smooth noise on a lattice of `cells` by `cells`, wrapping around.
    let octave = |u: f32, v: f32, cells: i32| {
        let (x, y) = (u * cells as f32, v * cells as f32);
        let (x0, y0) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let at = |i: i32, j: i32| hash(i.rem_euclid(cells), j.rem_euclid(cells));
        let top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * sx;
        let bottom = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * sx;
        top + (bottom - top) * sy
    };
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
}
