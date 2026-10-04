//! The ground: its shape, colors and what grows on it, and the cave ceiling.

use macroquad::prelude::*;
use shared::props::Scatter;
use shared::world::*;

use super::foliage::{ferns, flowers, tuft};
use super::paint::Card;
use super::rocks::{Stone, boulder};
use super::*;

/// Ground color variation in about `-0.5..0.5`: patches of all sizes, with
/// no grid to them.
pub(super) fn noise(x: f32, z: f32) -> f32 {
    (fbm(x * 0.06, z * 0.06, 3, 3) - 0.5) * 1.4
}

/// A zone's ground colors.
pub(super) struct Ground {
    main: Color,
    var_a: Color,
    var_b: Color,
    shore: Color,
    rock: Color,
    peak: Color,
    town: Color,
    road: Color,
    field: Color,
}

pub(super) fn ground_colors(zone: Zone) -> Ground {
    match zone {
        Zone::Amberfall => Ground {
            main: c(0.6, 0.52, 0.25),
            var_a: c(0.58, 0.38, 0.18),
            var_b: c(0.42, 0.45, 0.22),
            shore: c(0.72, 0.62, 0.45),
            rock: c(0.46, 0.42, 0.4),
            peak: c(0.92, 0.9, 0.94),
            town: c(0.56, 0.52, 0.48),
            road: c(0.5, 0.4, 0.28),
            field: c(0.4, 0.29, 0.18),
        },
        Zone::Scorchsand => Ground {
            main: c(0.88, 0.72, 0.47),
            var_a: c(0.84, 0.58, 0.33),
            var_b: c(0.9, 0.8, 0.58),
            shore: c(0.5, 0.6, 0.3),
            rock: c(0.62, 0.38, 0.25),
            peak: c(0.78, 0.5, 0.32),
            town: c(0.68, 0.45, 0.32),
            road: c(0.74, 0.58, 0.4),
            field: c(0.58, 0.4, 0.26),
        },
        Zone::Silverbough => Ground {
            main: c(0.3, 0.55, 0.32),
            var_a: c(0.25, 0.5, 0.42),
            var_b: c(0.42, 0.6, 0.35),
            shore: c(0.7, 0.72, 0.6),
            rock: c(0.55, 0.58, 0.62),
            peak: c(0.85, 0.88, 0.95),
            town: c(0.82, 0.82, 0.86),
            road: c(0.78, 0.76, 0.7),
            field: c(0.32, 0.42, 0.28),
        },
        Zone::Grubdeep => Ground {
            main: c(0.3, 0.27, 0.26),
            var_a: c(0.22, 0.32, 0.36),
            var_b: c(0.36, 0.3, 0.26),
            shore: c(0.26, 0.26, 0.28),
            rock: c(0.32, 0.31, 0.33),
            peak: c(0.25, 0.24, 0.27),
            town: c(0.42, 0.33, 0.24),
            road: c(0.36, 0.3, 0.24),
            field: c(0.28, 0.22, 0.2),
        },
        Zone::Frostcog => Ground {
            main: c(0.92, 0.94, 0.98),
            var_a: c(0.82, 0.88, 0.96),
            var_b: c(0.96, 0.97, 0.99),
            shore: c(0.8, 0.86, 0.94),
            rock: c(0.5, 0.52, 0.56),
            peak: c(0.98, 0.98, 1.0),
            town: c(0.62, 0.62, 0.66),
            road: c(0.8, 0.82, 0.86),
            field: c(0.5, 0.42, 0.36),
        },
        Zone::Witherwood => Ground {
            main: c(0.38, 0.36, 0.3),
            var_a: c(0.32, 0.38, 0.24),
            var_b: c(0.3, 0.26, 0.3),
            shore: c(0.3, 0.32, 0.22),
            rock: c(0.36, 0.34, 0.36),
            peak: c(0.5, 0.48, 0.52),
            town: c(0.34, 0.32, 0.34),
            road: c(0.3, 0.27, 0.24),
            field: c(0.28, 0.22, 0.18),
        },
    }
}

/// The color of a spot on the world map (world XZ position).
pub fn map_color(zone: Zone, world: Vec2) -> Color {
    let local = zone.to_local(world);
    let h = terrain_height(world.x, world.y);
    let e = 1.0;
    let normal = vec3(
        terrain_height(world.x - e, world.y) - terrain_height(world.x + e, world.y),
        2.0 * e,
        terrain_height(world.x, world.y - e) - terrain_height(world.x, world.y + e),
    )
    .normalize();
    let g = ground_colors(zone);
    let mut col = terrain_color(zone, &g, local, h, 1.0 - normal.y);
    // Hill shading from the north-west.
    let lit = 0.75 + normal.dot(vec3(0.5, 0.75, 0.45).normalize()) * 0.35;
    col = dark(col, lit);
    let water = zone.water_level();
    if h < water {
        let t = theme(zone).water;
        col = mix(
            Color::new(t.r, t.g, t.b, 1.0),
            dark(Color::new(t.r, t.g, t.b, 1.0), 0.6),
            (water - h) / 4.0,
        );
    }
    col
}

/// The main leaf color of a zone, for the map.
pub fn foliage(zone: Zone) -> Color {
    match zone {
        Zone::Amberfall => c(0.85, 0.42, 0.12),
        Zone::Scorchsand => c(0.3, 0.52, 0.28),
        Zone::Silverbough => c(0.3, 0.62, 0.45),
        Zone::Grubdeep => c(0.4, 0.38, 0.45),
        Zone::Frostcog => c(0.16, 0.3, 0.24),
        Zone::Witherwood => c(0.2, 0.17, 0.16),
    }
}

pub(super) fn terrain_color(zone: Zone, g: &Ground, local: Vec2, h: f32, slope: f32) -> Color {
    terrain_paint(zone, g, local, h, slope).0
}

/// The ground's color at a spot, and how much of it is rock (x) and how
/// much bare dirt, sand at the shore, road or field (y), for the painted
/// ground textures.
pub(super) fn terrain_paint(
    zone: Zone,
    g: &Ground,
    local: Vec2,
    h: f32,
    slope: f32,
) -> (Color, Vec2) {
    let (x, z) = (local.x, local.y);
    let n = noise(x, z);
    let mut col = mix(g.main, g.var_a, (n * 3.0 + 0.2).max(0.0));
    col = mix(col, g.var_b, (-n * 3.0 - 0.3).max(0.0));
    let shore = 1.0 - ((h - zone.water_level() - 0.3) / 1.2).clamp(0.0, 1.0);
    col = mix(g.shore, col, 1.0 - shore);
    let cliff = ((slope - 0.45) * 3.0).clamp(0.0, 1.0);
    let high = ((h - 12.0) / 3.0).clamp(0.0, 1.0);
    col = mix(col, g.rock, cliff);
    col = mix(col, g.rock, high);
    let peak = ((h - 18.0) / 2.0).clamp(0.0, 1.0);
    col = mix(col, g.peak, peak);
    let d = local.length();
    let road = (1.0 - x.abs().min(z.abs()) / 3.0).clamp(0.0, 1.0)
        * (1.0 - (d - 70.0) / 20.0).clamp(0.0, 1.0);
    col = mix(col, g.road, road * 0.85);
    let mut field = 0.0f32;
    for &f in &zone.layout().fields {
        let inside = (1.0 - (local.distance(f) - 11.0) / 2.0).clamp(0.0, 1.0);
        let rows = ((x - f.x) * 1.6).sin() * 0.5 + 0.5;
        col = mix(col, mix(g.field, dark(g.field, 1.25), rows), inside);
        field = field.max(inside);
    }
    let town = ((TOWN_RADIUS - d) / 3.0).clamp(0.0, 1.0);
    let col = mix(col, mix(g.town, dark(g.town, 1.1), n + 0.5), town);
    let rock = cliff.max(high) * (1.0 - peak * 0.6) * (1.0 - town);
    let dirt = shore.max(road).max(field).max(town * 0.8).max(cliff * 0.5);
    (col, vec2(rock, dirt))
}

pub(super) fn terrain(b: &mut Batch, zone: Zone) {
    let g = ground_colors(zone);
    let step = 2.0;
    let n = (WORLD_HALF_SIZE * 2.0 / step) as usize;
    let rows_per_strip = 50;
    let mut row = 0;
    while row < n {
        let rows = rows_per_strip.min(n - row);
        b.flush();
        let base = b.begin_shape((rows + 1) * (n + 1), rows * n * 6);
        for j in row..=row + rows {
            for i in 0..=n {
                let local = vec2(
                    -WORLD_HALF_SIZE + i as f32 * step,
                    -WORLD_HALF_SIZE + j as f32 * step,
                );
                let w = zone.to_world(local);
                let h = terrain_height(w.x, w.y);
                // Sampled a little wide, so hills shade as smooth, rounded
                // slopes rather than showing every small bump.
                let e = 1.5;
                let normal = vec3(
                    terrain_height(w.x - e, w.y) - terrain_height(w.x + e, w.y),
                    2.0 * e,
                    terrain_height(w.x, w.y - e) - terrain_height(w.x, w.y + e),
                )
                .normalize();
                let (color, weights) = terrain_paint(zone, &g, local, h, 1.0 - normal.y);
                b.vertex_uv(vec3(w.x, h, w.y), weights, normal, color);
            }
        }
        let stride = n as u16 + 1;
        for j in 0..rows as u16 {
            for i in 0..n as u16 {
                let a = base + j * stride + i;
                let cc = a + stride;
                b.index(&[a, cc, a + 1, a + 1, cc, cc + 1]);
            }
        }
        b.flush();
        row += rows;
    }
}

/// Grass, ferns, flowers, leaves, pebbles, snow lumps and other little
/// things on the ground.
pub(super) fn ground_cover(b: &mut Batch, cards: &mut Batch, zone: Zone) {
    let mut rng = Scatter(0xBEEF + zone.index() as u32);
    let water = zone.water_level();
    let g = ground_colors(zone);
    let pebble = |color| Stone {
        color,
        top: None,
        slices: 6,
        stacks: 4,
    };
    for n in 0..9000u32 {
        let x = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let z = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let kind = rng.unit();
        let local = vec2(x, z);
        let w = zone.to_world(local);
        let h = terrain_height(w.x, w.y);
        if local.length() < TOWN_RADIUS + 2.0 || !(water + 0.3..14.0).contains(&h) {
            continue;
        }
        let base = vec3(w.x, h, w.y);
        let shade_n = rng.range(-0.06, 0.06);
        // Grass takes after the ground it grows from, so it blends in.
        let soil = terrain_color(zone, &g, local, h, 0.0);
        match zone {
            Zone::Grubdeep => {
                if kind < 0.2 {
                    let col = if rng.unit() < 0.5 {
                        c(0.4, 1.0, 0.6)
                    } else {
                        c(0.75, 0.45, 1.0)
                    };
                    b.cylinder(base, Vec3::Y * 0.2, 0.03, 4, c(0.75, 0.75, 0.7));
                    b.lit(|b| b.ellipsoid(base + Vec3::Y * 0.22, vec3(0.1, 0.05, 0.1), col));
                } else if kind < 0.6 {
                    let col = c(0.34 + shade_n, 0.32 + shade_n, 0.34);
                    let r = vec3(0.17, 0.11, 0.14) * rng.range(0.7, 1.5);
                    boulder(
                        b,
                        base + Vec3::Y * r.y * 0.3,
                        r,
                        kind * 9.0,
                        n,
                        &pebble(col),
                    );
                }
            }
            Zone::Frostcog => {
                if kind < 0.5 {
                    b.ellipsoid(
                        base,
                        vec3(0.5, 0.18, 0.4) * rng.range(0.6, 1.4),
                        c(0.95, 0.97, 1.0),
                    );
                } else if kind < 0.6 {
                    b.cone(
                        base,
                        vec3(0.05, 0.6, 0.02),
                        0.08,
                        0.0,
                        4,
                        c(0.75, 0.88, 1.0),
                    );
                } else if kind < 0.7 {
                    // Frosted grass poking through the snow.
                    tuft(cards, &mut rng, base, 0.6, Card::Grass, c(0.7, 0.74, 0.62));
                }
            }
            Zone::Scorchsand => {
                if kind < 0.3 {
                    let col = mix(c(0.72 + shade_n, 0.64 + shade_n, 0.36), soil, 0.3);
                    tuft(cards, &mut rng, base, 0.75, Card::Grass, col);
                } else if kind < 0.55 {
                    let col = c(0.6 + shade_n, 0.42, 0.3);
                    let r = vec3(0.15, 0.08, 0.12) * rng.range(0.7, 1.6);
                    boulder(
                        b,
                        base + Vec3::Y * r.y * 0.3,
                        r,
                        kind * 7.0,
                        n,
                        &pebble(col),
                    );
                }
            }
            _ => {
                let (grass, leaves): (Color, &[Color]) = match zone {
                    Zone::Silverbough => (
                        c(0.32 + shade_n, 0.62 + shade_n, 0.34),
                        &[c(0.95, 0.95, 1.0), c(0.7, 0.75, 1.0), c(0.95, 0.85, 0.45)],
                    ),
                    Zone::Witherwood => (
                        c(0.4 + shade_n, 0.38 + shade_n, 0.3),
                        &[c(0.3, 0.25, 0.2), c(0.35, 0.3, 0.24), c(0.45, 0.42, 0.35)],
                    ),
                    _ => (
                        c(0.66 + shade_n, 0.56 + shade_n, 0.28),
                        &[
                            c(0.9, 0.46, 0.12),
                            c(0.75, 0.2, 0.1),
                            c(0.93, 0.72, 0.2),
                            c(0.82, 0.34, 0.1),
                        ],
                    ),
                };
                let grass = dark(mix(grass, soil, 0.45), 1.2);
                let pick = |rng: &mut Scatter| {
                    leaves[(rng.unit() * leaves.len() as f32) as usize % leaves.len()]
                };
                if kind < 0.68 {
                    let size = rng.range(0.6, 1.0);
                    tuft(cards, &mut rng, base, size, Card::Grass, grass);
                } else if kind < 0.74 {
                    let size = rng.range(0.6, 0.9);
                    ferns(cards, &mut rng, base, size, dark(grass, 0.85));
                } else if zone == Zone::Silverbough && kind < 0.8 {
                    // Glowing flowers.
                    let col = pick(&mut rng);
                    b.cylinder(base, Vec3::Y * 0.3, 0.02, 3, c(0.3, 0.55, 0.3));
                    b.glow_sphere(base + Vec3::Y * 0.32, 0.07, col);
                } else if zone == Zone::Amberfall && kind < 0.8 {
                    let col = pick(&mut rng);
                    flowers(cards, &mut rng, base, grass, dark(col, 1.2));
                } else {
                    for k in 0..5 {
                        let col = pick(&mut rng);
                        let a = rng.range(0.0, std::f32::consts::TAU);
                        let r = rng.range(0.15, 0.3);
                        let o = vec3((k as f32 * 2.4).cos(), 0.0, (k as f32 * 2.4).sin())
                            * rng.range(0.0, 0.8);
                        let at = base + o;
                        let at = vec3(at.x, terrain_height(at.x, at.z) + 0.04, at.z);
                        let u = vec3(a.cos(), 0.0, a.sin()) * r;
                        let v = vec3(-a.sin(), 0.0, a.cos()) * r * 0.6;
                        b.quad([at - u, at - v, at + u, at + v], Vec3::Y, col);
                    }
                }
            }
        }
    }
}

/// Meadow grass: tufts in drifts across the open ground, thinning out on
/// roads, fields, shores and high slopes.
pub(super) fn meadows(cards: &mut Batch, zone: Zone) {
    let (grass, count) = match zone {
        Zone::Amberfall => (c(0.66, 0.56, 0.28), 30_000),
        Zone::Silverbough => (c(0.32, 0.62, 0.34), 30_000),
        Zone::Witherwood => (c(0.4, 0.38, 0.3), 18_000),
        _ => return,
    };
    let mut rng = Scatter(0x6A55 + zone.index() as u32);
    let water = zone.water_level();
    let g = ground_colors(zone);
    for _ in 0..count {
        let x = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let z = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let keep = rng.unit();
        let local = vec2(x, z);
        let w = zone.to_world(local);
        let h = terrain_height(w.x, w.y);
        if local.length() < TOWN_RADIUS + 2.0 || !(water + 0.6..12.0).contains(&h) {
            continue;
        }
        let (soil, weights) = terrain_paint(zone, &g, local, h, 0.0);
        let drift = fbm(x * 0.05, z * 0.05, 2, 77 + zone.index() as u32);
        if keep > (drift - 0.2) * 2.5 * (1.0 - weights.y * 1.5) {
            continue;
        }
        let tint = dark(mix(grass, soil, 0.4), rng.range(1.1, 1.35));
        let size = rng.range(0.55, 0.95);
        tuft(cards, &mut rng, vec3(w.x, h, w.y), size, Card::Grass, tint);
    }
}

/// The rock ceiling over the caverns, with stalactites.
pub(super) fn cave_ceiling(b: &mut Batch, zone: Zone) {
    let center = zone.center();
    let step = 20.0;
    let n = (WORLD_HALF_SIZE * 2.0 / step) as i32;
    let height = |x: f32, z: f32| 30.0 + noise(x * 0.3, z * 0.3) * 4.0;
    let rock = c(0.2, 0.19, 0.21);
    for j in 0..n {
        for i in 0..n {
            let x0 = -WORLD_HALF_SIZE + i as f32 * step;
            let z0 = -WORLD_HALF_SIZE + j as f32 * step;
            let corners = [
                (x0, z0),
                (x0 + step, z0),
                (x0 + step, z0 + step),
                (x0, z0 + step),
            ]
            .map(|(x, z)| vec3(center.x + x, height(x, z), center.y + z));
            b.quad(corners, -Vec3::Y, rock);
        }
    }
    let mut rng = Scatter(0xCA7E);
    for _ in 0..700 {
        let x = rng.range(-WORLD_HALF_SIZE, WORLD_HALF_SIZE);
        let z = rng.range(-WORLD_HALF_SIZE, WORLD_HALF_SIZE);
        let len = rng.range(2.0, 9.0);
        let top = vec3(center.x + x, height(x, z) + 0.5, center.y + z);
        b.cone(
            top,
            -Vec3::Y * len,
            rng.range(0.6, 1.8),
            0.0,
            6,
            c(0.26, 0.24, 0.26),
        );
        if rng.unit() < 0.15 {
            b.glow_sphere(top - Vec3::Y * (len - 0.3), 0.18, c(0.5, 0.9, 1.0));
        }
    }
}
