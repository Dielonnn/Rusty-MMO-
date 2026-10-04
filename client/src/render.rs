//! 3D drawing: a triangle batcher with per-zone lighting and distance fog,
//! the scenery of all six starting areas, and the character and creature
//! models.

use std::cell::OnceCell;

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::models::{Mesh, Vertex, draw_mesh};
use macroquad::prelude::*;
use shared::props::{self, Prop, PropKind, Scatter};
use shared::world::*;

use crate::models::{BONE, GOLD, WOOD};
pub use crate::models::{
    Look, Pose, draw_model, hair_color, model_height, model_radius, skin_color,
};

const MAX_VERTICES: usize = 60_000;
const MAX_INDICES: usize = 180_000;

// ---- Lighting and themes ----

/// Where light comes from and what color it is.
#[derive(Clone, Copy)]
pub struct Light {
    /// The direction light comes from.
    pub sun_dir: Vec3,
    pub sun: Vec3,
    pub ambient: Vec3,
}

/// The look of a zone: light, sky, fog and water.
#[derive(Clone, Copy)]
pub struct Theme {
    pub light: Light,
    pub sky_top: Color,
    pub sky_mid: Color,
    pub sky_horizon: Color,
    pub fog: Color,
    pub fog_near: f32,
    pub fog_far: f32,
    pub water: Color,
    /// Underground: no sky, a rock ceiling.
    pub cave: bool,
    /// How bright the stars are.
    pub stars: f32,
    /// The sun's (or moon's) disc in the sky.
    pub sun_disc: Option<Color>,
}

pub(crate) const fn c(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

pub fn theme(zone: Zone) -> Theme {
    match zone {
        // Autumn dusk.
        Zone::Amberfall => Theme {
            light: Light {
                sun_dir: vec3(-0.75, 0.28, 0.45).normalize(),
                sun: vec3(1.0, 0.76, 0.55),
                ambient: vec3(0.42, 0.4, 0.52),
            },
            sky_top: c(0.16, 0.15, 0.34),
            sky_mid: c(0.55, 0.36, 0.5),
            sky_horizon: c(0.98, 0.6, 0.36),
            fog: c(0.74, 0.5, 0.44),
            fog_near: 60.0,
            fog_far: 260.0,
            water: Color::new(0.32, 0.36, 0.55, 0.78),
            cave: false,
            stars: 0.6,
            sun_disc: Some(c(1.0, 0.88, 0.6)),
        },
        // Blazing desert afternoon.
        Zone::Scorchsand => Theme {
            light: Light {
                sun_dir: vec3(0.35, 0.8, 0.3).normalize(),
                sun: vec3(1.05, 0.95, 0.8),
                ambient: vec3(0.5, 0.46, 0.42),
            },
            sky_top: c(0.3, 0.52, 0.85),
            sky_mid: c(0.55, 0.72, 0.92),
            sky_horizon: c(0.96, 0.86, 0.66),
            fog: c(0.92, 0.8, 0.62),
            fog_near: 80.0,
            fog_far: 320.0,
            water: Color::new(0.2, 0.62, 0.66, 0.82),
            cave: false,
            stars: 0.0,
            sun_disc: Some(c(1.0, 0.98, 0.9)),
        },
        // Silver twilight under great trees.
        Zone::Silverbough => Theme {
            light: Light {
                sun_dir: vec3(0.3, 0.6, -0.6).normalize(),
                sun: vec3(0.78, 0.82, 1.0),
                ambient: vec3(0.38, 0.42, 0.55),
            },
            sky_top: c(0.08, 0.1, 0.28),
            sky_mid: c(0.22, 0.3, 0.55),
            sky_horizon: c(0.55, 0.62, 0.85),
            fog: c(0.4, 0.46, 0.68),
            fog_near: 50.0,
            fog_far: 230.0,
            water: Color::new(0.35, 0.55, 0.8, 0.75),
            cave: false,
            stars: 0.9,
            sun_disc: Some(c(0.9, 0.93, 1.0)),
        },
        // A dark cave lit by crystals and mushrooms.
        Zone::Grubdeep => Theme {
            light: Light {
                sun_dir: vec3(0.2, 1.0, 0.1).normalize(),
                sun: vec3(0.3, 0.32, 0.4),
                ambient: vec3(0.42, 0.42, 0.52),
            },
            sky_top: c(0.03, 0.03, 0.05),
            sky_mid: c(0.05, 0.05, 0.07),
            sky_horizon: c(0.07, 0.08, 0.1),
            fog: c(0.06, 0.07, 0.1),
            fog_near: 25.0,
            fog_far: 140.0,
            water: Color::new(0.08, 0.18, 0.24, 0.85),
            cave: true,
            stars: 0.0,
            sun_disc: None,
        },
        // A crisp, bright snowy day.
        Zone::Frostcog => Theme {
            light: Light {
                sun_dir: vec3(-0.4, 0.6, -0.5).normalize(),
                sun: vec3(0.95, 0.97, 1.0),
                ambient: vec3(0.52, 0.56, 0.66),
            },
            sky_top: c(0.38, 0.55, 0.82),
            sky_mid: c(0.62, 0.74, 0.9),
            sky_horizon: c(0.88, 0.92, 0.97),
            fog: c(0.86, 0.9, 0.96),
            fog_near: 55.0,
            fog_far: 240.0,
            water: Color::new(0.78, 0.86, 0.95, 0.95),
            cave: false,
            stars: 0.0,
            sun_disc: Some(c(1.0, 1.0, 0.95)),
        },
        // A sickly, dying forest under a purple sky.
        Zone::Witherwood => Theme {
            light: Light {
                sun_dir: vec3(0.6, 0.35, 0.5).normalize(),
                sun: vec3(0.72, 0.8, 0.62),
                ambient: vec3(0.44, 0.42, 0.5),
            },
            sky_top: c(0.1, 0.06, 0.16),
            sky_mid: c(0.24, 0.16, 0.3),
            sky_horizon: c(0.42, 0.48, 0.34),
            fog: c(0.34, 0.38, 0.3),
            fog_near: 35.0,
            fog_far: 190.0,
            water: Color::new(0.24, 0.32, 0.16, 0.9),
            cave: false,
            stars: 0.4,
            sun_disc: Some(c(0.75, 0.85, 0.6)),
        },
    }
}

/// Lights a surface facing `normal`.
fn shade(light: &Light, color: Color, normal: Vec3) -> [u8; 4] {
    let n = normal.normalize_or_zero();
    let d = n.dot(light.sun_dir).max(0.0);
    let sky = 0.5 + 0.5 * n.y;
    let l = light.sun * d * 0.95 + light.ambient * (0.6 + 0.4 * sky);
    [
        (color.r * l.x * 255.0).clamp(0.0, 255.0) as u8,
        (color.g * l.y * 255.0).clamp(0.0, 255.0) as u8,
        (color.b * l.z * 255.0).clamp(0.0, 255.0) as u8,
        (color.a * 255.0).clamp(0.0, 255.0) as u8,
    ]
}

/// Full brightness, for things that glow.
pub(crate) fn glow(color: Color) -> [u8; 4] {
    [
        (color.r * 255.0).clamp(0.0, 255.0) as u8,
        (color.g * 255.0).clamp(0.0, 255.0) as u8,
        (color.b * 255.0).clamp(0.0, 255.0) as u8,
        (color.a * 255.0).clamp(0.0, 255.0) as u8,
    ]
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

pub(crate) fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::new(r, g, b, 1.0)
}

/// Darker (or, above 1, lighter) version of a color.
pub(crate) fn dark(c: Color, f: f32) -> Color {
    Color::new(
        (c.r * f).min(1.0),
        (c.g * f).min(1.0),
        (c.b * f).min(1.0),
        c.a,
    )
}

/// Two axes perpendicular to `axis`, the first as close to `reference` as possible.
pub(crate) fn basis(axis: Vec3, reference: Vec3) -> (Vec3, Vec3) {
    let up = axis.normalize_or_zero();
    let mut u = reference - up * reference.dot(up);
    if u.length_squared() < 1e-6 {
        let helper = if up.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        u = up.cross(helper);
    }
    let u = u.normalize();
    (u, up.cross(u))
}

// ---- Batching ----

/// Collects triangles and draws them in as few calls as possible.
/// In recording mode it keeps the meshes instead, for static scenery.
pub struct Batch {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    recorded: Option<Vec<Mesh>>,
    /// Skip lighting: everything drawn is emissive.
    pub glowing: bool,
    pub light: Light,
}

impl Batch {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            recorded: None,
            glowing: false,
            light: theme(Zone::Amberfall).light,
        }
    }

    pub fn recording(light: Light) -> Self {
        Self {
            recorded: Some(Vec::new()),
            light,
            ..Self::new()
        }
    }

    pub fn finish(mut self) -> Vec<Mesh> {
        self.flush();
        self.recorded.unwrap_or_default()
    }

    /// Makes room for `v` vertices, returning the index of the first one.
    fn reserve(&mut self, v: usize, i: usize) -> u16 {
        if self.vertices.len() + v > MAX_VERTICES || self.indices.len() + i > MAX_INDICES {
            self.flush();
        }
        self.vertices.len() as u16
    }

    pub fn flush(&mut self) {
        if self.vertices.is_empty() {
            return;
        }
        let mesh = Mesh {
            vertices: std::mem::take(&mut self.vertices),
            indices: std::mem::take(&mut self.indices),
            texture: None,
        };
        match &mut self.recorded {
            Some(meshes) => meshes.push(mesh),
            None => {
                draw_mesh(&mesh);
                self.vertices = mesh.vertices;
                self.indices = mesh.indices;
                self.vertices.clear();
                self.indices.clear();
            }
        }
    }

    fn color(&self, color: Color, normal: Vec3) -> [u8; 4] {
        if self.glowing {
            glow(color)
        } else {
            shade(&self.light, color, normal)
        }
    }

    fn vertex(&mut self, position: Vec3, color: [u8; 4]) {
        self.vertices.push(Vertex {
            position,
            uv: Vec2::ZERO,
            color,
            normal: Vec4::ZERO,
        });
    }

    pub fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, color: Color) {
        let c = self.color(color, normal);
        let base = self.reserve(4, 6);
        for p in corners {
            self.vertex(p, c);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) {
        let mut n = (b - a).cross(c - a);
        if n.y < 0.0 {
            n = -n;
        }
        let col = self.color(color, n);
        let base = self.reserve(3, 3);
        self.vertex(a, col);
        self.vertex(b, col);
        self.vertex(c, col);
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    /// A box given its center and three half-axis vectors.
    pub fn cuboid(&mut self, center: Vec3, axes: [Vec3; 3], color: Color) {
        for i in 0..3 {
            let a = axes[(i + 1) % 3];
            let b = axes[(i + 2) % 3];
            for sign in [-1.0, 1.0] {
                let n = axes[i] * sign;
                let fc = center + n;
                self.quad([fc - a - b, fc + a - b, fc + a + b, fc - a + b], n, color);
            }
        }
    }

    /// An upright box rotated around Y.
    pub fn block(&mut self, center: Vec3, half: Vec3, yaw: f32, color: Color) {
        let f = forward(yaw);
        let r = vec3(-f.z, 0.0, f.x);
        self.cuboid(center, [r * half.x, Vec3::Y * half.y, f * half.z], color);
    }

    /// An ellipsoid along three (perpendicular) half-axis vectors, so it
    /// turns with whatever it's part of.
    pub fn ellipsoid_axes(&mut self, center: Vec3, axes: [Vec3; 3], color: Color) {
        const SLICES: usize = 12;
        const STACKS: usize = 8;
        let base = self.reserve((SLICES + 1) * (STACKS + 1), SLICES * STACKS * 6);
        let len = axes.map(|a| a.length().max(1e-4));
        for j in 0..=STACKS {
            let phi = std::f32::consts::PI * j as f32 / STACKS as f32;
            for i in 0..=SLICES {
                let theta = std::f32::consts::TAU * i as f32 / SLICES as f32;
                let (x, y, z) = (phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
                let p = center + axes[0] * x + axes[1] * y + axes[2] * z;
                let n = axes[0] * (x / (len[0] * len[0]))
                    + axes[1] * (y / (len[1] * len[1]))
                    + axes[2] * (z / (len[2] * len[2]));
                let c = self.color(color, n);
                self.vertex(p, c);
            }
        }
        let row = SLICES as u16 + 1;
        for j in 0..STACKS as u16 {
            for i in 0..SLICES as u16 {
                let a = base + j * row + i;
                let b = a + row;
                self.indices
                    .extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
    }

    pub fn ellipsoid(&mut self, center: Vec3, radii: Vec3, color: Color) {
        self.ellipsoid_axes(
            center,
            [Vec3::X * radii.x, Vec3::Y * radii.y, Vec3::Z * radii.z],
            color,
        );
    }

    pub fn sphere(&mut self, center: Vec3, radius: f32, color: Color) {
        self.ellipsoid(center, Vec3::splat(radius), color);
    }

    /// A cone (or with `top_radius` > 0, a tapered cylinder) along `axis`.
    /// Its facets line up with `reference`, so it turns with its model.
    #[allow(clippy::too_many_arguments)]
    pub fn cone_ref(
        &mut self,
        base_center: Vec3,
        axis: Vec3,
        reference: Vec3,
        radius: f32,
        top_radius: f32,
        sides: usize,
        color: Color,
    ) {
        let up = axis.normalize_or_zero();
        let (u, v) = basis(axis, reference);
        let top = base_center + axis;
        for i in 0..sides {
            let a0 = std::f32::consts::TAU * i as f32 / sides as f32;
            let a1 = std::f32::consts::TAU * (i + 1) as f32 / sides as f32;
            let d0 = u * a0.cos() + v * a0.sin();
            let d1 = u * a1.cos() + v * a1.sin();
            let n = (d0 + d1).normalize() + up * (radius - top_radius) / axis.length().max(0.01);
            let b0 = base_center + d0 * radius;
            let b1 = base_center + d1 * radius;
            if top_radius > 0.0 {
                self.quad(
                    [b0, b1, top + d1 * top_radius, top + d0 * top_radius],
                    n,
                    color,
                );
            } else {
                let c = self.color(color, n);
                let base = self.reserve(3, 3);
                self.vertex(b0, c);
                self.vertex(b1, c);
                self.vertex(top, c);
                self.indices.extend_from_slice(&[base, base + 1, base + 2]);
            }
        }
        if top_radius > 0.0 {
            self.disc(top, up, reference, top_radius, sides, color);
        }
    }

    pub fn cone(
        &mut self,
        base_center: Vec3,
        axis: Vec3,
        radius: f32,
        top_radius: f32,
        sides: usize,
        color: Color,
    ) {
        self.cone_ref(base_center, axis, Vec3::X, radius, top_radius, sides, color);
    }

    pub fn cylinder(
        &mut self,
        base_center: Vec3,
        axis: Vec3,
        radius: f32,
        sides: usize,
        color: Color,
    ) {
        self.cone(base_center, axis, radius, radius, sides, color);
    }

    fn disc(
        &mut self,
        center: Vec3,
        normal: Vec3,
        reference: Vec3,
        radius: f32,
        sides: usize,
        color: Color,
    ) {
        let (u, v) = basis(normal, reference);
        let c = self.color(color, normal);
        let base = self.reserve(sides + 1, sides * 3);
        self.vertex(center, c);
        for i in 0..sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            self.vertex(center + (u * a.cos() + v * a.sin()) * radius, c);
        }
        for i in 0..sides as u16 {
            self.indices.extend_from_slice(&[
                base,
                base + 1 + i,
                base + 1 + (i + 1) % sides as u16,
            ]);
        }
    }

    /// A square beam from `a` to `b`.
    pub fn beam(&mut self, a: Vec3, b: Vec3, half_width: f32, reference: Vec3, color: Color) {
        let d = (b - a) * 0.5;
        let (u, v) = basis(d, reference);
        self.cuboid((a + b) * 0.5, [u * half_width, d, v * half_width], color);
    }

    /// A flat ring lying on the terrain, unshaded.
    pub fn ground_ring(&mut self, center: Vec3, radius: f32, width: f32, color: Color) {
        let sides = 40;
        let c = glow(color);
        let base = self.reserve(sides * 2, sides * 6);
        for i in 0..sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            let d = vec3(a.cos(), 0.0, a.sin());
            for r in [radius - width * 0.5, radius + width * 0.5] {
                let p = center + d * r;
                self.vertex(
                    vec3(
                        p.x,
                        terrain_height(p.x, p.z).max(center.y - 0.5) + 0.08,
                        p.z,
                    ),
                    c,
                );
            }
        }
        for i in 0..sides as u16 {
            let a = base + i * 2;
            let b = base + ((i + 1) % sides as u16) * 2;
            self.indices
                .extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
        }
    }

    /// A ring floating in the air, facing up, unshaded.
    pub fn air_ring(&mut self, center: Vec3, radius: f32, width: f32, color: Color) {
        let sides = 32;
        let c = glow(color);
        let base = self.reserve(sides * 2, sides * 6);
        for i in 0..sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            let d = vec3(a.cos(), 0.0, a.sin());
            self.vertex(center + d * (radius - width * 0.5), c);
            self.vertex(center + d * (radius + width * 0.5), c);
        }
        for i in 0..sides as u16 {
            let a = base + i * 2;
            let b = base + ((i + 1) % sides as u16) * 2;
            self.indices
                .extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
        }
    }

    /// A sphere that ignores lighting (sparks, lamps, spell effects).
    pub fn glow_sphere(&mut self, center: Vec3, radius: f32, color: Color) {
        self.lit(|b| b.sphere(center, radius, color));
    }

    /// Draws something unlit.
    pub(crate) fn lit<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let was = self.glowing;
        self.glowing = true;
        let r = f(self);
        self.glowing = was;
        r
    }
}

// ---- Model building ----

/// A local coordinate frame for building a model out of parts. Every part
/// is built along the frame's own axes, so the whole model turns together.
#[derive(Clone, Copy)]
pub struct Frame {
    origin: Vec3,
    r: Vec3,
    u: Vec3,
    f: Vec3,
    scale: f32,
}

impl Frame {
    pub fn upright(pos: Vec3, yaw: f32, scale: f32) -> Self {
        let f = forward(yaw);
        Self {
            origin: pos,
            r: vec3(-f.z, 0.0, f.x),
            u: Vec3::Y,
            f,
            scale,
        }
    }

    /// Fallen over on its back (people) or side (animals).
    pub fn fallen(pos: Vec3, yaw: f32, scale: f32, on_side: bool) -> Self {
        let up = Self::upright(pos, yaw, scale);
        if on_side {
            Self {
                origin: pos + Vec3::Y * 0.25 * scale,
                r: Vec3::Y,
                u: -up.r,
                f: up.f,
                scale,
            }
        } else {
            Self {
                origin: pos + Vec3::Y * 0.2 * scale,
                r: up.r,
                u: -up.f,
                f: Vec3::Y,
                scale,
            }
        }
    }

    /// Upright, but leaning forward by `pitch` (back if negative) from the
    /// feet.
    pub fn leaning(pos: Vec3, yaw: f32, scale: f32, pitch: f32) -> Self {
        let up = Self::upright(pos, yaw, scale);
        let (sn, cs) = pitch.sin_cos();
        Self {
            origin: pos,
            r: up.r,
            u: up.u * cs + up.f * sn,
            f: up.f * cs - up.u * sn,
            scale,
        }
    }

    pub(crate) fn dir(&self, l: Vec3) -> Vec3 {
        self.r * l.x + self.u * l.y + self.f * l.z
    }

    /// A limb from `joint` along local direction `d`. Returns where it ends.
    pub fn segment(
        &self,
        b: &mut Batch,
        joint: Vec3,
        d: Vec3,
        half_width: f32,
        length: f32,
        color: Color,
    ) -> Vec3 {
        let s = self.scale;
        let wd = self.dir(d).normalize_or_zero();
        let (u, v) = basis(wd, self.r);
        b.cuboid(
            self.p(joint + d * length * 0.5),
            [
                u * half_width * s,
                wd * length * 0.5 * s,
                v * half_width * s,
            ],
            color,
        );
        joint + d * length
    }

    pub(crate) fn p(&self, l: Vec3) -> Vec3 {
        self.origin + self.dir(l) * self.scale
    }

    pub fn cube(&self, b: &mut Batch, center: Vec3, half: Vec3, color: Color) {
        let s = self.scale;
        b.cuboid(
            self.p(center),
            [
                self.r * half.x * s,
                self.u * half.y * s,
                self.f * half.z * s,
            ],
            color,
        );
    }

    /// A box tilted forward by `pitch` around the frame's right axis.
    pub fn tilted(&self, b: &mut Batch, center: Vec3, half: Vec3, pitch: f32, color: Color) {
        let s = self.scale;
        let up = vec3(0.0, pitch.cos(), pitch.sin());
        let fwd = vec3(0.0, -pitch.sin(), pitch.cos());
        b.cuboid(
            self.p(center),
            [
                self.r * half.x * s,
                self.dir(up) * half.y * s,
                self.dir(fwd) * half.z * s,
            ],
            color,
        );
    }

    /// A limb hanging from `joint`, swung forward by `angle` radians.
    /// Returns where it ends.
    pub fn limb(
        &self,
        b: &mut Batch,
        joint: Vec3,
        angle: f32,
        half_width: f32,
        length: f32,
        color: Color,
    ) -> Vec3 {
        let d = vec3(0.0, -angle.cos(), angle.sin());
        let p = vec3(0.0, angle.sin(), angle.cos());
        let s = self.scale;
        let center = joint + d * length * 0.5;
        b.cuboid(
            self.p(center),
            [
                self.r * half_width * s,
                self.dir(d) * length * 0.5 * s,
                self.dir(p) * half_width * s,
            ],
            color,
        );
        joint + d * length
    }

    /// A beam between two local points.
    pub fn beam(&self, b: &mut Batch, from: Vec3, to: Vec3, half_width: f32, color: Color) {
        b.beam(
            self.p(from),
            self.p(to),
            half_width * self.scale,
            self.f,
            color,
        );
    }

    pub fn sphere(&self, b: &mut Batch, center: Vec3, radius: f32, color: Color) {
        self.ellipsoid(b, center, Vec3::splat(radius), color);
    }

    pub fn ellipsoid(&self, b: &mut Batch, center: Vec3, radii: Vec3, color: Color) {
        let s = self.scale;
        b.ellipsoid_axes(
            self.p(center),
            [
                self.r * radii.x * s,
                self.u * radii.y * s,
                self.f * radii.z * s,
            ],
            color,
        );
    }

    pub fn glow(&self, b: &mut Batch, center: Vec3, radius: f32, color: Color) {
        b.lit(|b| self.sphere(b, center, radius, color));
    }

    pub fn cone(&self, b: &mut Batch, base: Vec3, height: f32, radius: f32, color: Color) {
        b.cone_ref(
            self.p(base),
            self.u * height * self.scale,
            self.f,
            radius * self.scale,
            0.0,
            10,
            color,
        );
    }

    /// A cone pointing along a local direction.
    pub fn cone_dir(&self, b: &mut Batch, base: Vec3, dir: Vec3, radius: f32, color: Color) {
        b.cone_ref(
            self.p(base),
            self.dir(dir) * self.scale,
            self.f,
            radius * self.scale,
            0.0,
            6,
            color,
        );
    }

    pub fn cylinder(&self, b: &mut Batch, base: Vec3, height: f32, radius: f32, color: Color) {
        let r = radius * self.scale;
        b.cone_ref(
            self.p(base),
            self.u * height * self.scale,
            self.f,
            r,
            r,
            10,
            color,
        );
    }

    /// A cylinder along a local direction.
    pub fn cylinder_dir(&self, b: &mut Batch, base: Vec3, dir: Vec3, radius: f32, color: Color) {
        let r = radius * self.scale;
        b.cone_ref(
            self.p(base),
            self.dir(dir) * self.scale,
            self.u,
            r,
            r,
            10,
            color,
        );
    }
}

// ---- Scenery ----

const FOG_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec2 uv;
varying lowp vec4 color;
varying mediump float depth;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    // For a perspective camera, clip-space w is the distance along the view.
    depth = gl_Position.w;
    color = color0 / 255.0;
    uv = texcoord;
}"#;

const FOG_FRAGMENT: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying lowp vec2 uv;
varying mediump float depth;

uniform sampler2D Texture;
uniform vec4 FogColor;
uniform float FogNear;
uniform float FogFar;

void main() {
    vec4 c = color * texture2D(Texture, uv);
    float f = clamp((depth - FogNear) / (FogFar - FogNear), 0.0, 1.0);
    gl_FragColor = vec4(mix(c.rgb, FogColor.rgb, f * f * (3.0 - 2.0 * f)), c.a);
}"#;

/// A lamp or fire that flickers, with its color.
#[derive(Clone, Copy)]
struct Glow {
    pos: Vec3,
    color: Color,
}

/// One zone's static scenery.
struct ZoneScene {
    meshes: Vec<Mesh>,
    water: Vec<Mesh>,
    chimneys: Vec<Vec3>,
    lamps: Vec<Glow>,
    fires: Vec<Glow>,
    flags: Vec<Flag>,
}

/// A banner's cloth, which waves in the wind.
#[derive(Clone, Copy)]
struct Flag {
    /// Where the cloth hangs from the pole.
    top: Vec3,
    /// The direction the cloth streams out.
    yaw: f32,
    color: Color,
}

/// Static scenery for every zone (built the first time it's seen) and the
/// fog material.
pub struct Scene {
    zones: [OnceCell<ZoneScene>; 6],
    fog: Option<Material>,
}

impl Scene {
    pub fn new() -> Self {
        let fog = load_material(
            ShaderSource::Glsl {
                vertex: FOG_VERTEX,
                fragment: FOG_FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    depth_write: true,
                    depth_test: Comparison::LessOrEqual,
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                uniforms: vec![
                    UniformDesc::new("FogColor", UniformType::Float4),
                    UniformDesc::new("FogNear", UniformType::Float1),
                    UniformDesc::new("FogFar", UniformType::Float1),
                ],
                ..Default::default()
            },
        )
        .ok();
        Self {
            zones: Default::default(),
            fog,
        }
    }

    fn get(&self, zone: Zone) -> &ZoneScene {
        self.zones[zone.index()].get_or_init(|| build_zone(zone))
    }

    /// Call after `set_camera` for a 3D pass: turns on the zone's fog.
    pub fn begin_3d(&self, zone: Zone) {
        if let Some(fog) = &self.fog {
            let t = theme(zone);
            fog.set_uniform("FogColor", vec4(t.fog.r, t.fog.g, t.fog.b, 1.0));
            fog.set_uniform("FogNear", t.fog_near);
            fog.set_uniform("FogFar", t.fog_far);
            gl_use_material(fog);
        }
    }

    pub fn end_3d(&self) {
        gl_use_default_material();
    }

    pub fn draw(&self, zone: Zone) {
        for m in &self.get(zone).meshes {
            draw_mesh(m);
        }
    }

    /// Smoke, flickering fires and lamp glows.
    pub fn draw_effects(&self, zone: Zone, b: &mut Batch, time: f32, around: Vec3) {
        let z = self.get(zone);
        for f in &z.flags {
            flag(b, f, time);
        }
        weather(b, zone, time, around);
        let smoke = if zone == Zone::Frostcog {
            c(0.85, 0.87, 0.9)
        } else {
            c(0.55, 0.52, 0.55)
        };
        for (i, ch) in z.chimneys.iter().enumerate() {
            for k in 0..5 {
                let t = (time * 0.25 + k as f32 / 5.0 + i as f32 * 0.37) % 1.0;
                let p = *ch + vec3((t * 7.0 + i as f32).sin() * 0.3 + t * 1.2, t * 4.5, t * 0.6);
                b.sphere(
                    p,
                    0.25 + t * 0.6,
                    Color::new(smoke.r, smoke.g, smoke.b, 0.55 * (1.0 - t)),
                );
            }
        }
        for l in &z.lamps {
            let flicker = 0.9 + (time * 9.0 + l.pos.x).sin() * 0.05;
            b.glow_sphere(l.pos, 0.22 * flicker, l.color);
            b.glow_sphere(
                l.pos,
                0.45 * flicker,
                Color::new(l.color.r, l.color.g, l.color.b, 0.25),
            );
        }
        for f in &z.fires {
            for k in 0..4 {
                let t = (time * 1.5 + k as f32 * 0.25) % 1.0;
                let p = f.pos + vec3((time * 5.0 + k as f32).sin() * 0.08, 0.3 + t * 1.0, 0.0);
                let fade = Color::new(f.color.r, f.color.g, f.color.b, 0.0);
                b.glow_sphere(
                    p,
                    0.3 * (1.0 - t * 0.6),
                    mix(Color::new(1.0, 0.95, 0.7, 0.95), fade, t),
                );
            }
        }
    }

    /// Draw last: it's see-through.
    pub fn draw_water(&self, zone: Zone) {
        for m in &self.get(zone).water {
            draw_mesh(m);
        }
    }
}

/// A banner's cloth: a grid of quads rippling along its length.
fn flag(b: &mut Batch, f: &Flag, time: f32) {
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

/// Small things drifting through the air around the camera: falling
/// leaves, blowing sand, fireflies, spores, snow and ghostly wisps. Each
/// particle has a fixed home in a tile that repeats across the world, so
/// they stay put as you walk through them.
fn weather(b: &mut Batch, zone: Zone, time: f32, around: Vec3) {
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

fn noise(x: f32, z: f32) -> f32 {
    ((x * 0.37).sin() * (z * 0.41).cos()
        + (x * 0.11 + z * 0.13).sin()
        + (x * 0.023 - z * 0.031).sin())
        / 3.0
}

/// A zone's ground colors.
struct Ground {
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

fn ground_colors(zone: Zone) -> Ground {
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

fn terrain_color(zone: Zone, g: &Ground, local: Vec2, h: f32, slope: f32) -> Color {
    let (x, z) = (local.x, local.y);
    let n = noise(x, z);
    let mut col = mix(g.main, g.var_a, (n * 3.0 + 0.2).max(0.0));
    col = mix(col, g.var_b, (-n * 3.0 - 0.3).max(0.0));
    col = mix(g.shore, col, (h - zone.water_level() - 0.3) / 1.2);
    col = mix(col, g.rock, (slope - 0.45) * 3.0);
    col = mix(col, g.rock, (h - 12.0) / 3.0);
    col = mix(col, g.peak, (h - 18.0) / 2.0);
    let d = local.length();
    let road = (1.0 - x.abs().min(z.abs()) / 3.0).clamp(0.0, 1.0)
        * (1.0 - (d - 70.0) / 20.0).clamp(0.0, 1.0);
    col = mix(col, g.road, road * 0.85);
    for &f in &zone.layout().fields {
        let inside = (1.0 - (local.distance(f) - 11.0) / 2.0).clamp(0.0, 1.0);
        let rows = ((x - f.x) * 1.6).sin() * 0.5 + 0.5;
        col = mix(col, mix(g.field, dark(g.field, 1.25), rows), inside);
    }
    mix(
        col,
        mix(g.town, dark(g.town, 1.1), n + 0.5),
        (TOWN_RADIUS - d) / 3.0,
    )
}

fn build_zone(zone: Zone) -> ZoneScene {
    let t = theme(zone);
    let mut b = Batch::recording(t.light);
    let mut chimneys = Vec::new();
    let mut lamps = Vec::new();
    let mut fires = Vec::new();
    let mut flags = Vec::new();
    terrain(&mut b, zone);
    ground_cover(&mut b, zone);
    for p in props::props(zone) {
        draw_prop(
            &mut b,
            zone,
            &p,
            &mut chimneys,
            &mut lamps,
            &mut fires,
            &mut flags,
        );
    }
    if t.cave {
        cave_ceiling(&mut b, zone);
    }
    let meshes = b.finish();

    let mut w = Batch::recording(t.light);
    let s = WORLD_HALF_SIZE;
    let col = glow(t.water);
    let center = zone.center();
    let base = w.reserve(4, 6);
    for (x, z) in [(-s, -s), (s, -s), (s, s), (-s, s)] {
        w.vertex(vec3(center.x + x, zone.water_level(), center.y + z), col);
    }
    w.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    ZoneScene {
        meshes,
        water: w.finish(),
        chimneys,
        lamps,
        fires,
        flags,
    }
}

fn terrain(b: &mut Batch, zone: Zone) {
    let g = ground_colors(zone);
    let step = 2.0;
    let n = (WORLD_HALF_SIZE * 2.0 / step) as usize;
    let rows_per_strip = 50;
    let mut row = 0;
    while row < n {
        let rows = rows_per_strip.min(n - row);
        b.flush();
        let base = b.reserve((rows + 1) * (n + 1), rows * n * 6);
        for j in row..=row + rows {
            for i in 0..=n {
                let local = vec2(
                    -WORLD_HALF_SIZE + i as f32 * step,
                    -WORLD_HALF_SIZE + j as f32 * step,
                );
                let w = zone.to_world(local);
                let h = terrain_height(w.x, w.y);
                let e = 0.5;
                let normal = vec3(
                    terrain_height(w.x - e, w.y) - terrain_height(w.x + e, w.y),
                    2.0 * e,
                    terrain_height(w.x, w.y - e) - terrain_height(w.x, w.y + e),
                )
                .normalize();
                let color = terrain_color(zone, &g, local, h, 1.0 - normal.y);
                let c = shade(&b.light, color, normal);
                b.vertex(vec3(w.x, h, w.y), c);
            }
        }
        let stride = n as u16 + 1;
        for j in 0..rows as u16 {
            for i in 0..n as u16 {
                let a = base + j * stride + i;
                let cc = a + stride;
                b.indices
                    .extend_from_slice(&[a, cc, a + 1, a + 1, cc, cc + 1]);
            }
        }
        b.flush();
        row += rows;
    }
}

/// Grass, leaves, pebbles, snow lumps and other little things on the ground.
fn ground_cover(b: &mut Batch, zone: Zone) {
    let mut rng = Scatter(0xBEEF + zone.index() as u32);
    let water = zone.water_level();
    for _ in 0..6000 {
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
                    b.block(
                        base + Vec3::Y * 0.06,
                        vec3(0.15, 0.08, 0.12),
                        kind * 9.0,
                        col,
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
                }
            }
            Zone::Scorchsand => {
                if kind < 0.35 {
                    let col = c(0.7 + shade_n, 0.62 + shade_n, 0.35);
                    for k in 0..3 {
                        let a = rng.range(0.0, std::f32::consts::TAU) + k as f32;
                        let side = vec3(a.cos(), 0.0, a.sin()) * 0.08;
                        let tip = base + vec3(a.cos() * 0.15, rng.range(0.2, 0.4), a.sin() * 0.15);
                        b.triangle(base - side, base + side, tip, col);
                    }
                } else if kind < 0.6 {
                    b.block(
                        base + Vec3::Y * 0.05,
                        vec3(0.12, 0.06, 0.1),
                        kind * 7.0,
                        c(0.6 + shade_n, 0.42, 0.3),
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
                let pick = |rng: &mut Scatter| {
                    leaves[(rng.unit() * leaves.len() as f32) as usize % leaves.len()]
                };
                if kind < 0.65 {
                    for k in 0..3 {
                        let a = rng.range(0.0, std::f32::consts::TAU) + k as f32;
                        let side = vec3(a.cos(), 0.0, a.sin()) * 0.12;
                        let tip = base + vec3(a.cos() * 0.15, rng.range(0.35, 0.6), a.sin() * 0.15);
                        b.triangle(base - side, base + side, tip, grass);
                    }
                } else if zone == Zone::Silverbough && kind < 0.75 {
                    // Glowing flowers.
                    let col = pick(&mut rng);
                    b.cylinder(base, Vec3::Y * 0.3, 0.02, 3, c(0.3, 0.55, 0.3));
                    b.glow_sphere(base + Vec3::Y * 0.32, 0.07, col);
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

/// The rock ceiling over the caverns, with stalactites.
fn cave_ceiling(b: &mut Batch, zone: Zone) {
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

/// Color of a zone's lamps and fires.
fn lamp_color(zone: Zone) -> Color {
    match zone {
        Zone::Amberfall | Zone::Frostcog => c(1.0, 0.85, 0.45),
        Zone::Scorchsand => c(1.0, 0.6, 0.2),
        Zone::Silverbough => c(0.7, 0.85, 1.0),
        Zone::Grubdeep => c(0.5, 1.0, 0.45),
        Zone::Witherwood => c(0.55, 1.0, 0.35),
    }
}

fn draw_prop(
    b: &mut Batch,
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
            Zone::Silverbough => regal_tree(b, &mut rng, pos, s),
            _ => autumn_tree(b, &mut rng, pos - Vec3::Y * 0.2, s),
        },
        PropKind::Conifer => {
            let (green, snowy) = match zone {
                Zone::Frostcog => (c(0.16, 0.3, 0.24), true),
                Zone::Silverbough => (c(0.4, 0.55, 0.55), false),
                _ => (c(0.14, 0.3, 0.2), false),
            };
            let p0 = pos - Vec3::Y * 0.2;
            b.cylinder(p0, Vec3::Y * 1.6 * s, 0.22 * s, 6, c(0.36, 0.25, 0.16));
            for (y, r, hgt) in [
                (1.0, 1.6, 2.2),
                (2.1, 1.3, 2.0),
                (3.2, 1.0, 1.8),
                (4.2, 0.7, 1.5),
            ] {
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
            b.block(pos + Vec3::Y * 0.4 * s, vec3(0.9, 0.6, 0.7) * s, yaw, grey);
            b.block(
                pos + vec3(0.5, 0.9, 0.2) * s,
                vec3(0.5, 0.35, 0.45) * s,
                yaw + 0.7,
                grey,
            );
            let top = match zone {
                Zone::Frostcog => c(0.96, 0.97, 1.0),
                Zone::Scorchsand => dark(grey, 1.15),
                Zone::Grubdeep => c(0.22, 0.32, 0.36),
                _ => c(0.4, 0.45, 0.22),
            };
            b.block(
                pos + vec3(-0.2, 1.02, -0.1) * s,
                vec3(0.55, 0.05, 0.4) * s,
                yaw,
                top,
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
            b.ellipsoid(pos + Vec3::Y * 0.5 * s, vec3(0.8, 0.55, 0.8) * s, col);
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
            b.cone(pos - Vec3::Y, Vec3::Y * 9.0 * s, 4.2 * s, 3.0 * s, 7, rock);
            b.cone(
                pos + Vec3::Y * 8.0 * s,
                Vec3::Y * 1.0 * s,
                3.1 * s,
                2.8 * s,
                7,
                dark(rock, 1.15),
            );
            for k in 0..3 {
                let r = (4.1 - k as f32 * 0.4) * s;
                b.cylinder(
                    pos + Vec3::Y * (2.0 + k as f32 * 2.5) * s,
                    Vec3::Y * 0.3,
                    r,
                    7,
                    dark(rock, 0.8),
                );
            }
        }
        PropKind::Stalagmite => {
            let rock = c(0.33, 0.31, 0.34);
            b.cone(
                pos - Vec3::Y * 0.2,
                Vec3::Y * 4.0 * s,
                0.7 * s,
                0.0,
                7,
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
}

/// The color of a zone's banners.
fn banner_color(zone: Zone) -> Color {
    match zone {
        Zone::Amberfall => c(0.2, 0.32, 0.65),
        Zone::Scorchsand => c(0.75, 0.15, 0.1),
        Zone::Silverbough => c(0.25, 0.55, 0.45),
        Zone::Grubdeep => c(0.85, 0.6, 0.15),
        Zone::Frostcog => c(0.85, 0.2, 0.25),
        Zone::Witherwood => c(0.35, 0.15, 0.45),
    }
}

fn autumn_tree(b: &mut Batch, rng: &mut Scatter, p: Vec3, size: f32) {
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
    b.ellipsoid(p + Vec3::Y * 3.2 * size, vec3(1.5, 1.2, 1.5) * size, leaves);
    b.sphere(p + vec3(0.9, 2.8, 0.4) * size, 0.95 * size, leaves2);
    b.sphere(p + vec3(-0.8, 2.9, -0.5) * size, 1.0 * size, leaves);
    b.sphere(p + vec3(0.1, 3.9, -0.2) * size, 0.85 * size, leaves2);
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
fn regal_tree(b: &mut Batch, rng: &mut Scatter, p: Vec3, size: f32) {
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
        b.ellipsoid(at, vec3(1.6, 1.0, 1.6) * s, col);
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

fn dead_tree(b: &mut Batch, zone: Zone, rng: &mut Scatter, p: Vec3, size: f32) {
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

fn crop(b: &mut Batch, zone: Zone, p: Vec3, s: f32, field: u8) {
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

fn scarecrow(b: &mut Batch, zone: Zone, p: Vec3) {
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

fn market_stall(b: &mut Batch, zone: Zone, p: Vec3, yaw: f32, v: u8) {
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

fn house(
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
        Zone::Scorchsand => {
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
        Zone::Amberfall => {
            let walls = [c(0.9, 0.84, 0.7), c(0.86, 0.78, 0.66), c(0.93, 0.88, 0.78)];
            let roofs = [c(0.62, 0.22, 0.15), c(0.3, 0.33, 0.45), c(0.48, 0.3, 0.18)];
            let i = v as usize % 3;
            timber_house(b, center, yaw, walls[i], roofs[i], chimneys, lamps, lamp);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn timber_house(
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

fn centerpiece(b: &mut Batch, zone: Zone, p: Vec3, fires: &mut Vec<Glow>, lamps: &mut Vec<Glow>) {
    match zone {
        Zone::Amberfall => {
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
        Zone::Scorchsand => {
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
