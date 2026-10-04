//! The sky, and the weather drifting through the air.

use macroquad::prelude::*;
use shared::props::Scatter;
use shared::world::*;

use super::*;

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
        for (r, a) in [(120.0, 0.08), (70.0, 0.15), (40.0, 0.35)] {
            draw_circle(
                p.x,
                p.y,
                r,
                Color::new(disc.r, disc.g * 0.85, disc.b * 0.6, a),
            );
        }
        draw_circle(p.x, p.y, 26.0, disc);
    }
    // Clouds drifting slowly across the sky.
    if !t.cave {
        let cloud = match zone {
            Zone::Witherwood => Color::new(0.3, 0.25, 0.35, 0.55),
            Zone::Silverbough => Color::new(0.55, 0.6, 0.85, 0.35),
            Zone::Scorchsand => Color::new(1.0, 1.0, 1.0, 0.45),
            Zone::Frostcog => Color::new(1.0, 1.0, 1.0, 0.7),
            _ => mix(t.sky_horizon, Color::new(1.0, 0.85, 0.8, 1.0), 0.5),
        };
        let cloud = Color::new(cloud.r, cloud.g, cloud.b, cloud.a.min(0.6));
        let mut rng = Scatter(91 + zone.index() as u32);
        for _ in 0..16 {
            let az = rng.range(0.0, std::f32::consts::TAU) + time * 0.006;
            let el = rng.range(0.06, 0.32);
            let size = rng.range(0.6, 1.4);
            let dir = vec3(az.cos() * el.cos(), el.sin(), az.sin() * el.cos());
            let Some(p) = project(cam.position + dir * 1000.0) else {
                continue;
            };
            // Closer to the horizon looks further away: smaller and flatter.
            let r = 34.0 * size * (0.5 + el * 2.5);
            for k in 0..5 {
                let o = (k as f32 - 2.0) * r * 0.55;
                let rr = r * (1.0 - (k as f32 - 2.0).abs() * 0.18);
                draw_ellipse(
                    p.x + o,
                    p.y - (k % 2) as f32 * r * 0.15,
                    rr,
                    rr * 0.45,
                    0.0,
                    cloud,
                );
            }
        }
    }
}
