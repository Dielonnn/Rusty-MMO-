//! The sky, and the weather drifting through the air.

use macroquad::prelude::*;
use shared::props::Scatter;
use shared::world::*;

use std::cell::OnceCell;

use macroquad::models::{Mesh, Vertex, draw_mesh};

use super::theme::Theme;
use super::*;
use crate::gfx::texture::tile_noise;

/// Small things drifting through the air around the camera: falling
/// leaves, blowing sand, fireflies, spores, snow and ghostly wisps. Each
/// particle has a fixed home in a tile that repeats across the world, so
/// they stay put as you walk through them.
pub(super) fn weather(b: &mut Batch, zone: Zone, time: f32, around: Vec3) {
    const TILE: f32 = 70.0;
    let mut rng = Scatter(0x5EED + zone.index() as u32 * 977);
    let (count, height) = match zone {
        Zone::Amberfall => (110, 14.0),
        Zone::Scorchsand => (160, 3.0),
        Zone::Silverbough => (90, 5.0),
        Zone::Grubdeep => (110, 12.0),
        Zone::Frostcog => (320, 16.0),
        Zone::Witherwood => (70, 6.0),
    };
    let place = |base: f32, drift: f32, center: f32| {
        center + ((base * TILE + drift - center).rem_euclid(TILE) - TILE * 0.5)
    };
    for i in 0..count {
        let (bx, bz, by, phase) = (
            rng.unit(),
            rng.unit(),
            rng.unit(),
            rng.unit() * std::f32::consts::TAU,
        );
        let speed = 0.6 + rng.unit() * 0.8;
        match zone {
            Zone::Amberfall => {
                // Leaves tumbling down on a breeze.
                let x = place(bx, time * 0.8, around.x);
                let z = place(bz, time * 0.3, around.z);
                let fall = (by * height - time * speed).rem_euclid(height);
                let ground_y = terrain_height(x, z);
                let p = vec3(x + (time * 1.3 + phase).sin() * 0.6, ground_y + fall, z);
                let spin = time * 2.0 + phase;
                let u = vec3(spin.cos(), (spin * 0.7).sin() * 0.5, spin.sin()) * 0.12;
                let v = vec3(-spin.sin(), 0.3, spin.cos()) * 0.08;
                let col = [c(0.9, 0.46, 0.12), c(0.75, 0.2, 0.1), c(0.93, 0.72, 0.2)][i % 3];
                b.quad([p - u, p - v, p + u, p + v], Vec3::Y, col);
            }
            Zone::Scorchsand => {
                // Sand streaming low over the dunes.
                let x = place(bx, time * 7.0 * speed, around.x);
                let z = place(bz, time * 2.0 * speed, around.z);
                let y = terrain_height(x, z) + 0.2 + by * height + (time * 3.0 + phase).sin() * 0.1;
                let col = Color::new(0.95, 0.85, 0.62, 0.55);
                let streak = vec3(0.35, 0.0, 0.1);
                b.beam(vec3(x, y, z) - streak, vec3(x, y, z), 0.025, Vec3::Y, col);
            }
            Zone::Silverbough => {
                // Fireflies wandering and pulsing.
                let x = place(bx, (time * 0.3 + phase).sin() * 2.0, around.x);
                let z = place(bz, (time * 0.25 + phase).cos() * 2.0, around.z);
                let y = terrain_height(x, z) + 0.6 + by * height + (time * 0.9 + phase).sin() * 0.4;
                let pulse = 0.5 + 0.5 * (time * 2.5 + phase).sin();
                b.glow_sphere(
                    vec3(x, y, z),
                    0.05 + pulse * 0.03,
                    Color::new(0.75, 1.0, 0.55, 0.4 + pulse * 0.6),
                );
            }
            Zone::Grubdeep => {
                // Glowing spores rising slowly.
                let x = place(bx, (time * 0.2 + phase).sin(), around.x);
                let z = place(bz, 0.0, around.z);
                let rise = (by * height + time * speed * 0.5).rem_euclid(height);
                let y = terrain_height(x, z) + rise;
                let col = if i % 2 == 0 {
                    Color::new(0.45, 1.0, 0.6, 0.7)
                } else {
                    Color::new(0.75, 0.5, 1.0, 0.7)
                };
                b.glow_sphere(vec3(x, y, z), 0.04, col);
            }
            Zone::Frostcog => {
                // Snowfall.
                let x = place(bx, (time * 0.5 + phase).sin() * 0.8 + time * 0.4, around.x);
                let z = place(bz, 0.0, around.z);
                let fall = (by * height - time * speed * 1.6).rem_euclid(height);
                let y = terrain_height(x, z) + fall;
                b.block(vec3(x, y, z), Vec3::splat(0.045), phase, c(1.0, 1.0, 1.0));
            }
            Zone::Witherwood => {
                // Pale green wisps drifting and fading in and out.
                let x = place(bx, time * 0.6 * speed, around.x);
                let z = place(bz, (time * 0.4 + phase).sin() * 3.0, around.z);
                let y = terrain_height(x, z) + 0.8 + by * height + (time + phase).sin() * 0.5;
                let fade = (0.5 + 0.5 * (time * 0.7 + phase).sin()).powi(2);
                let col = Color::new(0.55, 1.0, 0.55, 0.15 + fade * 0.5);
                b.glow_sphere(vec3(x, y, z), 0.12, col);
                b.glow_sphere(
                    vec3(x - 0.25, y - 0.05, z),
                    0.07,
                    Color::new(col.r, col.g, col.b, col.a * 0.5),
                );
            }
        }
    }
}

/// The sky, drawn in screen space before the 3D pass.
pub fn draw_sky(cam: &Camera3D, zone: Zone, time: f32, project: impl Fn(Vec3) -> Option<Vec2>) {
    let t = theme(zone);
    // Also clears the depth buffer for the 3D pass.
    clear_background(t.fog);
    let (w, h) = (screen_width(), screen_height());
    let look = (cam.target - cam.position).normalize_or_zero();
    let flat = vec3(look.x, 0.0, look.z).normalize_or_zero();
    let horizon = project(cam.position + flat * 1000.0).map_or(h * 0.5, |p| p.y);
    let bands = 160;
    for i in 0..bands {
        let y0 = h * i as f32 / bands as f32;
        let y1 = h * (i + 1) as f32 / bands as f32;
        let k = ((y0 + y1) * 0.5 - horizon) / h;
        let col = if k < 0.0 {
            let up = (-k * 2.2).min(1.0);
            if up < 0.35 {
                mix(t.sky_horizon, t.sky_mid, up / 0.35)
            } else {
                mix(t.sky_mid, t.sky_top, (up - 0.35) / 0.65)
            }
        } else {
            mix(t.sky_horizon, t.fog, k * 3.0)
        };
        draw_rectangle(0.0, y0, w, y1 - y0 + 1.0, col);
    }
    if t.stars > 0.0 {
        let mut rng = Scatter(77);
        for _ in 0..60 {
            let dir = vec3(
                rng.range(-1.0, 1.0),
                rng.range(0.35, 1.0),
                rng.range(-1.0, 1.0),
            )
            .normalize();
            if let Some(p) = project(cam.position + dir * 1000.0) {
                let alpha = ((horizon - p.y) / h).clamp(0.0, 0.6) * t.stars;
                draw_circle(p.x, p.y, 1.2, Color::new(1.0, 0.95, 0.9, alpha));
            }
        }
    }
    if let (Some(disc), Some(p)) = (t.sun_disc, project(cam.position + t.light.sun_dir * 1000.0)) {
        // A wide, soft glow that brightens the sky around the sun.
        for k in 0..40 {
            let r = 300.0 * (1.0 - k as f32 / 40.0).powi(2) + 30.0;
            draw_circle(
                p.x,
                p.y,
                r,
                Color::new(disc.r, disc.g * 0.88, disc.b * 0.7, 0.014),
            );
        }
        draw_circle(p.x, p.y, 30.0, Color::new(disc.r, disc.g, disc.b, 0.5));
        draw_circle(p.x, p.y, 25.0, disc);
    }
    if t.cave {
        return;
    }
    clouds(cam, zone, &t, time, &project);
    ridges(cam, zone, &t, &project);
}

/// Painted clouds drifting slowly across the sky.
fn clouds(
    cam: &Camera3D,
    zone: Zone,
    t: &Theme,
    time: f32,
    project: &impl Fn(Vec3) -> Option<Vec2>,
) {
    thread_local! {
        static CLOUDS: OnceCell<Texture2D> = const { OnceCell::new() };
    }
    let texture = CLOUDS.with(|c| c.get_or_init(paint::clouds).clone());
    let cloud = match zone {
        Zone::Witherwood => Color::new(0.42, 0.36, 0.48, 0.8),
        Zone::Silverbough => Color::new(0.62, 0.68, 0.92, 0.6),
        Zone::Scorchsand => Color::new(1.0, 0.98, 0.95, 0.75),
        Zone::Frostcog => Color::new(1.0, 1.0, 1.0, 0.9),
        _ => {
            let warm = mix(t.sky_horizon, Color::new(1.0, 0.86, 0.8, 1.0), 0.55);
            Color::new(warm.r, warm.g, warm.b, 0.85)
        }
    };
    let (w, h) = texture.size().into();
    let cell = w / paint::CLOUDS as f32;
    let mut rng = Scatter(91 + zone.index() as u32);
    for _ in 0..18 {
        let az = rng.range(0.0, std::f32::consts::TAU) + time * 0.006;
        let el = rng.range(0.05, 0.34);
        let size = rng.range(0.7, 1.4);
        let shape = (rng.unit() * paint::CLOUDS as f32) as usize % paint::CLOUDS;
        let flip = rng.unit() < 0.5;
        let dir = vec3(az.cos() * el.cos(), el.sin(), az.sin() * el.cos());
        let Some(p) = project(cam.position + dir * 1000.0) else {
            continue;
        };
        // Closer to the horizon looks further away: smaller and flatter.
        let width = 330.0 * size * (0.45 + el * 2.4);
        let height = width * 0.5 * (0.7 + el * 1.2);
        draw_texture_ex(
            &texture,
            p.x - width * 0.5,
            p.y - height * 0.7,
            cloud,
            DrawTextureParams {
                dest_size: Some(vec2(width, height)),
                source: Some(Rect::new(shape as f32 * cell, 0.0, cell, h)),
                flip_x: flip,
                ..Default::default()
            },
        );
    }
}

/// Faraway mountains along the horizon, in two hazy layers, the further
/// one paler. Their feet fade into the mist.
fn ridges(cam: &Camera3D, zone: Zone, t: &Theme, project: &impl Fn(Vec3) -> Option<Vec2>) {
    const STEPS: usize = 240;
    let far = mix(t.sky_horizon, t.sky_mid, 0.3);
    let near = mix(far, dark(t.fog, 0.72), 0.55);
    let mist = Color::new(t.fog.r, t.fog.g, t.fog.b, 1.0);
    for (layer, (color, lift, tall)) in [(far, 0.012, 0.075), (near, 0.004, 0.045)]
        .into_iter()
        .enumerate()
    {
        let seed = zone.index() as u32 * 13 + layer as u32 * 101;
        let height = |a: f32| {
            let u = a / std::f32::consts::TAU;
            let n = tile_noise(u, 0.37, 9, seed) * 0.55
                + tile_noise(u, 0.37, 23, seed + 1) * 0.3
                + tile_noise(u, 0.37, 61, seed + 2) * 0.15;
            // Folded, so the ridges come to peaks.
            let peaks = 1.0 - (n * 2.0 - 1.0).abs();
            lift + tall * (n * 0.5 + peaks * 0.5).powf(1.4)
        };
        let at = |a: f32, el: f32| {
            project(cam.position + vec3(a.cos() * el.cos(), el.sin(), a.sin() * el.cos()) * 1000.0)
        };
        let mut vertices = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let mut prev: Option<(Vec2, Vec2)> = None;
        for k in 0..=STEPS {
            let a = std::f32::consts::TAU * k as f32 / STEPS as f32;
            let here = at(a, height(a)).zip(at(a, -0.03));
            if let (Some((t0, b0)), Some((t1, b1))) = (prev, here) {
                // Skip segments that wrap around behind the camera.
                if (t1.x - t0.x).abs() < screen_width() * 0.5 {
                    let base = vertices.len() as u16;
                    for (p, col) in [(t0, color), (t1, color), (b1, mist), (b0, mist)] {
                        vertices.push(Vertex::new(p.x, p.y, 0.0, 0.0, 0.0, col));
                    }
                    indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base,
                        base + 2,
                        base + 3,
                    ]);
                }
            }
            prev = here;
        }
        draw_mesh(&Mesh {
            vertices,
            indices,
            texture: None,
        });
    }
}
