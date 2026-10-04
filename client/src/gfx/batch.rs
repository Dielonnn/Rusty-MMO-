//! The triangle batcher every 3D thing in the game is drawn through.
//!
//! Vertices carry their surface color and normal; lighting happens on the
//! GPU (see `shader.rs`). A vertex's `normal.w` says how to light it: 1 for
//! lit surfaces, 0 for things that glow at full brightness.

use macroquad::models::{Mesh, Vertex, draw_mesh};
use macroquad::prelude::*;
use shared::world::{forward, terrain_height};

const MAX_VERTICES: usize = 60_000;
const MAX_INDICES: usize = 180_000;

/// How far box corners bend their normals outward, so a box's faces shade
/// in soft gradients like a rounded shape instead of three flat tones.
const BOX_ROUNDING: f32 = 0.35;

/// Two axes perpendicular to `axis`, the first as close to `reference` as possible.
pub fn basis(axis: Vec3, reference: Vec3) -> (Vec3, Vec3) {
    let up = axis.normalize_or_zero();
    let mut u = reference - up * reference.dot(up);
    if u.length_squared() < 1e-6 {
        let helper = if up.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        u = up.cross(helper);
    }
    let u = u.normalize();
    (u, up.cross(u))
}

fn bytes(color: Color) -> [u8; 4] {
    [
        (color.r * 255.0).clamp(0.0, 255.0) as u8,
        (color.g * 255.0).clamp(0.0, 255.0) as u8,
        (color.b * 255.0).clamp(0.0, 255.0) as u8,
        (color.a * 255.0).clamp(0.0, 255.0) as u8,
    ]
}

/// Collects triangles and draws them in as few calls as possible.
/// In recording mode it keeps the meshes instead, for static scenery.
pub struct Batch {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    recorded: Option<Vec<Mesh>>,
    /// The texture everything is drawn with (`None`: plain white).
    texture: Option<Texture2D>,
    /// Skip lighting: everything drawn is emissive.
    pub glowing: bool,
}

impl Default for Batch {
    fn default() -> Self {
        Self::new()
    }
}

impl Batch {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            recorded: None,
            texture: None,
            glowing: false,
        }
    }

    pub fn recording() -> Self {
        Self {
            recorded: Some(Vec::new()),
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
            texture: self.texture.clone(),
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

    /// Draws what follows with `texture` (`None` for plain colors). Texture
    /// coordinates come from `quad_uv` and `vertex_uv`; everything else
    /// samples the texture's corner.
    pub fn set_texture(&mut self, texture: Option<Texture2D>) {
        if self.texture != texture {
            self.flush();
            self.texture = texture;
        }
    }

    /// Draws something with a texture, then goes back to the one before.
    pub fn textured<R>(&mut self, texture: &Texture2D, f: impl FnOnce(&mut Self) -> R) -> R {
        let was = self.texture.clone();
        self.set_texture(Some(texture.clone()));
        let r = f(self);
        self.set_texture(was);
        r
    }

    /// Adds a vertex lit by the scene's light (or glowing, if `glowing` is
    /// set), returning its index. Use with `index` to build custom shapes.
    pub fn vertex_uv(&mut self, position: Vec3, uv: Vec2, normal: Vec3, color: Color) -> u16 {
        let i = self.vertices.len() as u16;
        let n = normal.try_normalize().unwrap_or(Vec3::Y);
        let lit = if self.glowing { 0.0 } else { 1.0 };
        self.vertices.push(Vertex {
            position,
            uv,
            color: bytes(color),
            normal: n.extend(lit),
        });
        i
    }

    fn vertex(&mut self, position: Vec3, normal: Vec3, color: Color) {
        self.vertex_uv(position, Vec2::ZERO, normal, color);
    }

    /// A vertex that always glows, whatever `glowing` says.
    fn glow_vertex(&mut self, position: Vec3, color: Color) {
        self.vertices.push(Vertex {
            position,
            uv: Vec2::ZERO,
            color: bytes(color),
            normal: Vec4::ZERO,
        });
    }

    /// Makes room for a custom shape of `v` vertices and `i` indices,
    /// returning the index its first vertex will get.
    pub fn begin_shape(&mut self, v: usize, i: usize) -> u16 {
        self.reserve(v, i)
    }

    /// Adds triangle indices for a custom shape.
    pub fn index(&mut self, indices: &[u16]) {
        self.indices.extend_from_slice(indices);
    }

    pub fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, color: Color) {
        self.quad_normals(corners, [normal; 4], color);
    }

    /// A quad with its own normal at each corner, for smooth shading.
    pub fn quad_normals(&mut self, corners: [Vec3; 4], normals: [Vec3; 4], color: Color) {
        let base = self.reserve(4, 6);
        for (p, n) in corners.into_iter().zip(normals) {
            self.vertex(p, n, color);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A quad showing part of the current texture: `uvs` for each corner.
    pub fn quad_uv(&mut self, corners: [Vec3; 4], uvs: [Vec2; 4], normal: Vec3, color: Color) {
        let base = self.reserve(4, 6);
        for (p, uv) in corners.into_iter().zip(uvs) {
            self.vertex_uv(p, uv, normal, color);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) {
        let mut n = (b - a).cross(c - a);
        if n.y < 0.0 {
            n = -n;
        }
        let base = self.reserve(3, 3);
        self.vertex(a, n, color);
        self.vertex(b, n, color);
        self.vertex(c, n, color);
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    /// A box given its center and three half-axis vectors. Its corners'
    /// normals lean outward, so it shades like a slightly rounded block.
    pub fn cuboid(&mut self, center: Vec3, axes: [Vec3; 3], color: Color) {
        let unit = axes.map(|a| a.normalize_or_zero());
        for i in 0..3 {
            let (j, k) = ((i + 1) % 3, (i + 2) % 3);
            let (a, b) = (axes[j], axes[k]);
            let (ua, ub) = (unit[j] * BOX_ROUNDING, unit[k] * BOX_ROUNDING);
            for sign in [-1.0, 1.0] {
                let n = unit[i] * sign;
                let fc = center + axes[i] * sign;
                self.quad_normals(
                    [fc - a - b, fc + a - b, fc + a + b, fc - a + b],
                    [n - ua - ub, n + ua - ub, n + ua + ub, n - ua + ub],
                    color,
                );
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
                self.vertex(p, n, color);
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
    /// Its facets line up with `reference`, so it turns with its model. Its
    /// sides are smooth-shaded, so it reads as round.
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
        let slope = up * (radius - top_radius) / axis.length().max(0.01);
        for i in 0..sides {
            let a0 = std::f32::consts::TAU * i as f32 / sides as f32;
            let a1 = std::f32::consts::TAU * (i + 1) as f32 / sides as f32;
            let d0 = u * a0.cos() + v * a0.sin();
            let d1 = u * a1.cos() + v * a1.sin();
            let (n0, n1) = (d0 + slope, d1 + slope);
            let b0 = base_center + d0 * radius;
            let b1 = base_center + d1 * radius;
            if top_radius > 0.0 {
                self.quad_normals(
                    [b0, b1, top + d1 * top_radius, top + d0 * top_radius],
                    [n0, n1, n1, n0],
                    color,
                );
            } else {
                let base = self.reserve(3, 3);
                self.vertex(b0, n0, color);
                self.vertex(b1, n1, color);
                self.vertex(top, (d0 + d1).normalize_or_zero() + slope, color);
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
        let base = self.reserve(sides + 1, sides * 3);
        self.vertex(center, normal, color);
        for i in 0..sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            self.vertex(center + (u * a.cos() + v * a.sin()) * radius, normal, color);
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
        let base = self.reserve(sides * 2, sides * 6);
        for i in 0..sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            let d = vec3(a.cos(), 0.0, a.sin());
            for r in [radius - width * 0.5, radius + width * 0.5] {
                let p = center + d * r;
                self.glow_vertex(
                    vec3(
                        p.x,
                        terrain_height(p.x, p.z).max(center.y - 0.5) + 0.08,
                        p.z,
                    ),
                    color,
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

    /// A soft dark patch on the ground under something standing at `foot`,
    /// so it sits on the ground instead of floating over it. It fades as
    /// `foot` rises above the ground. Draw it after the thing itself, so
    /// it doesn't hide the bottom of the feet.
    pub fn blob_shadow(&mut self, foot: Vec3, radius: f32, strength: f32) {
        const SIDES: usize = 16;
        let ground = terrain_height(foot.x, foot.z);
        let lift = (foot.y - ground).max(0.0);
        let strength = strength * (1.0 - lift / 4.0).clamp(0.0, 1.0);
        if strength <= 0.01 || radius <= 0.0 {
            return;
        }
        let radius = radius * (1.0 + lift * 0.15);
        let at = |p: Vec3| vec3(p.x, terrain_height(p.x, p.z) + 0.07, p.z);
        let dark = |a: f32| Color::new(0.0, 0.0, 0.0, a);
        let base = self.reserve(SIDES * 2 + 1, SIDES * 9);
        self.glow_vertex(at(foot), dark(strength));
        // An inner ring at full darkness and an outer one fading to nothing.
        for i in 0..SIDES {
            let a = std::f32::consts::TAU * i as f32 / SIDES as f32;
            let d = vec3(a.cos(), 0.0, a.sin()) * radius;
            self.glow_vertex(at(foot + d * 0.55), dark(strength * 0.8));
            self.glow_vertex(at(foot + d), dark(0.0));
        }
        for i in 0..SIDES as u16 {
            let (a, b) = (base + 1 + i * 2, base + 1 + ((i + 1) % SIDES as u16) * 2);
            self.indices
                .extend_from_slice(&[base, a, b, a, a + 1, b, b, a + 1, b + 1]);
        }
    }

    /// A ring floating in the air, facing up, unshaded.
    pub fn air_ring(&mut self, center: Vec3, radius: f32, width: f32, color: Color) {
        let sides = 32;
        let base = self.reserve(sides * 2, sides * 6);
        for i in 0..sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            let d = vec3(a.cos(), 0.0, a.sin());
            self.glow_vertex(center + d * (radius - width * 0.5), color);
            self.glow_vertex(center + d * (radius + width * 0.5), color);
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
    pub fn lit<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let was = self.glowing;
        self.glowing = true;
        let r = f(self);
        self.glowing = was;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertices_say_whether_they_are_lit() {
        let mut b = Batch::recording();
        b.sphere(Vec3::ZERO, 1.0, WHITE);
        b.glow_sphere(Vec3::X * 3.0, 1.0, WHITE);
        let meshes = b.finish();
        let v = &meshes[0].vertices;
        let half = v.len() / 2;
        assert!(v[..half].iter().all(|v| v.normal.w == 1.0));
        assert!(v[half..].iter().all(|v| v.normal.w == 0.0));
        // Sphere normals point away from the center.
        for v in &v[..half] {
            let n = v.normal.truncate();
            assert!((n.length() - 1.0).abs() < 1e-3);
            assert!(n.dot(v.position) > 0.0 || v.position.length() < 1e-3);
        }
    }

    #[test]
    fn box_normals_point_out_of_their_face() {
        let mut b = Batch::recording();
        b.cuboid(Vec3::ZERO, [Vec3::X, Vec3::Y * 2.0, Vec3::Z * 0.5], WHITE);
        let meshes = b.finish();
        for v in &meshes[0].vertices {
            let n = v.normal.truncate();
            // The face's own direction still dominates the rounding.
            let face = v.position / vec3(1.0, 2.0, 0.5);
            assert!(n.dot(face) > 0.5, "{n} at {}", v.position);
        }
    }

    #[test]
    fn shadows_fade_when_high_above_the_ground() {
        let ground = terrain_height(10.0, 10.0);
        let mut b = Batch::recording();
        b.blob_shadow(vec3(10.0, ground, 10.0), 1.0, 0.5);
        assert!(!b.finish().is_empty());
        let mut b = Batch::recording();
        b.blob_shadow(vec3(10.0, ground + 10.0, 10.0), 1.0, 0.5);
        assert!(b.finish().is_empty());
    }
}
