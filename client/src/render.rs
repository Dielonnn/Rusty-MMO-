//! 3D drawing: a triangle batcher with per-zone lighting and distance fog,
//! the scenery of all six starting areas, and the character and creature
//! models.

use std::cell::OnceCell;

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::models::{Mesh, Vertex, draw_mesh};
use macroquad::prelude::*;
use shared::data::{
    Appearance, Class, GiantStyle, HumanoidStyle, ItemId, MobModel, Race, Slot, item, items,
};
use shared::props::{self, Prop, PropKind, Scatter};
use shared::protocol::EntityKind;
use shared::world::*;

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

const fn c(r: f32, g: f32, b: f32) -> Color {
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
fn glow(color: Color) -> [u8; 4] {
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

fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::new(r, g, b, 1.0)
}

/// Darker (or, above 1, lighter) version of a color.
fn dark(c: Color, f: f32) -> Color {
    Color::new(
        (c.r * f).min(1.0),
        (c.g * f).min(1.0),
        (c.b * f).min(1.0),
        c.a,
    )
}

/// Two axes perpendicular to `axis`, the first as close to `reference` as possible.
fn basis(axis: Vec3, reference: Vec3) -> (Vec3, Vec3) {
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
    fn lit<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
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

    fn dir(&self, l: Vec3) -> Vec3 {
        self.r * l.x + self.u * l.y + self.f * l.z
    }

    fn p(&self, l: Vec3) -> Vec3 {
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

/// How a character is moving, for animation.
#[derive(Clone, Copy, Default)]
pub struct Pose {
    /// Walk cycle phase in radians.
    pub walk: f32,
    pub moving: bool,
    pub casting: bool,
    /// Attack animation progress, 0 (start) to 1 (done).
    pub swing: f32,
    pub dead: bool,
    pub time: f32,
}

/// Everything needed to draw a character or creature.
#[derive(Clone, Copy)]
pub struct Look {
    pub kind: EntityKind,
    pub appearance: Appearance,
    pub gear: [Option<ItemId>; 5],
    /// Varies small details between individuals.
    pub seed: u32,
}

/// Size and build of each race.
struct RaceShape {
    scale: f32,
    head: f32,
    /// Limb thickness.
    limbs: f32,
}

fn race_shape(race: Race) -> RaceShape {
    match race {
        Race::Human => RaceShape {
            scale: 1.0,
            head: 1.0,
            limbs: 1.0,
        },
        Race::Orc => RaceShape {
            scale: 1.1,
            head: 1.05,
            limbs: 1.2,
        },
        Race::Elf => RaceShape {
            scale: 1.05,
            head: 0.97,
            limbs: 0.9,
        },
        Race::Goblin => RaceShape {
            scale: 0.72,
            head: 1.35,
            limbs: 0.9,
        },
        Race::Gnome => RaceShape {
            scale: 0.66,
            head: 1.35,
            limbs: 1.0,
        },
        Race::Undead => RaceShape {
            scale: 0.98,
            head: 0.95,
            limbs: 0.8,
        },
    }
}

/// How tall something is, for nameplates and picking.
pub fn model_height(kind: EntityKind, appearance: Appearance) -> f32 {
    match kind {
        EntityKind::Player(_) | EntityKind::Merchant(_) => 2.1 * race_shape(appearance.race).scale,
        EntityKind::Mob { kind, .. } => match kind.template().model {
            MobModel::Wolf => 1.3,
            MobModel::Boar => 1.25,
            MobModel::Spider => 1.1,
            MobModel::Scorpion => 1.3,
            MobModel::Humanoid(_) => 2.1,
            MobModel::Giant(_) => 2.1 * 2.2,
        },
    }
}

/// Radius of the selection circle.
pub fn model_radius(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::Mob { kind, .. } => match kind.template().model {
            MobModel::Giant(_) => 2.2,
            MobModel::Humanoid(_) => 0.9,
            _ => 1.2,
        },
        _ => 0.9,
    }
}

pub fn draw_model(b: &mut Batch, look: &Look, pos: Vec3, yaw: f32, pose: Pose) {
    match look.kind {
        EntityKind::Player(class) => humanoid(b, pos, yaw, Outfit::Class(class), look, pose),
        EntityKind::Merchant(_) => humanoid(b, pos, yaw, Outfit::Merchant, look, pose),
        EntityKind::Mob { kind, .. } => {
            let t = kind.template();
            let colors = t.colors.map(rgb);
            match t.model {
                MobModel::Wolf => wolf(b, pos, yaw, colors, look.seed, pose),
                MobModel::Boar => boar(b, pos, yaw, colors, look.seed, pose),
                MobModel::Spider => spider(b, pos, yaw, colors, pose),
                MobModel::Scorpion => scorpion(b, pos, yaw, colors, pose),
                MobModel::Humanoid(style) => {
                    humanoid(b, pos, yaw, Outfit::Mob(style, colors), look, pose)
                }
                MobModel::Giant(style) => {
                    humanoid(b, pos, yaw, Outfit::Giant(style, colors), look, pose)
                }
            }
        }
    }
}

pub fn skin_color(race: Race, i: u8) -> Color {
    let palette = match race {
        Race::Human => [
            c(0.98, 0.84, 0.72),
            c(0.93, 0.74, 0.58),
            c(0.8, 0.6, 0.44),
            c(0.6, 0.42, 0.3),
            c(0.4, 0.27, 0.19),
        ],
        Race::Orc => [
            c(0.45, 0.6, 0.3),
            c(0.38, 0.52, 0.26),
            c(0.5, 0.56, 0.32),
            c(0.33, 0.45, 0.28),
            c(0.55, 0.47, 0.3),
        ],
        Race::Elf => [
            c(0.98, 0.89, 0.82),
            c(0.92, 0.8, 0.7),
            c(0.8, 0.68, 0.6),
            c(0.72, 0.64, 0.8),
            c(0.52, 0.5, 0.65),
        ],
        Race::Goblin => [
            c(0.5, 0.7, 0.3),
            c(0.42, 0.62, 0.28),
            c(0.62, 0.74, 0.36),
            c(0.35, 0.55, 0.32),
            c(0.58, 0.64, 0.25),
        ],
        Race::Gnome => [
            c(0.98, 0.83, 0.76),
            c(0.95, 0.75, 0.65),
            c(0.85, 0.65, 0.5),
            c(0.7, 0.5, 0.38),
            c(0.98, 0.88, 0.84),
        ],
        Race::Undead => [
            c(0.62, 0.66, 0.62),
            c(0.56, 0.6, 0.64),
            c(0.62, 0.56, 0.64),
            c(0.5, 0.52, 0.44),
            c(0.72, 0.72, 0.7),
        ],
    };
    palette[i as usize % 5]
}

pub fn hair_color(i: u8) -> Color {
    [
        c(0.12, 0.09, 0.07),
        c(0.38, 0.22, 0.12),
        c(0.85, 0.68, 0.35),
        c(0.62, 0.22, 0.1),
        c(0.75, 0.75, 0.75),
        c(0.95, 0.92, 0.85),
    ][i as usize % 6]
}

/// What a humanoid wears and carries.
#[derive(Clone, Copy)]
enum Outfit {
    Class(Class),
    Merchant,
    Mob(HumanoidStyle, [Color; 3]),
    Giant(GiantStyle, [Color; 3]),
}

const STEEL: Color = c(0.72, 0.74, 0.78);
const GOLD: Color = c(0.92, 0.74, 0.3);
const WOOD: Color = c(0.42, 0.28, 0.16);
const LEATHER: Color = c(0.42, 0.28, 0.17);
const BONE: Color = c(0.88, 0.85, 0.76);

/// Colors of a class's clothes: torso, legs, sleeves, boots, cape (if any).
fn class_colors(class: Class) -> (Color, Color, Color, Color, Option<Color>) {
    match class {
        Class::Barbarian => (
            c(0.55, 0.38, 0.24),
            c(0.35, 0.25, 0.17),
            c(0.55, 0.38, 0.24),
            c(0.3, 0.2, 0.13),
            None,
        ),
        Class::Fighter => (
            c(0.6, 0.62, 0.66),
            c(0.3, 0.28, 0.27),
            STEEL,
            c(0.3, 0.22, 0.16),
            Some(c(0.18, 0.3, 0.6)),
        ),
        Class::Paladin => (
            c(0.85, 0.85, 0.88),
            c(0.75, 0.75, 0.8),
            c(0.85, 0.85, 0.88),
            c(0.6, 0.55, 0.45),
            Some(c(0.9, 0.88, 0.8)),
        ),
        Class::Monk => (
            c(0.92, 0.55, 0.15),
            c(0.85, 0.48, 0.12),
            c(0.92, 0.55, 0.15),
            c(0.3, 0.22, 0.15),
            None,
        ),
        Class::Rogue => (
            c(0.2, 0.2, 0.22),
            c(0.17, 0.17, 0.19),
            c(0.2, 0.2, 0.22),
            c(0.14, 0.12, 0.11),
            None,
        ),
        Class::Ranger => (
            c(0.3, 0.42, 0.22),
            c(0.4, 0.3, 0.2),
            c(0.3, 0.42, 0.22),
            c(0.32, 0.22, 0.14),
            Some(c(0.22, 0.32, 0.18)),
        ),
        Class::Artificer => (
            c(0.5, 0.36, 0.22),
            c(0.3, 0.26, 0.22),
            c(0.82, 0.78, 0.7),
            c(0.25, 0.2, 0.16),
            None,
        ),
        Class::Bard => (
            c(0.15, 0.55, 0.6),
            c(0.6, 0.18, 0.45),
            c(0.15, 0.55, 0.6),
            c(0.4, 0.26, 0.15),
            Some(c(0.6, 0.18, 0.45)),
        ),
        Class::Cleric => (
            c(0.95, 0.93, 0.86),
            c(0.9, 0.88, 0.8),
            c(0.95, 0.93, 0.86),
            c(0.55, 0.45, 0.3),
            Some(c(0.9, 0.8, 0.45)),
        ),
        Class::Druid => (
            c(0.36, 0.48, 0.25),
            c(0.32, 0.4, 0.22),
            c(0.45, 0.35, 0.22),
            c(0.35, 0.25, 0.15),
            Some(c(0.3, 0.42, 0.22)),
        ),
        Class::Mage => (
            c(0.34, 0.24, 0.72),
            c(0.28, 0.2, 0.62),
            c(0.34, 0.24, 0.72),
            c(0.25, 0.18, 0.4),
            Some(c(0.24, 0.14, 0.5)),
        ),
        Class::Sorcerer => (
            c(0.62, 0.12, 0.16),
            c(0.5, 0.1, 0.14),
            c(0.62, 0.12, 0.16),
            c(0.25, 0.1, 0.1),
            Some(c(0.35, 0.06, 0.1)),
        ),
        Class::Warlock => (
            c(0.16, 0.1, 0.2),
            c(0.12, 0.08, 0.16),
            c(0.16, 0.1, 0.2),
            c(0.1, 0.08, 0.1),
            Some(c(0.28, 0.08, 0.3)),
        ),
    }
}

/// Classes that wear a long robe.
fn robed(class: Class) -> bool {
    matches!(
        class,
        Class::Mage | Class::Cleric | Class::Sorcerer | Class::Warlock | Class::Druid
    )
}

fn humanoid(b: &mut Batch, pos: Vec3, yaw: f32, outfit: Outfit, look: &Look, pose: Pose) {
    let a = look.appearance;
    let seed = look.seed;
    let person = matches!(outfit, Outfit::Class(_) | Outfit::Merchant);
    let (race, scale) = match outfit {
        Outfit::Class(_) | Outfit::Merchant => (a.race, race_shape(a.race).scale),
        Outfit::Giant(..) => (Race::Human, 2.2),
        Outfit::Mob(..) => (Race::Human, 1.0),
    };
    let shape = race_shape(race);
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, scale, false)
    } else {
        Frame::upright(pos, yaw, scale)
    };
    let skin = match outfit {
        Outfit::Class(_) | Outfit::Merchant => skin_color(race, a.skin),
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Bandit
            | HumanoidStyle::Mystic
            | HumanoidStyle::Raider
            | HumanoidStyle::Necromancer => skin_color(Race::Human, (seed % 4) as u8),
            HumanoidStyle::Shaman => skin_color(Race::Orc, (seed % 5) as u8),
            HumanoidStyle::Skeleton => BONE,
            _ => colors[0],
        },
        Outfit::Giant(_, colors) => colors[1],
    };
    let hair = if person {
        hair_color(a.hair_color)
    } else {
        hair_color((seed / 3 % 4) as u8)
    };
    let slender = a.body == 1 && person;

    // Clothes.
    let (mut torso, mut legs, mut arms, mut boots, cape) = match outfit {
        Outfit::Class(class) => class_colors(class),
        Outfit::Merchant => (
            c(0.5, 0.3, 0.2),
            c(0.35, 0.28, 0.22),
            c(0.85, 0.82, 0.72),
            c(0.3, 0.2, 0.14),
            None,
        ),
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Skeleton => (BONE, BONE, BONE, BONE, None),
            HumanoidStyle::Satyr | HumanoidStyle::Trickster => {
                (skin, colors[1], skin, c(0.15, 0.12, 0.1), None)
            }
            HumanoidStyle::Trogg | HumanoidStyle::Troll => (skin, colors[1], skin, colors[1], None),
            _ => (colors[0], colors[1], colors[0], dark(colors[1], 0.8), None),
        },
        Outfit::Giant(_, colors) => (colors[0], colors[1], dark(colors[0], 0.95), colors[1], None),
    };
    if let Outfit::Mob(HumanoidStyle::Bandit | HumanoidStyle::Raider, _) = outfit {
        // Fighters go bare-armed.
        arms = skin;
    }
    // Worn armor shows on the body.
    let worn = |s: Slot| look.gear[s.index()].map(|id| rgb(item(id).color));
    let bare_skin = matches!(
        outfit,
        Outfit::Giant(..) | Outfit::Mob(HumanoidStyle::Skeleton, _)
    );
    let mut hands = if bare_skin { legs } else { skin };
    if let Some(cc) = worn(Slot::Chest) {
        torso = cc;
        arms = dark(cc, 0.92);
    }
    if let Some(cc) = worn(Slot::Legs) {
        legs = cc;
    }
    if let Some(cc) = worn(Slot::Feet) {
        boots = cc;
    }
    if let Some(cc) = worn(Slot::Hands) {
        hands = cc;
    }
    let is_robed = match outfit {
        Outfit::Class(class) => robed(class) && worn(Slot::Chest).is_none(),
        Outfit::Mob(style, _) => matches!(
            style,
            HumanoidStyle::Mystic | HumanoidStyle::Necromancer | HumanoidStyle::TrollShaman
        ),
        _ => false,
    };
    let goat_legs = matches!(
        outfit,
        Outfit::Mob(HumanoidStyle::Satyr | HumanoidStyle::Trickster, _)
    );
    let skeletal = matches!(
        outfit,
        Outfit::Mob(HumanoidStyle::Skeleton, _) | Outfit::Giant(GiantStyle::Bone, _)
    );
    let limb = shape.limbs * if skeletal { 0.55 } else { 1.0 };

    let moving = pose.moving && !pose.dead;
    let stride = if moving { pose.walk.sin() * 0.65 } else { 0.0 };
    // Legs: thigh, shin with a bending knee, and a boot (or hoof).
    for (side, phase) in [(-1.0f32, 0.0f32), (1.0, std::f32::consts::PI)] {
        let swing = if moving {
            (pose.walk + phase).sin() * 0.65
        } else {
            0.0
        };
        let bend = if moving {
            (pose.walk + phase + 1.2).sin().max(0.0) * 0.9
        } else {
            0.0
        };
        let hip = vec3(0.13 * side * if slender { 1.1 } else { 1.0 }, 0.95, 0.0);
        let knee = fr.limb(b, hip, swing, 0.1 * limb, 0.46, legs);
        let hoof = if goat_legs { 0.6 } else { 0.0 };
        let ankle = fr.limb(b, knee, swing - bend + hoof, 0.085 * limb, 0.45, legs);
        if goat_legs {
            fr.cube(b, ankle, vec3(0.07, 0.06, 0.08), c(0.12, 0.1, 0.08));
        } else {
            fr.cube(
                b,
                ankle + vec3(0.0, -0.01, 0.07),
                vec3(0.085, 0.06, 0.15),
                boots,
            );
        }
    }
    if is_robed {
        let sway = stride * 0.15;
        fr.tilted(b, vec3(0.0, 0.6, 0.0), vec3(0.27, 0.36, 0.2), sway, legs);
        fr.tilted(
            b,
            vec3(0.0, 0.3, 0.0),
            vec3(0.3, 0.08, 0.23),
            sway * 1.5,
            dark(legs, 0.85),
        );
    }
    if goat_legs {
        fr.cube(
            b,
            vec3(0.0, 0.85, 0.0),
            vec3(0.28, 0.16, 0.18),
            dark(legs, 0.85),
        );
    }

    // Body.
    let y = if moving {
        (pose.walk * 2.0).sin().abs() * 0.03
    } else {
        0.0
    };
    let breathe = (pose.time * 1.6).sin() * 0.01;
    let hips_w = if slender { 0.25 } else { 0.24 } * shape.limbs.max(0.9);
    let chest_w = if slender { 0.25 } else { 0.3 } * shape.limbs.max(0.85);
    if skeletal {
        fr.cube(b, vec3(0.0, 1.0 + y, 0.0), vec3(0.2, 0.06, 0.1), BONE);
        fr.cube(b, vec3(0.0, 1.25 + y, -0.05), vec3(0.04, 0.22, 0.04), BONE);
        for k in 0..4 {
            let yy = 1.28 + k as f32 * 0.07;
            fr.cube(
                b,
                vec3(0.0, yy + y, 0.02),
                vec3(0.2 - k as f32 * 0.01, 0.018, 0.12),
                BONE,
            );
        }
    } else {
        fr.cube(b, vec3(0.0, 1.0 + y, 0.0), vec3(hips_w, 0.1, 0.15), legs);
        fr.cube(
            b,
            vec3(0.0, 1.18 + y, 0.0),
            vec3(hips_w - 0.02, 0.1, 0.14),
            torso,
        );
        fr.cube(
            b,
            vec3(0.0, 1.4 + y + breathe, 0.0),
            vec3(chest_w, 0.15, 0.17),
            torso,
        );
        fr.cube(
            b,
            vec3(0.0, 1.07 + y, 0.0),
            vec3(hips_w + 0.01, 0.04, 0.155),
            c(0.25, 0.17, 0.1),
        );
        fr.cube(b, vec3(0.0, 1.07 + y, 0.16), vec3(0.045, 0.035, 0.01), GOLD);
    }
    if let Some(cc) = cape.filter(|_| worn(Slot::Chest).is_none()) {
        fr.tilted(
            b,
            vec3(0.0, 1.13 + y, -0.21),
            vec3(0.25, 0.44, 0.02),
            -0.08 - stride.abs() * 0.14,
            cc,
        );
    }

    // Neck and head.
    let hs = shape.head;
    fr.cylinder(
        b,
        vec3(0.0, 1.53 + y, 0.0),
        0.1,
        0.075 * limb.max(0.8),
        skin,
    );
    let head = vec3(0.0, 1.55 + 0.21 * hs + y, 0.01);
    match outfit {
        Outfit::Giant(style, colors) => giant_head(b, &fr, head, style, colors, pose),
        _ => face(b, &fr, head, hs, race, skin, hair, outfit, skeletal),
    }

    // Hair (players pick a style; mobs vary by seed).
    let style = match outfit {
        Outfit::Class(_) | Outfit::Merchant => a.hair_style,
        Outfit::Mob(HumanoidStyle::Bandit | HumanoidStyle::Raider, _) => 1 + (seed % 2) as u8,
        _ => 0,
    };
    let headgear = worn(Slot::Head);
    if headgear.is_none() && !matches!(outfit, Outfit::Giant(..)) {
        hair_style(b, &fr, head, hs, style, hair, pose, stride);
        // Gnomes with a broad build grow a beard.
        if person && race == Race::Gnome && a.body == 0 {
            fr.ellipsoid(
                b,
                head + vec3(0.0, -0.17, 0.12) * hs,
                vec3(0.13, 0.12, 0.08) * hs,
                hair,
            );
        }
    }
    match headgear {
        Some(cc) => {
            if look.gear[Slot::Head.index()] == Some(items::LINEN_HOOD) {
                fr.cube(
                    b,
                    head + vec3(0.0, 0.08, -0.03) * hs,
                    vec3(0.23, 0.18, 0.21) * hs,
                    cc,
                );
                fr.cone(
                    b,
                    head + vec3(0.0, 0.2, -0.06) * hs,
                    0.22 * hs,
                    0.16 * hs,
                    cc,
                );
            } else {
                fr.cube(
                    b,
                    head + vec3(0.0, 0.15, 0.0) * hs,
                    vec3(0.215, 0.08, 0.215) * hs,
                    cc,
                );
                fr.cube(
                    b,
                    head + vec3(0.0, 0.09, 0.2) * hs,
                    vec3(0.2, 0.025, 0.04) * hs,
                    dark(cc, 0.8),
                );
            }
        }
        None => headwear(b, &fr, head, hs, outfit, pose),
    }

    // Arms: swing while walking, raise while casting, chop when attacking.
    let mut left = -stride * 0.8;
    let mut right = stride * 0.8;
    let mut l_elbow = 0.25;
    let mut r_elbow = 0.25;
    if pose.casting {
        let wobble = (pose.time * 6.0).sin() * 0.1;
        left = 1.0 + wobble;
        right = 1.0 - wobble;
        l_elbow = 0.5;
        r_elbow = 0.5;
    }
    if pose.swing > 0.0 && pose.swing < 1.0 {
        let t = pose.swing;
        right = if t < 0.35 {
            2.7 * (t / 0.35)
        } else {
            2.7 * (1.0 - (t - 0.35) / 0.65) + 0.3
        };
        r_elbow = if t < 0.35 { 0.9 } else { 0.2 };
    }
    let shoulder_y = 1.53 + y;
    let sw = chest_w + 0.08;
    let mut hand_pos = [Vec3::ZERO; 2];
    for (i, (side, angle, elbow)) in [(-1.0f32, left, l_elbow), (1.0, right, r_elbow)]
        .into_iter()
        .enumerate()
    {
        let shoulder = vec3(sw * side, shoulder_y, 0.0);
        fr.sphere(b, shoulder, 0.1 * limb.max(0.7), arms);
        let el = fr.limb(b, shoulder, angle, 0.08 * limb, 0.34, arms);
        let wrist = fr.limb(
            b,
            el,
            angle + elbow,
            0.07 * limb,
            0.32,
            if is_robed { arms } else { dark(arms, 0.95) },
        );
        fr.sphere(
            b,
            wrist + vec3(0.0, -0.04, 0.0),
            0.075 * limb.max(0.8),
            hands,
        );
        hand_pos[i] = wrist + vec3(0.0, -0.05, 0.0);
    }
    let hands = Hands {
        left: hand_pos[0],
        right: hand_pos[1],
        left_angle: left + l_elbow + 1.55,
        right_angle: right + r_elbow + 1.55,
        shoulder_y,
        sw,
        y,
        chest_w,
    };
    gear_and_weapons(b, &fr, outfit, &hands, look, pose, legs);
    if look.gear[Slot::Chest.index()] == Some(items::HEARTSTONE_CHESTGUARD) {
        fr.glow(b, vec3(0.0, 1.42 + y, 0.18), 0.06, c(0.35, 0.9, 1.0));
        for sx in [-1.0, 1.0] {
            fr.ellipsoid(
                b,
                vec3(sw * sx, shoulder_y + 0.06, 0.0),
                vec3(0.15, 0.09, 0.15),
                c(0.45, 0.47, 0.52),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn face(
    b: &mut Batch,
    fr: &Frame,
    head: Vec3,
    hs: f32,
    race: Race,
    skin: Color,
    hair: Color,
    outfit: Outfit,
    skeletal: bool,
) {
    let person = matches!(outfit, Outfit::Class(_) | Outfit::Merchant);
    let undead = (person && race == Race::Undead) || skeletal;
    if skeletal {
        fr.ellipsoid(b, head, vec3(0.17, 0.2, 0.18) * hs, BONE);
        fr.cube(
            b,
            head + vec3(0.0, -0.15, 0.06) * hs,
            vec3(0.12, 0.05, 0.1) * hs,
            dark(BONE, 0.85),
        );
    } else {
        let jaw = if race == Race::Orc { 1.1 } else { 1.0 };
        fr.ellipsoid(b, head, vec3(0.19 * jaw, 0.21, 0.2) * hs, skin);
    }
    let eye_glow = match outfit {
        Outfit::Mob(
            HumanoidStyle::Skeleton | HumanoidStyle::Necromancer | HumanoidStyle::Trickster,
            colors,
        ) => Some(colors[2]),
        _ if undead => Some(c(0.95, 0.9, 0.55)),
        _ => None,
    };
    let tusks = (person && race == Race::Orc)
        || matches!(
            outfit,
            Outfit::Mob(
                HumanoidStyle::Troll | HumanoidStyle::TrollShaman | HumanoidStyle::Trogg,
                _
            )
        );
    for sx in [-1.0, 1.0] {
        let eye = head + vec3(0.07 * sx, 0.03, 0.17) * hs;
        match eye_glow {
            Some(g) => {
                fr.sphere(b, eye, 0.04 * hs, c(0.08, 0.06, 0.08));
                fr.glow(b, eye + vec3(0.0, 0.0, 0.02) * hs, 0.022 * hs, g);
            }
            None => {
                fr.sphere(b, eye, 0.035 * hs, c(0.95, 0.95, 0.95));
                fr.sphere(
                    b,
                    eye + vec3(0.0, 0.0, 0.025) * hs,
                    0.018 * hs,
                    c(0.12, 0.1, 0.1),
                );
            }
        }
        if !skeletal {
            fr.cube(
                b,
                head + vec3(0.07 * sx, 0.085, 0.18) * hs,
                vec3(0.04, 0.012, 0.012) * hs,
                dark(hair, 0.9),
            );
        }
        // Ears.
        let ear = head + vec3(0.18 * sx, 0.02, -0.02) * hs;
        match race {
            _ if !person => {
                if !skeletal {
                    fr.sphere(b, head + vec3(0.19 * sx, 0.0, 0.0) * hs, 0.04 * hs, skin);
                }
            }
            Race::Elf => fr.cone_dir(b, ear, vec3(0.2 * sx, 0.12, -0.08) * hs, 0.045 * hs, skin),
            Race::Goblin => fr.cone_dir(b, ear, vec3(0.3 * sx, 0.06, -0.04) * hs, 0.07 * hs, skin),
            Race::Orc => fr.cone_dir(b, ear, vec3(0.1 * sx, 0.06, -0.03) * hs, 0.05 * hs, skin),
            _ => fr.sphere(b, head + vec3(0.19 * sx, 0.0, 0.0) * hs, 0.04 * hs, skin),
        }
        if tusks {
            fr.cone_dir(
                b,
                head + vec3(0.06 * sx, -0.12, 0.15) * hs,
                vec3(0.01 * sx, 0.09, 0.02) * hs,
                0.02 * hs,
                c(0.96, 0.94, 0.85),
            );
        }
    }
    if skeletal {
        fr.cube(
            b,
            head + vec3(0.0, -0.06, 0.17) * hs,
            vec3(0.02, 0.03, 0.01) * hs,
            c(0.1, 0.08, 0.08),
        );
        return;
    }
    // Nose and mouth.
    match race {
        Race::Goblin if person => fr.cone_dir(
            b,
            head + vec3(0.0, -0.01, 0.18) * hs,
            vec3(0.0, -0.02, 0.14) * hs,
            0.04 * hs,
            dark(skin, 0.9),
        ),
        Race::Gnome if person => fr.sphere(
            b,
            head + vec3(0.0, -0.02, 0.2) * hs,
            0.05 * hs,
            dark(skin, 0.95),
        ),
        _ => fr.cube(
            b,
            head + vec3(0.0, -0.015, 0.205) * hs,
            vec3(0.022, 0.04, 0.025) * hs,
            dark(skin, 0.9),
        ),
    }
    let mouth = if undead {
        c(0.2, 0.12, 0.15)
    } else {
        c(0.55, 0.28, 0.25)
    };
    let w = if race == Race::Goblin && person {
        0.07
    } else {
        0.045
    };
    fr.cube(
        b,
        head + vec3(0.0, -0.09, 0.18) * hs,
        vec3(w, 0.01, 0.01) * hs,
        mouth,
    );
}

#[allow(clippy::too_many_arguments)]
fn hair_style(
    b: &mut Batch,
    fr: &Frame,
    head: Vec3,
    hs: f32,
    style: u8,
    hair: Color,
    pose: Pose,
    stride: f32,
) {
    let top = head + vec3(0.0, 0.17, -0.01) * hs;
    match style {
        1 => {
            fr.cube(b, top, vec3(0.2, 0.06, 0.2) * hs, hair);
            fr.cube(
                b,
                head + vec3(0.0, 0.05, -0.13) * hs,
                vec3(0.2, 0.13, 0.08) * hs,
                hair,
            );
        }
        2 => {
            fr.cube(b, top, vec3(0.21, 0.06, 0.21) * hs, hair);
            fr.cube(
                b,
                head + vec3(0.0, -0.08, -0.14) * hs,
                vec3(0.21, 0.26, 0.07) * hs,
                hair,
            );
            for sx in [-1.0, 1.0] {
                fr.cube(
                    b,
                    head + vec3(0.19 * sx, -0.04, -0.03) * hs,
                    vec3(0.03, 0.2, 0.09) * hs,
                    hair,
                );
            }
        }
        3 => {
            fr.cube(b, top, vec3(0.2, 0.06, 0.2) * hs, hair);
            fr.cube(
                b,
                head + vec3(0.0, 0.05, -0.14) * hs,
                vec3(0.19, 0.12, 0.07) * hs,
                hair,
            );
            let sway = (pose.time * 2.0).sin() * 0.1 + stride * 0.2;
            let tie = head + vec3(0.0, 0.05, -0.21) * hs;
            fr.sphere(b, tie, 0.05 * hs, dark(hair, 0.8));
            fr.limb(b, tie, -0.35 + sway, 0.045 * hs, 0.32 * hs, hair);
        }
        4 => {
            fr.cube(
                b,
                head + vec3(0.0, 0.2, -0.02) * hs,
                vec3(0.035, 0.08, 0.19) * hs,
                hair,
            );
        }
        _ => {}
    }
}

fn headwear(b: &mut Batch, fr: &Frame, head: Vec3, hs: f32, outfit: Outfit, pose: Pose) {
    let h = |v: Vec3| head + v * hs;
    match outfit {
        Outfit::Class(class) => match class {
            Class::Barbarian => {
                // War paint and a leather headband.
                fr.cube(
                    b,
                    h(vec3(0.0, 0.1, 0.0)),
                    vec3(0.205, 0.025, 0.205) * hs,
                    c(0.45, 0.32, 0.2),
                );
                for sx in [-1.0, 1.0] {
                    fr.cube(
                        b,
                        h(vec3(0.09 * sx, 0.0, 0.19)),
                        vec3(0.015, 0.05, 0.01) * hs,
                        c(0.75, 0.12, 0.1),
                    );
                }
            }
            Class::Fighter => {
                fr.cube(
                    b,
                    h(vec3(0.0, 0.16, 0.0)),
                    vec3(0.22, 0.07, 0.22) * hs,
                    STEEL,
                );
                fr.cube(
                    b,
                    h(vec3(0.0, 0.1, 0.0)),
                    vec3(0.225, 0.02, 0.225) * hs,
                    dark(STEEL, 0.8),
                );
            }
            Class::Paladin | Class::Cleric => {
                fr.cylinder(b, h(vec3(0.0, 0.17, 0.0)), 0.04 * hs, 0.21 * hs, GOLD);
                fr.glow(b, h(vec3(0.0, 0.2, 0.2)), 0.03 * hs, c(1.0, 0.95, 0.6));
            }
            Class::Monk => {
                let red = c(0.85, 0.15, 0.12);
                fr.cube(
                    b,
                    h(vec3(0.0, 0.1, 0.0)),
                    vec3(0.205, 0.03, 0.205) * hs,
                    red,
                );
                fr.limb(
                    b,
                    h(vec3(0.0, 0.1, -0.21)),
                    -0.6 + (pose.time * 3.0).sin() * 0.1,
                    0.03 * hs,
                    0.25 * hs,
                    red,
                );
            }
            Class::Rogue => {
                fr.cube(
                    b,
                    h(vec3(0.0, -0.075, 0.12)),
                    vec3(0.195, 0.065, 0.1) * hs,
                    c(0.12, 0.12, 0.14),
                );
            }
            Class::Ranger => {
                // A hood thrown back over the shoulders.
                fr.cube(
                    b,
                    h(vec3(0.0, -0.12, -0.17)),
                    vec3(0.2, 0.1, 0.08) * hs,
                    c(0.22, 0.32, 0.18),
                );
            }
            Class::Artificer => {
                // Goggles pushed up on the forehead.
                fr.cube(
                    b,
                    h(vec3(0.0, 0.1, 0.0)),
                    vec3(0.205, 0.025, 0.205) * hs,
                    LEATHER,
                );
                for sx in [-1.0, 1.0] {
                    fr.cylinder_dir(
                        b,
                        h(vec3(0.07 * sx, 0.1, 0.18)),
                        vec3(0.0, 0.0, 0.05) * hs,
                        0.05 * hs,
                        c(0.7, 0.55, 0.25),
                    );
                    b.lit(|b| {
                        fr.cylinder_dir(
                            b,
                            h(vec3(0.07 * sx, 0.1, 0.23)),
                            vec3(0.0, 0.0, 0.01) * hs,
                            0.04 * hs,
                            c(0.5, 0.85, 1.0),
                        )
                    });
                }
            }
            Class::Bard => {
                // A wide hat with a feather.
                let hat = c(0.6, 0.18, 0.45);
                fr.cylinder(b, h(vec3(0.0, 0.14, 0.0)), 0.04 * hs, 0.3 * hs, hat);
                fr.ellipsoid(b, h(vec3(0.0, 0.22, 0.0)), vec3(0.2, 0.08, 0.2) * hs, hat);
                fr.beam(
                    b,
                    h(vec3(0.15, 0.2, -0.05)),
                    h(vec3(0.3, 0.55, -0.25)),
                    0.02 * hs,
                    c(0.95, 0.85, 0.3),
                );
            }
            Class::Druid => {
                let antler = c(0.6, 0.5, 0.35);
                for sx in [-1.0, 1.0] {
                    let base = h(vec3(0.12 * sx, 0.15, 0.0));
                    let tip = h(vec3(0.28 * sx, 0.45, -0.05));
                    fr.beam(b, base, tip, 0.025 * hs, antler);
                    fr.beam(
                        b,
                        base.lerp(tip, 0.5),
                        h(vec3(0.3 * sx, 0.3, 0.1)),
                        0.02 * hs,
                        antler,
                    );
                }
                fr.cube(
                    b,
                    h(vec3(0.0, 0.11, 0.0)),
                    vec3(0.205, 0.02, 0.205) * hs,
                    c(0.3, 0.5, 0.2),
                );
            }
            Class::Mage => {
                // The pointed hat is built in the head's frame, so it turns with it.
                let hat = c(0.27, 0.17, 0.6);
                fr.cylinder(b, h(vec3(0.0, 0.14, 0.0)), 0.04 * hs, 0.38 * hs, hat);
                fr.cone(b, h(vec3(0.0, 0.17, 0.0)), 0.65 * hs, 0.23 * hs, hat);
                fr.cylinder(b, h(vec3(0.0, 0.18, 0.0)), 0.05 * hs, 0.235 * hs, GOLD);
            }
            Class::Sorcerer => {
                fr.cylinder(b, h(vec3(0.0, 0.17, 0.0)), 0.04 * hs, 0.21 * hs, GOLD);
                fr.cone_dir(
                    b,
                    h(vec3(0.0, 0.21, 0.14)),
                    vec3(0.0, 0.12, 0.03) * hs,
                    0.03 * hs,
                    GOLD,
                );
            }
            Class::Warlock => {
                // A deep hood with small horns.
                fr.cube(
                    b,
                    h(vec3(0.0, 0.07, -0.04)),
                    vec3(0.22, 0.18, 0.2) * hs,
                    c(0.12, 0.08, 0.14),
                );
                for sx in [-1.0, 1.0] {
                    fr.cone_dir(
                        b,
                        h(vec3(0.12 * sx, 0.2, 0.05)),
                        vec3(0.08 * sx, 0.18, -0.08) * hs,
                        0.04 * hs,
                        c(0.2, 0.15, 0.15),
                    );
                }
            }
        },
        Outfit::Merchant => {
            // A feathered cap.
            fr.cylinder(
                b,
                h(vec3(0.0, 0.15, 0.0)),
                0.07 * hs,
                0.21 * hs,
                c(0.2, 0.42, 0.3),
            );
            fr.beam(
                b,
                h(vec3(0.15, 0.2, 0.0)),
                h(vec3(0.25, 0.42, -0.2)),
                0.02 * hs,
                c(0.9, 0.3, 0.2),
            );
        }
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Bandit => {
                fr.cube(
                    b,
                    h(vec3(0.0, -0.06, 0.15)),
                    vec3(0.19, 0.07, 0.07) * hs,
                    colors[2],
                );
            }
            HumanoidStyle::Mystic | HumanoidStyle::Necromancer => {
                let hood = dark(colors[0], 0.7);
                fr.cube(
                    b,
                    h(vec3(0.0, 0.06, -0.04)),
                    vec3(0.23, 0.2, 0.21) * hs,
                    hood,
                );
                fr.cone(b, h(vec3(0.0, 0.22, -0.08)), 0.32 * hs, 0.17 * hs, hood);
            }
            HumanoidStyle::Raider => {
                // A turban and face wrap.
                fr.ellipsoid(
                    b,
                    h(vec3(0.0, 0.15, -0.01)),
                    vec3(0.23, 0.12, 0.23) * hs,
                    c(0.92, 0.88, 0.78),
                );
                fr.cube(
                    b,
                    h(vec3(0.0, -0.07, 0.14)),
                    vec3(0.19, 0.07, 0.07) * hs,
                    colors[2],
                );
            }
            HumanoidStyle::Shaman | HumanoidStyle::TrollShaman | HumanoidStyle::TroggShaman => {
                // A feathered headdress.
                for k in 0..5 {
                    let a = (k as f32 - 2.0) * 0.35;
                    let col = if k % 2 == 0 {
                        colors[2]
                    } else {
                        c(0.95, 0.92, 0.85)
                    };
                    fr.beam(
                        b,
                        h(vec3(a * 0.3, 0.15, -0.05)),
                        h(vec3(a * 0.6, 0.5, -0.15)),
                        0.025 * hs,
                        col,
                    );
                }
            }
            HumanoidStyle::Satyr | HumanoidStyle::Trickster => {
                let horn = c(0.85, 0.8, 0.7);
                for sx in [-1.0, 1.0] {
                    let bend = h(vec3(0.2 * sx, 0.32, -0.05));
                    fr.beam(b, h(vec3(0.1 * sx, 0.15, 0.03)), bend, 0.035 * hs, horn);
                    fr.beam(b, bend, h(vec3(0.26 * sx, 0.3, -0.2)), 0.025 * hs, horn);
                }
            }
            HumanoidStyle::Troll => {
                fr.cube(
                    b,
                    h(vec3(0.0, 0.2, -0.02)),
                    vec3(0.04, 0.09, 0.18) * hs,
                    c(0.85, 0.3, 0.2),
                );
            }
            HumanoidStyle::Trogg | HumanoidStyle::Skeleton => {}
        },
        Outfit::Giant(..) => {}
    }
}

fn giant_head(
    b: &mut Batch,
    fr: &Frame,
    head: Vec3,
    style: GiantStyle,
    colors: [Color; 3],
    pose: Pose,
) {
    let glow_c = Color::new(
        colors[2].r,
        colors[2].g,
        colors[2].b,
        0.7 + 0.3 * (pose.time * 2.0).sin().abs(),
    );
    match style {
        GiantStyle::Yeti => {
            fr.ellipsoid(b, head, vec3(0.24, 0.24, 0.24), colors[0]);
            fr.ellipsoid(
                b,
                head + vec3(0.0, -0.03, 0.16),
                vec3(0.14, 0.12, 0.08),
                c(0.55, 0.65, 0.8),
            );
            for sx in [-1.0, 1.0] {
                fr.cone_dir(
                    b,
                    head + vec3(0.17 * sx, 0.14, 0.0),
                    vec3(0.15 * sx, 0.12, 0.1),
                    0.05,
                    c(0.75, 0.7, 0.62),
                );
                fr.glow(b, head + vec3(0.07 * sx, 0.03, 0.22), 0.035, glow_c);
            }
        }
        GiantStyle::Bone => {
            fr.ellipsoid(b, head, vec3(0.2, 0.22, 0.22), colors[0]);
            fr.cube(
                b,
                head + vec3(0.0, -0.17, 0.06),
                vec3(0.14, 0.05, 0.12),
                dark(colors[0], 0.85),
            );
            for sx in [-1.0, 1.0] {
                fr.sphere(
                    b,
                    head + vec3(0.08 * sx, 0.03, 0.18),
                    0.05,
                    c(0.08, 0.06, 0.08),
                );
                fr.glow(b, head + vec3(0.08 * sx, 0.03, 0.2), 0.03, glow_c);
                fr.cone_dir(
                    b,
                    head + vec3(0.15 * sx, 0.15, 0.0),
                    vec3(0.12 * sx, 0.25, -0.05),
                    0.04,
                    colors[0],
                );
            }
        }
        GiantStyle::Treant => {
            fr.cube(
                b,
                head + vec3(0.0, 0.03, 0.0),
                vec3(0.2, 0.22, 0.19),
                colors[0],
            );
            // A leafy crown.
            for k in 0..5 {
                let a = k as f32 * 1.26;
                fr.sphere(
                    b,
                    head + vec3(a.cos() * 0.2, 0.3, a.sin() * 0.2),
                    0.2,
                    colors[2],
                );
            }
            for sx in [-1.0, 1.0] {
                fr.glow(b, head + vec3(0.08 * sx, 0.05, 0.2), 0.04, c(0.6, 1.0, 0.4));
            }
        }
        _ => {
            fr.cube(
                b,
                head + vec3(0.0, 0.03, 0.0),
                vec3(0.2, 0.19, 0.19),
                colors[0],
            );
            fr.cube(
                b,
                head + vec3(0.0, 0.17, -0.02),
                vec3(0.16, 0.05, 0.15),
                dark(colors[0], 0.85),
            );
            for sx in [-1.0, 1.0] {
                fr.glow(b, head + vec3(0.08 * sx, 0.05, 0.19), 0.045, glow_c);
            }
            if style == GiantStyle::Crystal {
                for k in 0..3 {
                    let x = k as f32 - 1.0;
                    fr.cone_dir(
                        b,
                        head + vec3(x * 0.1, 0.2, -0.05),
                        vec3(x * 0.08, 0.3, -0.05),
                        0.05,
                        colors[2],
                    );
                }
            }
        }
    }
}

/// Where the hands ended up, and other measurements gear hangs off.
struct Hands {
    left: Vec3,
    right: Vec3,
    left_angle: f32,
    right_angle: f32,
    shoulder_y: f32,
    sw: f32,
    y: f32,
    chest_w: f32,
}

fn sword(b: &mut Batch, fr: &Frame, hand: Vec3, angle: f32, length: f32) {
    let guard = fr.limb(b, hand, angle, 0.025, 0.12, WOOD);
    fr.limb(b, guard, angle, 0.13, 0.04, GOLD);
    fr.limb(b, guard, angle, 0.04, length, STEEL);
}

fn shield(b: &mut Batch, fr: &Frame, hand: Vec3, face: Color, trim: Color) {
    fr.cube(
        b,
        hand + vec3(-0.07, 0.05, 0.13),
        vec3(0.04, 0.33, 0.27),
        face,
    );
    fr.cube(
        b,
        hand + vec3(-0.11, 0.05, 0.13),
        vec3(0.015, 0.34, 0.03),
        trim,
    );
    fr.cube(
        b,
        hand + vec3(-0.11, 0.05, 0.13),
        vec3(0.015, 0.03, 0.28),
        trim,
    );
}

fn gear_and_weapons(
    b: &mut Batch,
    fr: &Frame,
    outfit: Outfit,
    h: &Hands,
    look: &Look,
    pose: Pose,
    legs: Color,
) {
    let (rhand, lhand) = (h.right, h.left);
    let (wa, la) = (h.right_angle, h.left_angle);
    let y = h.y;
    match outfit {
        Outfit::Class(class) => match class {
            Class::Barbarian => {
                // A fur mantle and a great two-handed axe.
                for sx in [-1.0, 1.0] {
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx, h.shoulder_y + 0.05, -0.02),
                        vec3(0.17, 0.1, 0.17),
                        c(0.55, 0.45, 0.35),
                    );
                }
                fr.cube(
                    b,
                    vec3(0.0, 1.4 + y, 0.175),
                    vec3(0.03, 0.15, 0.01),
                    LEATHER,
                );
                let haft = fr.limb(b, rhand, wa, 0.035, 1.2, WOOD);
                fr.cube(
                    b,
                    haft + vec3(0.0, 0.05, 0.0),
                    vec3(0.03, 0.18, 0.14),
                    STEEL,
                );
            }
            Class::Fighter => {
                for sx in [-1.0, 1.0] {
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx, h.shoulder_y + 0.06, 0.0),
                        vec3(0.15, 0.08, 0.15),
                        STEEL,
                    );
                }
                fr.cube(
                    b,
                    vec3(0.0, 1.25 + y, 0.175),
                    vec3(0.14, 0.32, 0.01),
                    c(0.18, 0.3, 0.6),
                );
                sword(b, fr, rhand, wa, 0.85);
                shield(b, fr, lhand, c(0.18, 0.3, 0.6), STEEL);
            }
            Class::Paladin => {
                for sx in [-1.0, 1.0] {
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx, h.shoulder_y + 0.06, 0.0),
                        vec3(0.16, 0.09, 0.16),
                        c(0.88, 0.88, 0.92),
                    );
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx, h.shoulder_y + 0.04, 0.0),
                        vec3(0.17, 0.04, 0.17),
                        GOLD,
                    );
                }
                fr.cube(b, vec3(0.0, 1.3 + y, 0.175), vec3(0.04, 0.2, 0.01), GOLD);
                fr.cube(b, vec3(0.0, 1.38 + y, 0.18), vec3(0.12, 0.035, 0.01), GOLD);
                let head = fr.limb(b, rhand, wa, 0.03, 0.7, WOOD);
                fr.cube(b, head, vec3(0.1, 0.1, 0.16), STEEL);
                shield(b, fr, lhand, c(0.9, 0.88, 0.8), GOLD);
            }
            Class::Monk => {
                // A sash, and wrapped fists.
                let red = c(0.85, 0.15, 0.12);
                fr.cube(
                    b,
                    vec3(0.0, 1.07 + y, 0.0),
                    vec3(h.chest_w - 0.04, 0.05, 0.16),
                    red,
                );
                fr.tilted(
                    b,
                    vec3(0.12, 0.9 + y, 0.15),
                    vec3(0.04, 0.15, 0.01),
                    0.1,
                    red,
                );
                fr.sphere(b, rhand, 0.085, c(0.9, 0.88, 0.8));
                fr.sphere(b, lhand, 0.085, c(0.9, 0.88, 0.8));
            }
            Class::Rogue => {
                fr.cube(
                    b,
                    vec3(0.0, 1.36 + y, 0.0),
                    vec3(h.chest_w + 0.01, 0.025, 0.175),
                    LEATHER,
                );
                for (hand, angle) in [(rhand, wa), (lhand, la)] {
                    let hilt = fr.limb(b, hand, angle, 0.025, 0.1, c(0.2, 0.12, 0.08));
                    fr.limb(b, hilt, angle, 0.08, 0.03, STEEL);
                    fr.limb(b, hilt, angle, 0.03, 0.42, c(0.82, 0.84, 0.88));
                }
            }
            Class::Ranger => {
                // A longbow in the left hand and a quiver on the back.
                let top = lhand + vec3(-0.02, 0.75, 0.05);
                let bottom = lhand + vec3(-0.02, -0.75, 0.05);
                let bend = lhand + vec3(-0.02, 0.0, 0.22);
                fr.beam(b, top, bend, 0.025, WOOD);
                fr.beam(b, bend, bottom, 0.025, WOOD);
                fr.beam(b, top, bottom, 0.006, c(0.9, 0.9, 0.85));
                fr.tilted(
                    b,
                    vec3(0.12, 1.35 + y, -0.22),
                    vec3(0.07, 0.25, 0.06),
                    -0.35,
                    LEATHER,
                );
                for k in 0..3 {
                    let x = 0.08 + k as f32 * 0.04;
                    fr.tilted(
                        b,
                        vec3(x, 1.65 + y, -0.32),
                        vec3(0.012, 0.08, 0.012),
                        -0.35,
                        c(0.85, 0.2, 0.15),
                    );
                }
            }
            Class::Artificer => {
                // A backpack of pipes and a rifle.
                fr.cube(
                    b,
                    vec3(0.0, 1.32 + y, -0.27),
                    vec3(0.2, 0.22, 0.1),
                    c(0.45, 0.35, 0.22),
                );
                for sx in [-1.0, 1.0] {
                    fr.cylinder(
                        b,
                        vec3(0.12 * sx, 1.5 + y, -0.3),
                        0.3,
                        0.04,
                        c(0.7, 0.55, 0.25),
                    );
                }
                fr.glow(b, vec3(0.0, 1.35 + y, -0.38), 0.06, c(0.5, 0.85, 1.0));
                let stock = fr.limb(b, rhand, wa, 0.05, 0.25, WOOD);
                fr.limb(b, stock, wa, 0.03, 0.55, c(0.35, 0.35, 0.38));
            }
            Class::Bard => {
                // A lute held across the body.
                fr.ellipsoid(
                    b,
                    lhand + vec3(0.1, 0.0, 0.15),
                    vec3(0.16, 0.2, 0.06),
                    c(0.6, 0.38, 0.18),
                );
                fr.beam(
                    b,
                    lhand + vec3(0.1, 0.15, 0.15),
                    lhand + vec3(0.25, 0.6, 0.15),
                    0.025,
                    c(0.35, 0.22, 0.12),
                );
                fr.cube(b, vec3(0.0, 1.3 + y, 0.175), vec3(0.04, 0.2, 0.01), GOLD);
            }
            Class::Cleric => {
                fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.07, 0.3, 0.01), GOLD);
                fr.cube(b, vec3(0.0, 1.38 + y, 0.18), vec3(0.14, 0.035, 0.01), GOLD);
                let head = fr.limb(b, rhand, wa, 0.03, 0.6, WOOD);
                fr.sphere(b, head, 0.11, c(0.82, 0.82, 0.85));
            }
            Class::Druid => {
                fr.cube(
                    b,
                    rhand + vec3(0.0, 0.25, 0.0),
                    vec3(0.035, 0.95, 0.035),
                    c(0.45, 0.32, 0.2),
                );
                for k in 0..4 {
                    let a = k as f32 * 1.57;
                    fr.sphere(
                        b,
                        rhand + vec3(a.cos() * 0.08, 1.2, a.sin() * 0.08),
                        0.07,
                        c(0.35, 0.65, 0.25),
                    );
                }
                fr.glow(b, rhand + vec3(0.0, 1.25, 0.0), 0.04, c(0.8, 1.0, 0.6));
            }
            Class::Mage => {
                fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.04, 0.27, 0.01), GOLD);
                fr.cube(b, rhand + vec3(0.0, 0.3, 0.0), vec3(0.03, 0.95, 0.03), WOOD);
                fr.glow(b, rhand + vec3(0.0, 1.32, 0.0), 0.1, c(0.55, 0.85, 1.0));
            }
            Class::Sorcerer => {
                fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.04, 0.27, 0.01), GOLD);
                // Orbs of wild magic circling.
                for k in 0..3 {
                    let a = pose.time * 2.0 + k as f32 * 2.09;
                    let p = vec3(
                        a.cos() * 0.55,
                        1.5 + y + (a * 2.0).sin() * 0.1,
                        a.sin() * 0.55,
                    );
                    let col = [c(1.0, 0.4, 0.2), c(0.6, 0.4, 1.0), c(0.3, 0.9, 1.0)][k];
                    fr.glow(b, p, 0.07, col);
                }
            }
            Class::Warlock => {
                // A skull at the belt and fel fire in the hands.
                fr.cube(b, vec3(0.18, 1.0 + y, 0.13), vec3(0.06, 0.07, 0.06), BONE);
                let flame = Color::new(0.4, 1.0, 0.3, 0.9);
                fr.glow(
                    b,
                    rhand + vec3(0.0, 0.05, 0.1),
                    0.08 + (pose.time * 7.0).sin().abs() * 0.02,
                    flame,
                );
                fr.glow(b, lhand + vec3(0.0, 0.05, 0.1), 0.06, flame);
            }
        },
        Outfit::Merchant => {
            // An apron and a coin purse.
            fr.cube(
                b,
                vec3(0.0, 1.05 + y, 0.17),
                vec3(0.2, 0.3, 0.01),
                c(0.88, 0.85, 0.75),
            );
            fr.sphere(b, vec3(0.2, 1.0 + y, 0.12), 0.07, c(0.55, 0.4, 0.2));
        }
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Bandit | HumanoidStyle::Raider | HumanoidStyle::Trogg => {
                let hilt = fr.limb(b, rhand, wa, 0.025, 0.1, WOOD);
                if style == HumanoidStyle::Trogg || look.seed.is_multiple_of(2) {
                    fr.limb(b, hilt, wa, 0.07, 0.6, c(0.35, 0.24, 0.14));
                } else {
                    fr.limb(b, hilt, wa, 0.03, 0.5, STEEL);
                }
            }
            HumanoidStyle::Satyr => {
                let hilt = fr.limb(b, rhand, wa, 0.025, 0.1, WOOD);
                fr.limb(b, hilt, wa, 0.03, 0.55, STEEL);
                fr.limb(b, lhand, la, 0.025, 0.4, STEEL);
            }
            HumanoidStyle::Troll => {
                let haft = fr.limb(b, rhand, wa, 0.03, 0.9, WOOD);
                fr.cube(b, haft, vec3(0.03, 0.15, 0.12), c(0.75, 0.85, 0.95));
            }
            HumanoidStyle::Skeleton => {
                let hilt = fr.limb(b, rhand, wa, 0.025, 0.1, WOOD);
                fr.limb(b, hilt, wa, 0.035, 0.6, c(0.55, 0.42, 0.3));
                shield(b, fr, lhand, c(0.35, 0.3, 0.28), c(0.55, 0.42, 0.3));
            }
            HumanoidStyle::Mystic
            | HumanoidStyle::Trickster
            | HumanoidStyle::Necromancer
            | HumanoidStyle::Shaman
            | HumanoidStyle::TrollShaman
            | HumanoidStyle::TroggShaman => {
                let pulse = 0.08 + (pose.time * 3.0).sin().abs() * 0.03;
                let g = colors[2];
                fr.glow(b, rhand + vec3(0.0, 0.05, 0.12), pulse, g);
                if matches!(
                    style,
                    HumanoidStyle::Necromancer | HumanoidStyle::Shaman | HumanoidStyle::TrollShaman
                ) {
                    fr.cube(b, lhand + vec3(0.0, 0.3, 0.0), vec3(0.03, 0.95, 0.03), WOOD);
                    let top = if style == HumanoidStyle::Necromancer {
                        BONE
                    } else {
                        g
                    };
                    fr.sphere(b, lhand + vec3(0.0, 1.3, 0.0), 0.1, top);
                } else {
                    fr.glow(b, lhand + vec3(0.0, 0.05, 0.12), pulse * 0.7, g);
                }
            }
        },
        Outfit::Giant(style, colors) => {
            fr.cube(b, lhand, vec3(0.16, 0.16, 0.16), legs);
            fr.cube(b, rhand, vec3(0.16, 0.16, 0.16), legs);
            for sx in [-1.0, 1.0] {
                fr.cube(
                    b,
                    vec3(h.sw * sx, h.shoulder_y + 0.08, 0.0),
                    vec3(0.16, 0.12, 0.16),
                    dark(legs, 0.9),
                );
            }
            let accent = colors[2];
            match style {
                GiantStyle::Stone | GiantStyle::Sandstone => {
                    let moss = if style == GiantStyle::Stone {
                        c(0.32, 0.45, 0.2)
                    } else {
                        dark(colors[0], 0.8)
                    };
                    fr.cube(
                        b,
                        vec3(-0.15, h.shoulder_y + 0.21, -0.05),
                        vec3(0.12, 0.03, 0.1),
                        moss,
                    );
                    let rune = Color::new(
                        accent.r,
                        accent.g,
                        accent.b,
                        0.6 + 0.4 * (pose.time * 2.0).sin().abs(),
                    );
                    fr.glow(b, vec3(0.0, 1.4 + y, 0.18), 0.07, rune);
                    for (x, yy) in [(-0.15, 1.25), (0.15, 1.25), (0.0, 1.17)] {
                        b.lit(|b| {
                            fr.cube(b, vec3(x, yy + y, 0.172), vec3(0.03, 0.03, 0.005), rune)
                        });
                    }
                }
                GiantStyle::Treant => {
                    // Branches growing from the shoulders.
                    for sx in [-1.0, 1.0] {
                        let from = vec3(h.sw * sx, h.shoulder_y, 0.0);
                        let to = vec3(h.sw * sx * 1.8, h.shoulder_y + 0.5, -0.1);
                        fr.beam(b, from, to, 0.04, colors[0]);
                        fr.sphere(b, to + vec3(0.03 * sx, 0.1, 0.0), 0.15, accent);
                    }
                }
                GiantStyle::Crystal => {
                    b.lit(|b| {
                        for k in 0..5 {
                            let x = (k as f32 - 2.0) * 0.12;
                            fr.cone_dir(
                                b,
                                vec3(x, 1.45 + y, -0.15),
                                vec3(x * 0.5, 0.35, -0.2),
                                0.06,
                                accent,
                            );
                        }
                        for sx in [-1.0, 1.0] {
                            fr.cone_dir(
                                b,
                                vec3(h.sw * sx, h.shoulder_y + 0.15, 0.0),
                                vec3(0.1 * sx, 0.3, 0.0),
                                0.07,
                                accent,
                            );
                        }
                    });
                }
                GiantStyle::Yeti => {
                    // Shaggy fur.
                    fr.ellipsoid(
                        b,
                        vec3(0.0, 1.35 + y, 0.0),
                        vec3(0.36, 0.3, 0.24),
                        colors[0],
                    );
                    for sx in [-1.0, 1.0] {
                        fr.ellipsoid(
                            b,
                            vec3(h.sw * sx, h.shoulder_y + 0.05, 0.0),
                            vec3(0.2, 0.15, 0.2),
                            colors[0],
                        );
                    }
                }
                GiantStyle::Bone => {
                    for k in 0..4 {
                        fr.cone_dir(
                            b,
                            vec3(0.0, 1.55 - k as f32 * 0.12 + y, -0.1),
                            vec3(0.0, 0.05, -0.2),
                            0.04,
                            colors[0],
                        );
                    }
                    fr.glow(b, vec3(0.0, 1.3 + y, 0.05), 0.08, accent);
                }
            }
        }
    }
}

fn wolf(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], seed: u32, pose: Pose) {
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, 1.0, true)
    } else {
        Frame::upright(pos, yaw, 1.0)
    };
    let tint = (seed % 3) as f32 * 0.04;
    let fur = Color::new(colors[0].r + tint, colors[0].g + tint, colors[0].b, 1.0);
    let back = colors[1];
    let belly = mix(colors[0], c(0.85, 0.82, 0.78), 0.5);
    let moving = pose.moving && !pose.dead;
    let gait = if moving { pose.walk } else { 0.0 };
    let lunge = if pose.swing > 0.0 && pose.swing < 1.0 {
        (pose.swing * std::f32::consts::PI).sin() * 0.3
    } else {
        0.0
    };
    let y = 0.78
        + if moving {
            (gait * 2.0).sin().abs() * 0.04
        } else {
            0.0
        };
    fr.cube(b, vec3(0.0, y, 0.22), vec3(0.23, 0.24, 0.32), fur);
    fr.cube(b, vec3(0.0, y + 0.02, -0.32), vec3(0.19, 0.2, 0.28), fur);
    fr.cube(b, vec3(0.0, y + 0.23, -0.05), vec3(0.17, 0.05, 0.55), back);
    fr.cube(b, vec3(0.0, y - 0.2, 0.15), vec3(0.17, 0.06, 0.3), belly);
    fr.cube(b, vec3(0.0, y + 0.08, 0.5), vec3(0.25, 0.25, 0.1), back);
    for (x, z, phase) in [
        (-1.0, 1.0, 0.0),
        (1.0, 1.0, 3.1),
        (-1.0, -1.0, 3.1),
        (1.0, -1.0, 0.0),
    ] {
        let swing = if moving {
            (gait + phase).sin() * 0.55
        } else {
            0.0
        };
        let bend = if moving {
            (gait + phase + 1.0).sin().max(0.0) * 0.6
        } else {
            0.0
        };
        let hip = vec3(x * 0.15, y - 0.12, z * 0.4);
        let knee = fr.limb(b, hip, swing, 0.07, 0.36, fur);
        let ankle = fr.limb(b, knee, swing - bend * z, 0.055, 0.32, back);
        fr.cube(
            b,
            ankle + vec3(0.0, 0.0, 0.04),
            vec3(0.06, 0.035, 0.08),
            back,
        );
    }
    let head = vec3(0.0, y + 0.22, 0.72 + lunge);
    fr.cube(b, head, vec3(0.16, 0.15, 0.17), fur);
    fr.cube(
        b,
        head + vec3(0.0, -0.05, 0.24),
        vec3(0.08, 0.07, 0.13),
        fur,
    );
    fr.cube(
        b,
        head + vec3(0.0, -0.1, 0.22),
        vec3(0.07, 0.025, 0.12),
        belly,
    );
    fr.sphere(b, head + vec3(0.0, -0.01, 0.38), 0.04, c(0.08, 0.07, 0.07));
    for sx in [-1.0, 1.0] {
        fr.glow(b, head + vec3(0.08 * sx, 0.05, 0.16), 0.032, colors[2]);
        fr.cone_dir(
            b,
            head + vec3(0.09 * sx, 0.12, -0.04),
            vec3(0.03 * sx, 0.17, -0.03),
            0.06,
            back,
        );
        fr.cone_dir(
            b,
            head + vec3(0.05 * sx, -0.11, 0.33),
            vec3(0.0, -0.06, 0.0),
            0.015,
            c(0.95, 0.94, 0.88),
        );
    }
    let wag = (pose.time * 4.0).sin() * 0.2;
    let t1 = fr.limb(b, vec3(0.0, y + 0.12, -0.58), 2.2 + wag, 0.07, 0.25, fur);
    let t2 = fr.limb(b, t1, 2.6 + wag, 0.09, 0.25, back);
    fr.sphere(b, t2, 0.07, belly);
}

fn boar(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], seed: u32, pose: Pose) {
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, 1.0, true)
    } else {
        Frame::upright(pos, yaw, 1.0)
    };
    let tint = (seed % 3) as f32 * 0.03;
    let hide = Color::new(colors[0].r + tint, colors[0].g, colors[0].b, 1.0);
    let dark_hide = colors[1];
    let snout = mix(hide, c(0.85, 0.55, 0.5), 0.6);
    let tusk = colors[2];
    let moving = pose.moving && !pose.dead;
    let gait = if moving { pose.walk * 1.3 } else { 0.0 };
    let lunge = if pose.swing > 0.0 && pose.swing < 1.0 {
        (pose.swing * std::f32::consts::PI).sin() * 0.3
    } else {
        0.0
    };
    let y = 0.68
        + if moving {
            (gait * 2.0).sin().abs() * 0.03
        } else {
            0.0
        };
    // The body is built along the boar's own axes, so it turns with the boar.
    fr.ellipsoid(b, vec3(0.0, y, 0.0), vec3(0.34, 0.33, 0.62), hide);
    fr.ellipsoid(b, vec3(0.0, y + 0.05, 0.38), vec3(0.32, 0.33, 0.3), hide);
    for i in 0..6 {
        let z = 0.45 - i as f32 * 0.17;
        let top = y + 0.3 - (i as f32 - 2.0).abs() * 0.02;
        fr.cone_dir(
            b,
            vec3(0.0, top, z),
            vec3(0.0, 0.14, -0.06),
            0.06,
            dark_hide,
        );
    }
    for (x, z, phase) in [
        (-1.0, 1.0, 0.0),
        (1.0, 1.0, 3.1),
        (-1.0, -1.0, 3.1),
        (1.0, -1.0, 0.0),
    ] {
        let swing = if moving {
            (gait + phase).sin() * 0.5
        } else {
            0.0
        };
        let hip = vec3(x * 0.2, y - 0.18, z * 0.38);
        let ankle = fr.limb(b, hip, swing, 0.08, 0.36, hide);
        fr.cube(b, ankle, vec3(0.07, 0.04, 0.07), c(0.15, 0.12, 0.1));
    }
    let head = vec3(0.0, y - 0.02, 0.72 + lunge);
    fr.cube(b, head, vec3(0.19, 0.18, 0.2), hide);
    fr.cylinder_dir(
        b,
        head + vec3(0.0, -0.04, 0.2),
        vec3(0.0, 0.0, 0.1),
        0.09,
        snout,
    );
    for sx in [-1.0, 1.0] {
        fr.sphere(
            b,
            head + vec3(0.035 * sx, -0.04, 0.3),
            0.02,
            c(0.2, 0.1, 0.1),
        );
        fr.sphere(
            b,
            head + vec3(0.12 * sx, 0.07, 0.17),
            0.03,
            c(0.08, 0.06, 0.05),
        );
        fr.cone_dir(
            b,
            head + vec3(0.13 * sx, 0.15, -0.05),
            vec3(0.06 * sx, 0.12, -0.05),
            0.06,
            dark_hide,
        );
        fr.cone_dir(
            b,
            head + vec3(0.12 * sx, -0.12, 0.2),
            vec3(0.05 * sx, 0.1, 0.1),
            0.03,
            tusk,
        );
    }
    let t = fr.limb(b, vec3(0.0, y + 0.15, -0.6), 2.5, 0.025, 0.12, dark_hide);
    fr.limb(
        b,
        t,
        1.3 + (pose.time * 5.0).sin() * 0.3,
        0.025,
        0.1,
        dark_hide,
    );
}

fn spider(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], pose: Pose) {
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, 1.0, true)
    } else {
        Frame::upright(pos, yaw, 1.0)
    };
    let moving = pose.moving && !pose.dead;
    let lunge = if pose.swing > 0.0 && pose.swing < 1.0 {
        (pose.swing * std::f32::consts::PI).sin() * 0.25
    } else {
        0.0
    };
    let y = 0.6;
    fr.ellipsoid(
        b,
        vec3(0.0, y, 0.15 + lunge),
        vec3(0.3, 0.22, 0.32),
        colors[0],
    );
    fr.ellipsoid(
        b,
        vec3(0.0, y + 0.15, -0.5),
        vec3(0.45, 0.38, 0.55),
        colors[1],
    );
    // Markings on the abdomen.
    fr.ellipsoid(
        b,
        vec3(0.0, y + 0.5, -0.5),
        vec3(0.12, 0.04, 0.2),
        colors[2],
    );
    for k in 0..4 {
        for sx in [-1.0f32, 1.0] {
            let phase = k as f32 * 1.6 + if sx > 0.0 { 3.1 } else { 0.0 };
            let lift = if moving {
                (pose.walk * 1.5 + phase).sin().max(0.0) * 0.2
            } else {
                0.0
            };
            let z = 0.35 - k as f32 * 0.22;
            let hip = vec3(0.22 * sx, y, z + lunge);
            let knee = vec3(0.75 * sx, y + 0.45 + lift, z * 1.4 + 0.05);
            let foot = vec3(1.05 * sx, 0.0, z * 1.7 + 0.1);
            fr.beam(b, hip, knee, 0.045, colors[0]);
            fr.beam(b, knee, foot, 0.035, colors[1]);
        }
    }
    for sx in [-1.0, 1.0] {
        fr.glow(b, vec3(0.08 * sx, y + 0.12, 0.45 + lunge), 0.04, colors[2]);
        fr.glow(b, vec3(0.16 * sx, y + 0.08, 0.4 + lunge), 0.03, colors[2]);
        fr.cone_dir(
            b,
            vec3(0.07 * sx, y - 0.05, 0.45 + lunge),
            vec3(0.0, -0.15, 0.08),
            0.03,
            c(0.1, 0.08, 0.08),
        );
    }
}

fn scorpion(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], pose: Pose) {
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, 1.0, true)
    } else {
        Frame::upright(pos, yaw, 1.0)
    };
    let moving = pose.moving && !pose.dead;
    let strike = if pose.swing > 0.0 && pose.swing < 1.0 {
        (pose.swing * std::f32::consts::PI).sin()
    } else {
        0.0
    };
    let y = 0.45;
    fr.ellipsoid(b, vec3(0.0, y, 0.0), vec3(0.35, 0.16, 0.55), colors[0]);
    for k in 0..3 {
        let kf = k as f32;
        fr.cube(
            b,
            vec3(0.0, y + 0.12, 0.3 - kf * 0.3),
            vec3(0.3 - kf * 0.03, 0.04, 0.1),
            colors[1],
        );
    }
    for k in 0..3 {
        for sx in [-1.0f32, 1.0] {
            let phase = k as f32 * 1.9 + if sx > 0.0 { 3.1 } else { 0.0 };
            let lift = if moving {
                (pose.walk * 1.5 + phase).sin().max(0.0) * 0.15
            } else {
                0.0
            };
            let z = 0.25 - k as f32 * 0.25;
            let knee = vec3(0.65 * sx, y + 0.2 + lift, z);
            fr.beam(b, vec3(0.3 * sx, y, z), knee, 0.035, colors[0]);
            fr.beam(b, knee, vec3(0.85 * sx, 0.0, z - 0.05), 0.03, colors[1]);
        }
    }
    // Pincers.
    for sx in [-1.0f32, 1.0] {
        let elbow = vec3(0.4 * sx, y + 0.1, 0.65);
        let claw = vec3(0.3 * sx, y + 0.1, 1.0 + strike * 0.2);
        fr.beam(b, vec3(0.25 * sx, y, 0.45), elbow, 0.05, colors[0]);
        fr.beam(b, elbow, claw, 0.05, colors[0]);
        fr.ellipsoid(
            b,
            claw + vec3(0.0, 0.0, 0.1),
            vec3(0.1, 0.07, 0.16),
            colors[1],
        );
        fr.cone_dir(
            b,
            claw + vec3(0.04 * sx, 0.0, 0.22),
            vec3(-0.04 * sx, 0.0, 0.15),
            0.04,
            colors[1],
        );
    }
    // A tail curling up over the back, with a stinger.
    let mut p = vec3(0.0, y + 0.05, -0.5);
    for k in 0..6 {
        let a = 0.2 + k as f32 * 0.42 + strike * 0.3;
        let next = p + vec3(0.0, a.sin() * 0.22, -a.cos() * 0.22);
        let col = if k % 2 == 0 { colors[0] } else { colors[1] };
        fr.sphere(b, next, 0.11 - k as f32 * 0.008, col);
        p = next;
    }
    fr.cone_dir(b, p, vec3(0.0, -0.05, 0.25), 0.05, colors[2]);
    for sx in [-1.0, 1.0] {
        fr.sphere(b, vec3(0.08 * sx, y + 0.12, 0.5), 0.03, c(0.05, 0.05, 0.05));
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
    pub fn draw_effects(&self, zone: Zone, b: &mut Batch, time: f32) {
        let z = self.get(zone);
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

/// The sky, drawn in screen space before the 3D pass.
pub fn draw_sky(cam: &Camera3D, zone: Zone, project: impl Fn(Vec3) -> Option<Vec2>) {
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
    for f in props::FIELDS {
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
    terrain(&mut b, zone);
    ground_cover(&mut b, zone);
    for p in props::props(zone) {
        draw_prop(&mut b, zone, &p, &mut chimneys, &mut lamps, &mut fires);
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
