//! 3D drawing: a triangle batcher with simple directional shading, the static
//! scenery (terrain, trees, town, camps) and the character models.

use macroquad::models::{Mesh, Vertex, draw_mesh};
use macroquad::prelude::*;
use shared::data::{Class, MobKind};
use shared::protocol::EntityKind;
use shared::world::*;

const MAX_VERTICES: usize = 60_000;
const MAX_INDICES: usize = 180_000;

fn light_dir() -> Vec3 {
    vec3(0.35, 0.85, 0.4).normalize()
}

/// Darkens a color for a surface facing `normal`.
fn shade(color: Color, normal: Vec3) -> [u8; 4] {
    let d = normal.normalize_or_zero().dot(light_dir()).max(0.0);
    let s = 0.5 + 0.5 * d;
    [
        (color.r * s * 255.0).clamp(0.0, 255.0) as u8,
        (color.g * s * 255.0).clamp(0.0, 255.0) as u8,
        (color.b * s * 255.0).clamp(0.0, 255.0) as u8,
        (color.a * 255.0).clamp(0.0, 255.0) as u8,
    ]
}

fn flat(color: Color) -> [u8; 4] {
    shade(color, light_dir())
}

/// Collects triangles and draws them in as few calls as possible.
/// In recording mode it keeps the meshes instead, for static scenery.
pub struct Batch {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    recorded: Option<Vec<Mesh>>,
}

impl Batch {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            recorded: None,
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

    fn vertex(&mut self, position: Vec3, color: [u8; 4]) {
        self.vertices.push(Vertex {
            position,
            uv: Vec2::ZERO,
            color,
            normal: Vec4::ZERO,
        });
    }

    pub fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, color: Color) {
        let c = shade(color, normal);
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
        let col = shade(color, n);
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

    pub fn sphere(&mut self, center: Vec3, radius: f32, color: Color) {
        const SLICES: usize = 10;
        const STACKS: usize = 7;
        let base = self.reserve((SLICES + 1) * (STACKS + 1), SLICES * STACKS * 6);
        for j in 0..=STACKS {
            let phi = std::f32::consts::PI * j as f32 / STACKS as f32;
            for i in 0..=SLICES {
                let theta = std::f32::consts::TAU * i as f32 / SLICES as f32;
                let n = vec3(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
                self.vertex(center + n * radius, shade(color, n));
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

    /// A cone (or with `top_radius` > 0, a tapered cylinder) along `axis`.
    pub fn cone(
        &mut self,
        base_center: Vec3,
        axis: Vec3,
        radius: f32,
        top_radius: f32,
        sides: usize,
        color: Color,
    ) {
        let up = axis.normalize_or_zero();
        let helper = if up.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let u = up.cross(helper).normalize();
        let v = up.cross(u);
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
                let c = shade(color, n);
                let base = self.reserve(3, 3);
                self.vertex(b0, c);
                self.vertex(b1, c);
                self.vertex(top, c);
                self.indices.extend_from_slice(&[base, base + 1, base + 2]);
            }
        }
        if top_radius > 0.0 {
            self.disc(top, up, top_radius, sides, color);
        }
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

    fn disc(&mut self, center: Vec3, normal: Vec3, radius: f32, sides: usize, color: Color) {
        let helper = if normal.y.abs() > 0.9 {
            Vec3::X
        } else {
            Vec3::Y
        };
        let u = normal.cross(helper).normalize();
        let v = normal.cross(u);
        let c = shade(color, normal);
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

    /// A flat ring lying on the terrain, unshaded.
    pub fn ground_ring(&mut self, center: Vec3, radius: f32, width: f32, color: Color) {
        let sides = 32;
        let c = flat(color);
        let c = [
            color_u8(color.r),
            color_u8(color.g),
            color_u8(color.b),
            c[3],
        ];
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
        let c = [
            color_u8(color.r),
            color_u8(color.g),
            color_u8(color.b),
            color_u8(color.a),
        ];
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
}

fn color_u8(v: f32) -> u8 {
    (v * 255.0).clamp(0.0, 255.0) as u8
}

/// A local coordinate frame for building a model out of parts.
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

    pub fn sphere(&self, b: &mut Batch, center: Vec3, radius: f32, color: Color) {
        b.sphere(self.p(center), radius * self.scale, color);
    }

    pub fn cone(&self, b: &mut Batch, base: Vec3, height: f32, radius: f32, color: Color) {
        b.cone(
            self.p(base),
            self.u * height * self.scale,
            radius * self.scale,
            0.0,
            8,
            color,
        );
    }

    pub fn cylinder(&self, b: &mut Batch, base: Vec3, height: f32, radius: f32, color: Color) {
        b.cylinder(
            self.p(base),
            self.u * height * self.scale,
            radius * self.scale,
            8,
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

/// How tall something is, for nameplates and picking.
pub fn model_height(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::Player(_) => 2.1,
        EntityKind::Mob { kind, .. } => match kind {
            MobKind::Wolf => 1.3,
            MobKind::Boar => 1.2,
            MobKind::Bandit | MobKind::BanditMystic => 2.1,
            MobKind::Golem => 2.1 * 2.2,
        },
    }
}

/// Radius of the selection circle.
pub fn model_radius(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::Mob {
            kind: MobKind::Golem,
            ..
        } => 2.2,
        EntityKind::Mob {
            kind: MobKind::Wolf | MobKind::Boar,
            ..
        } => 1.2,
        _ => 0.9,
    }
}

pub fn draw_model(b: &mut Batch, kind: EntityKind, pos: Vec3, yaw: f32, pose: Pose) {
    match kind {
        EntityKind::Player(class) => {
            let gear = match class {
                Class::Warrior => Gear::Warrior,
                Class::Mage => Gear::Mage,
                Class::Cleric => Gear::Cleric,
            };
            humanoid(b, pos, yaw, 1.0, gear, pose);
        }
        EntityKind::Mob { kind, .. } => match kind {
            MobKind::Wolf => quadruped(b, pos, yaw, false, pose),
            MobKind::Boar => quadruped(b, pos, yaw, true, pose),
            MobKind::Bandit => humanoid(b, pos, yaw, 1.0, Gear::Bandit, pose),
            MobKind::BanditMystic => humanoid(b, pos, yaw, 1.0, Gear::Mystic, pose),
            MobKind::Golem => humanoid(b, pos, yaw, 2.2, Gear::Golem, pose),
        },
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Gear {
    Warrior,
    Mage,
    Cleric,
    Bandit,
    Mystic,
    Golem,
}

fn humanoid(b: &mut Batch, pos: Vec3, yaw: f32, scale: f32, gear: Gear, pose: Pose) {
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, scale, false)
    } else {
        Frame::upright(pos, yaw, scale)
    };
    let skin = Color::new(0.93, 0.76, 0.6, 1.0);
    let steel = Color::new(0.75, 0.77, 0.8, 1.0);
    let wood = Color::new(0.45, 0.3, 0.17, 1.0);
    let (torso, legs, arms, head_col) = match gear {
        Gear::Warrior => (
            Color::new(0.62, 0.18, 0.15, 1.0),
            Color::new(0.35, 0.3, 0.28, 1.0),
            steel,
            skin,
        ),
        Gear::Mage => (
            Color::new(0.35, 0.25, 0.75, 1.0),
            Color::new(0.3, 0.22, 0.65, 1.0),
            Color::new(0.35, 0.25, 0.75, 1.0),
            skin,
        ),
        Gear::Cleric => (
            Color::new(0.95, 0.93, 0.86, 1.0),
            Color::new(0.9, 0.88, 0.8, 1.0),
            Color::new(0.95, 0.93, 0.86, 1.0),
            skin,
        ),
        Gear::Bandit => (
            Color::new(0.45, 0.32, 0.2, 1.0),
            Color::new(0.25, 0.22, 0.2, 1.0),
            Color::new(0.45, 0.32, 0.2, 1.0),
            skin,
        ),
        Gear::Mystic => (
            Color::new(0.3, 0.12, 0.35, 1.0),
            Color::new(0.25, 0.1, 0.3, 1.0),
            Color::new(0.3, 0.12, 0.35, 1.0),
            skin,
        ),
        Gear::Golem => {
            let stone = Color::new(0.5, 0.5, 0.47, 1.0);
            (
                stone,
                Color::new(0.42, 0.42, 0.4, 1.0),
                Color::new(0.46, 0.46, 0.44, 1.0),
                stone,
            )
        }
    };
    let robed = matches!(gear, Gear::Mage | Gear::Cleric | Gear::Mystic);

    let stride = if pose.moving && !pose.dead {
        pose.walk.sin() * 0.7
    } else {
        0.0
    };
    // Legs.
    for (side, a) in [(-1.0, stride), (1.0, -stride)] {
        fr.limb(b, vec3(0.14 * side, 0.9, 0.0), a, 0.1, 0.9, legs);
    }
    if robed {
        fr.cube(b, vec3(0.0, 0.62, 0.0), vec3(0.28, 0.3, 0.2), legs);
    }
    // Body and head.
    fr.cube(b, vec3(0.0, 1.27, 0.0), vec3(0.28, 0.37, 0.16), torso);
    if gear == Gear::Golem {
        fr.cube(b, vec3(0.0, 1.85, 0.02), vec3(0.2, 0.2, 0.2), head_col);
        let glow = Color::new(0.4, 0.95, 1.0, 1.0);
        fr.sphere(b, vec3(-0.08, 1.88, 0.2), 0.05, glow);
        fr.sphere(b, vec3(0.08, 1.88, 0.2), 0.05, glow);
        fr.sphere(b, vec3(-0.38, 1.62, 0.0), 0.2, legs);
        fr.sphere(b, vec3(0.38, 1.62, 0.0), 0.2, legs);
    } else {
        fr.sphere(b, vec3(0.0, 1.84, 0.0), 0.2, head_col);
        let eye = Color::new(0.1, 0.1, 0.12, 1.0);
        fr.sphere(b, vec3(-0.07, 1.87, 0.18), 0.03, eye);
        fr.sphere(b, vec3(0.07, 1.87, 0.18), 0.03, eye);
    }
    fr.cube(
        b,
        vec3(0.0, 0.9, 0.0),
        vec3(0.29, 0.05, 0.17),
        Color::new(0.25, 0.18, 0.1, 1.0),
    );

    // Arms: swing while walking, raise while casting, chop when attacking.
    let mut left = -stride * 0.8;
    let mut right = stride * 0.8;
    if pose.casting {
        let wobble = (pose.time * 6.0).sin() * 0.1;
        left = 1.2 + wobble;
        right = 1.2 - wobble;
    }
    if pose.swing > 0.0 && pose.swing < 1.0 {
        let t = pose.swing;
        right = if t < 0.35 {
            2.6 * (t / 0.35)
        } else {
            2.6 * (1.0 - (t - 0.35) / 0.65) + 0.3
        };
    }
    let lhand = fr.limb(b, vec3(-0.38, 1.6, 0.0), left, 0.085, 0.7, arms);
    let rhand = fr.limb(b, vec3(0.38, 1.6, 0.0), right, 0.085, 0.7, arms);
    fr.sphere(b, lhand, 0.08, head_col);
    fr.sphere(b, rhand, 0.08, head_col);

    match gear {
        Gear::Warrior => {
            fr.limb(b, rhand, right + 1.6, 0.04, 1.0, steel);
            fr.limb(
                b,
                rhand,
                right + 1.6,
                0.12,
                0.06,
                Color::new(0.8, 0.65, 0.2, 1.0),
            );
            fr.cube(
                b,
                lhand + vec3(-0.05, 0.05, 0.12),
                vec3(0.04, 0.32, 0.26),
                Color::new(0.5, 0.15, 0.12, 1.0),
            );
            fr.cube(
                b,
                lhand + vec3(-0.1, 0.05, 0.12),
                vec3(0.02, 0.1, 0.08),
                Color::new(0.85, 0.7, 0.25, 1.0),
            );
            fr.cube(b, vec3(0.0, 1.95, 0.0), vec3(0.22, 0.1, 0.22), steel);
            fr.cube(b, vec3(-0.36, 1.66, 0.0), vec3(0.14, 0.08, 0.14), steel);
            fr.cube(b, vec3(0.36, 1.66, 0.0), vec3(0.14, 0.08, 0.14), steel);
        }
        Gear::Mage => {
            let hat = Color::new(0.28, 0.18, 0.62, 1.0);
            fr.cylinder(b, vec3(0.0, 1.95, 0.0), 0.04, 0.38, hat);
            fr.cone(b, vec3(0.0, 1.98, 0.0), 0.65, 0.24, hat);
            fr.cube(
                b,
                rhand + vec3(0.0, 0.25, 0.0),
                vec3(0.035, 0.9, 0.035),
                wood,
            );
            let orb = Color::new(0.55, 0.85, 1.0, 1.0);
            fr.sphere(b, rhand + vec3(0.0, 1.2, 0.0), 0.11, orb);
        }
        Gear::Cleric => {
            let gold = Color::new(0.9, 0.75, 0.3, 1.0);
            fr.cube(b, vec3(0.0, 1.27, 0.17), vec3(0.07, 0.3, 0.01), gold);
            fr.cylinder(b, vec3(0.0, 1.96, 0.0), 0.04, 0.21, gold);
            let head = fr.limb(b, rhand, right + 1.6, 0.035, 0.6, wood);
            fr.sphere(b, head, 0.12, Color::new(0.8, 0.8, 0.82, 1.0));
        }
        Gear::Bandit => {
            fr.cube(
                b,
                vec3(0.0, 1.8, 0.12),
                vec3(0.2, 0.06, 0.1),
                Color::new(0.12, 0.1, 0.1, 1.0),
            );
            fr.cube(
                b,
                vec3(0.0, 1.97, 0.0),
                vec3(0.21, 0.06, 0.21),
                Color::new(0.6, 0.12, 0.1, 1.0),
            );
            fr.limb(b, rhand, right + 1.6, 0.03, 0.5, steel);
        }
        Gear::Mystic => {
            let hood = Color::new(0.22, 0.08, 0.26, 1.0);
            fr.cone(b, vec3(0.0, 1.8, -0.02), 0.5, 0.25, hood);
            let glow = Color::new(0.75, 0.35, 1.0, 1.0);
            fr.sphere(b, rhand + vec3(0.0, -0.05, 0.1), 0.1, glow);
        }
        Gear::Golem => {
            fr.cube(b, lhand, vec3(0.15, 0.15, 0.15), legs);
            fr.cube(b, rhand, vec3(0.15, 0.15, 0.15), legs);
            let moss = Color::new(0.3, 0.5, 0.25, 1.0);
            fr.cube(b, vec3(0.0, 1.55, -0.1), vec3(0.22, 0.08, 0.12), moss);
        }
    }
}

fn quadruped(b: &mut Batch, pos: Vec3, yaw: f32, boar: bool, pose: Pose) {
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, 1.0, true)
    } else {
        Frame::upright(pos, yaw, 1.0)
    };
    let (fur, dark, body_half, body_y, leg_len) = if boar {
        (
            Color::new(0.45, 0.3, 0.2, 1.0),
            Color::new(0.3, 0.2, 0.13, 1.0),
            vec3(0.32, 0.3, 0.62),
            0.68,
            0.45,
        )
    } else {
        (
            Color::new(0.5, 0.5, 0.52, 1.0),
            Color::new(0.35, 0.35, 0.37, 1.0),
            vec3(0.22, 0.22, 0.6),
            0.78,
            0.58,
        )
    };
    let stride = if pose.moving && !pose.dead {
        pose.walk.sin() * 0.6
    } else {
        0.0
    };
    let lunge = if pose.swing > 0.0 && pose.swing < 1.0 {
        (pose.swing * std::f32::consts::PI).sin() * 0.3
    } else {
        0.0
    };
    fr.cube(b, vec3(0.0, body_y, 0.0), body_half, fur);
    for (x, z, a) in [
        (-1.0, 1.0, stride),
        (1.0, 1.0, -stride),
        (-1.0, -1.0, -stride),
        (1.0, -1.0, stride),
    ] {
        let joint = vec3(
            x * (body_half.x - 0.07),
            body_y - 0.1,
            z * (body_half.z - 0.12),
        );
        fr.limb(b, joint, a, 0.07, leg_len, dark);
    }
    let head = vec3(0.0, body_y + 0.18, body_half.z + 0.15 + lunge);
    fr.cube(b, head, vec3(0.17, 0.16, 0.2), fur);
    let eye = Color::new(0.08, 0.08, 0.08, 1.0);
    fr.sphere(b, head + vec3(-0.1, 0.06, 0.15), 0.035, eye);
    fr.sphere(b, head + vec3(0.1, 0.06, 0.15), 0.035, eye);
    if boar {
        fr.cube(
            b,
            head + vec3(0.0, -0.05, 0.28),
            vec3(0.11, 0.09, 0.1),
            Color::new(0.75, 0.5, 0.45, 1.0),
        );
        let tusk = Color::new(0.95, 0.93, 0.85, 1.0);
        fr.cube(
            b,
            head + vec3(-0.12, -0.08, 0.3),
            vec3(0.025, 0.08, 0.025),
            tusk,
        );
        fr.cube(
            b,
            head + vec3(0.12, -0.08, 0.3),
            vec3(0.025, 0.08, 0.025),
            tusk,
        );
        fr.cube(
            b,
            vec3(0.0, body_y + body_half.y, 0.0),
            vec3(0.05, 0.06, body_half.z * 0.8),
            dark,
        );
    } else {
        fr.cube(
            b,
            head + vec3(0.0, -0.05, 0.28),
            vec3(0.08, 0.07, 0.14),
            fur,
        );
        fr.sphere(b, head + vec3(0.0, -0.02, 0.42), 0.04, eye);
        fr.cone(b, head + vec3(-0.1, 0.14, -0.05), 0.18, 0.06, dark);
        fr.cone(b, head + vec3(0.1, 0.14, -0.05), 0.18, 0.06, dark);
        let wag = (pose.time * 3.0).sin() * 0.2;
        fr.limb(
            b,
            vec3(0.0, body_y + 0.1, -body_half.z),
            2.3 + wag,
            0.06,
            0.5,
            fur,
        );
    }
}

/// A tiny deterministic random number generator for placing scenery.
struct Scatter(u32);

impl Scatter {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 & 0xFFFFFF) as f32 / 0x1000000 as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// Places with special scenery. They match the mob camps on the server.
const BANDIT_CAMPS: [Vec2; 3] = [
    Vec2::new(92.0, -72.0),
    Vec2::new(122.0, 92.0),
    Vec2::new(-120.0, -120.0),
];
const GOLEM_RUINS: Vec2 = Vec2::new(-150.0, -150.0);

/// Everything that never moves, prebuilt into meshes.
pub struct Scene {
    meshes: Vec<Mesh>,
    water: Vec<Mesh>,
}

impl Scene {
    pub fn new() -> Self {
        let mut b = Batch::recording();
        terrain(&mut b);
        trees_and_rocks(&mut b);
        town(&mut b);
        camps(&mut b);
        let meshes = b.finish();

        let mut w = Batch::recording();
        let s = WORLD_HALF_SIZE;
        let c = Color::new(0.2, 0.45, 0.75, 0.7);
        let col = [color_u8(c.r), color_u8(c.g), color_u8(c.b), color_u8(c.a)];
        let base = w.reserve(4, 6);
        for (x, z) in [(-s, -s), (s, -s), (s, s), (-s, s)] {
            w.vertex(vec3(x, WATER_LEVEL, z), col);
        }
        w.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        let water = w.finish();
        Self { meshes, water }
    }

    pub fn draw(&self) {
        for m in &self.meshes {
            draw_mesh(m);
        }
    }

    /// Draw last: it's see-through.
    pub fn draw_water(&self) {
        for m in &self.water {
            draw_mesh(m);
        }
    }
}

fn terrain_color(x: f32, z: f32, h: f32, slope: f32) -> Color {
    let n = ((x * 0.37).sin() * (z * 0.41).cos() + (x * 0.11 + z * 0.13).sin()) * 0.5;
    let grass = Color::new(0.33 + n * 0.05, 0.55 + n * 0.07, 0.24, 1.0);
    let sand = Color::new(0.78, 0.72, 0.52, 1.0);
    let rock = Color::new(0.47, 0.45, 0.42, 1.0);
    let snow = Color::new(0.93, 0.94, 0.97, 1.0);
    let cobble = Color::new(0.6 + n * 0.05, 0.56 + n * 0.05, 0.5, 1.0);
    let mix = |a: Color, b: Color, t: f32| {
        let t = t.clamp(0.0, 1.0);
        Color::new(
            a.r + (b.r - a.r) * t,
            a.g + (b.g - a.g) * t,
            a.b + (b.b - a.b) * t,
            1.0,
        )
    };
    let mut c = mix(sand, grass, (h - WATER_LEVEL - 0.3) / 1.2);
    c = mix(c, rock, (slope - 0.45) * 3.0);
    c = mix(c, rock, (h - 12.0) / 3.0);
    c = mix(c, snow, (h - 18.0) / 2.0);
    let d = (x * x + z * z).sqrt();
    // Roads out of town along the axes.
    let road = (1.0 - x.abs().min(z.abs()) / 3.0).clamp(0.0, 1.0)
        * (1.0 - (d - 70.0) / 20.0).clamp(0.0, 1.0);
    c = mix(c, Color::new(0.55, 0.47, 0.35, 1.0), road * 0.85);
    mix(c, cobble, (TOWN_RADIUS - d) / 3.0)
}

fn terrain(b: &mut Batch) {
    let step = 4.0;
    let n = (WORLD_HALF_SIZE * 2.0 / step) as usize;
    let base = b.reserve((n + 1) * (n + 1), n * n * 6);
    for j in 0..=n {
        for i in 0..=n {
            let x = -WORLD_HALF_SIZE + i as f32 * step;
            let z = -WORLD_HALF_SIZE + j as f32 * step;
            let h = terrain_height(x, z);
            let e = 0.5;
            let normal = vec3(
                terrain_height(x - e, z) - terrain_height(x + e, z),
                2.0 * e,
                terrain_height(x, z - e) - terrain_height(x, z + e),
            )
            .normalize();
            let color = terrain_color(x, z, h, 1.0 - normal.y);
            b.vertex(vec3(x, h, z), shade(color, normal));
        }
    }
    let row = n as u16 + 1;
    for j in 0..n as u16 {
        for i in 0..n as u16 {
            let a = base + j * row + i;
            let c = a + row;
            b.indices.extend_from_slice(&[a, c, a + 1, a + 1, c, c + 1]);
        }
    }
    b.flush();
}

fn near_landmark(x: f32, z: f32) -> bool {
    let p = Vec2::new(x, z);
    let d = p.length();
    d < TOWN_RADIUS + 8.0
        || x.abs() < 5.0 && d < 90.0
        || z.abs() < 5.0 && d < 90.0
        || BANDIT_CAMPS.iter().any(|c| c.distance(p) < 14.0)
        || GOLEM_RUINS.distance(p) < 16.0
}

fn trees_and_rocks(b: &mut Batch) {
    let mut rng = Scatter(0x9E3779B9);
    let trunk = Color::new(0.4, 0.27, 0.16, 1.0);
    for _ in 0..900 {
        let x = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let z = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let kind = rng.next();
        let size = rng.range(0.8, 1.4);
        let shade_n = rng.range(-0.05, 0.05);
        let h = terrain_height(x, z);
        if near_landmark(x, z) || !(WATER_LEVEL + 0.4..=16.0).contains(&h) {
            continue;
        }
        // Clump trees together: skip most of them in "meadow" areas.
        let forest = ((x * 0.02).sin() + (z * 0.025).cos()) * 0.5;
        if kind < 0.75 && forest < -0.1 && rng.next() < 0.8 {
            continue;
        }
        let p = vec3(x, h - 0.2, z);
        if kind < 0.45 {
            // Pine.
            b.cylinder(p, Vec3::Y * 1.6 * size, 0.22 * size, 6, trunk);
            let green = Color::new(0.16 + shade_n, 0.38 + shade_n, 0.2, 1.0);
            for (y, r, hgt) in [(1.2, 1.5, 2.2), (2.4, 1.15, 1.9), (3.5, 0.8, 1.6)] {
                b.cone(
                    p + Vec3::Y * y * size,
                    Vec3::Y * hgt * size,
                    r * size,
                    0.0,
                    7,
                    green,
                );
            }
        } else if kind < 0.75 {
            // Leafy tree.
            b.cylinder(p, Vec3::Y * 2.2 * size, 0.25 * size, 6, trunk);
            let green = Color::new(0.28 + shade_n, 0.52 + shade_n, 0.22, 1.0);
            b.sphere(p + Vec3::Y * 3.0 * size, 1.4 * size, green);
            b.sphere(p + vec3(0.8, 2.6, 0.3) * size, 0.95 * size, green);
            b.sphere(p + vec3(-0.7, 2.7, -0.4) * size, 1.0 * size, green);
        } else if kind < 0.9 {
            // Rock.
            let yaw = rng.range(0.0, std::f32::consts::TAU);
            let grey = Color::new(0.52 + shade_n, 0.5 + shade_n, 0.48, 1.0);
            b.block(
                p + Vec3::Y * 0.4 * size,
                vec3(0.9, 0.6, 0.7) * size,
                yaw,
                grey,
            );
            b.block(
                p + vec3(0.5, 0.9, 0.2) * size,
                vec3(0.5, 0.35, 0.45) * size,
                yaw + 0.7,
                grey,
            );
        } else {
            // Bush.
            let green = Color::new(0.25 + shade_n, 0.45 + shade_n, 0.2, 1.0);
            b.sphere(p + Vec3::Y * 0.5 * size, 0.7 * size, green);
        }
    }
}

fn house(b: &mut Batch, center: Vec3, yaw: f32, wall: Color, roof: Color) {
    let half = vec3(3.0, 1.8, 2.4);
    b.block(center + Vec3::Y * half.y, half, yaw, wall);
    let f = forward(yaw);
    let r = vec3(-f.z, 0.0, f.x);
    // Beams on the corners.
    let beam = Color::new(0.35, 0.22, 0.12, 1.0);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let p = center + r * sx * half.x + f * sz * half.z + Vec3::Y * half.y;
        b.block(p, vec3(0.15, half.y, 0.15), yaw, beam);
    }
    // Gabled roof running along the house's width.
    let eave = 0.4;
    let top = center + Vec3::Y * (half.y * 2.0 + 1.8);
    let base_y = half.y * 2.0;
    let corner = |sx: f32, sz: f32| {
        center + r * sx * (half.x + eave) + f * sz * (half.z + eave) + Vec3::Y * base_y
    };
    let ridge = |sx: f32| top + r * sx * (half.x + eave);
    for sz in [-1.0, 1.0] {
        let n = (f * sz * 1.8 + Vec3::Y * (half.z + eave)).normalize();
        b.quad(
            [corner(-1.0, sz), corner(1.0, sz), ridge(1.0), ridge(-1.0)],
            n,
            roof,
        );
    }
    for sx in [-1.0, 1.0] {
        b.triangle(corner(sx, -1.0), corner(sx, 1.0), ridge(sx), wall);
    }
    // Door and windows on the front.
    let front = center + f * (half.z + 0.02);
    b.block(
        front + Vec3::Y * 1.0,
        vec3(0.6, 1.0, 0.03),
        yaw,
        Color::new(0.35, 0.22, 0.12, 1.0),
    );
    let glass = Color::new(0.95, 0.85, 0.5, 1.0);
    for sx in [-1.0, 1.0] {
        b.block(
            front + r * sx * 1.8 + Vec3::Y * 2.0,
            vec3(0.45, 0.4, 0.03),
            yaw,
            glass,
        );
    }
}

fn town(b: &mut Batch) {
    let walls = [
        Color::new(0.9, 0.85, 0.72, 1.0),
        Color::new(0.85, 0.8, 0.7, 1.0),
        Color::new(0.92, 0.88, 0.8, 1.0),
    ];
    let roofs = [
        Color::new(0.65, 0.25, 0.18, 1.0),
        Color::new(0.3, 0.38, 0.55, 1.0),
        Color::new(0.45, 0.3, 0.2, 1.0),
    ];
    for (i, deg) in [28.0f32, 62.0, 118.0, 152.0, 208.0, 242.0, 298.0, 332.0]
        .into_iter()
        .enumerate()
    {
        let a = deg.to_radians();
        let p = vec3(a.cos() * 20.0, 0.0, a.sin() * 20.0);
        let yaw = yaw_towards(p, Vec3::ZERO);
        house(b, p, yaw, walls[i % 3], roofs[i % 3]);
    }
    // Fountain in the square.
    let stone = Color::new(0.65, 0.63, 0.6, 1.0);
    b.cylinder(Vec3::ZERO, Vec3::Y * 0.7, 3.0, 16, stone);
    b.cylinder(
        Vec3::Y * 0.7,
        Vec3::Y * 0.02,
        2.6,
        16,
        Color::new(0.3, 0.55, 0.85, 1.0),
    );
    b.cylinder(Vec3::ZERO, Vec3::Y * 2.2, 0.35, 8, stone);
    b.cylinder(Vec3::Y * 2.2, Vec3::Y * 0.25, 1.0, 12, stone);
    b.sphere(Vec3::Y * 2.7, 0.35, Color::new(0.5, 0.75, 1.0, 1.0));
    // A small graveyard beside where players appear.
    let grave = Color::new(0.6, 0.6, 0.62, 1.0);
    for row in 0..2 {
        for i in 0..3 {
            let x = -9.0 - i as f32 * 2.2;
            let z = GRAVEYARD.y + 4.0 - row as f32 * 3.0;
            b.block(vec3(x, 0.6, z), vec3(0.12, 0.6, 0.45), 0.0, grave);
            b.block(
                vec3(x + 0.9, 0.05, z),
                vec3(0.8, 0.05, 0.45),
                0.0,
                Color::new(0.35, 0.3, 0.22, 1.0),
            );
        }
    }
    // Lamp posts at the edge of town on each road.
    for (x, z) in [(4.0, 26.0), (-4.0, -26.0), (26.0, -4.0), (-26.0, 4.0)] {
        b.cylinder(
            vec3(x, 0.0, z),
            Vec3::Y * 3.5,
            0.12,
            6,
            Color::new(0.2, 0.2, 0.22, 1.0),
        );
        b.sphere(vec3(x, 3.7, z), 0.3, Color::new(1.0, 0.9, 0.5, 1.0));
    }
}

fn camps(b: &mut Batch) {
    let canvas = Color::new(0.6, 0.5, 0.35, 1.0);
    let wood = Color::new(0.4, 0.27, 0.16, 1.0);
    for c in BANDIT_CAMPS {
        for i in 0..4 {
            let a = i as f32 * 1.6 + 0.4;
            let x = c.x + a.cos() * 7.0;
            let z = c.y + a.sin() * 7.0;
            let p = ground(x, z) - Vec3::Y * 0.2;
            b.cone(p, Vec3::Y * 3.0, 2.2, 0.0, 5, canvas);
        }
        let fire = ground(c.x, c.y);
        for i in 0..3 {
            let a = i as f32 * 2.1;
            b.block(fire + Vec3::Y * 0.15, vec3(0.9, 0.12, 0.12), a, wood);
        }
        b.sphere(fire + Vec3::Y * 0.4, 0.35, Color::new(1.0, 0.55, 0.15, 1.0));
        b.sphere(fire + Vec3::Y * 0.75, 0.2, Color::new(1.0, 0.85, 0.3, 1.0));
    }
    let stone = Color::new(0.55, 0.55, 0.52, 1.0);
    for i in 0..9 {
        let a = i as f32 * std::f32::consts::TAU / 9.0;
        let p = ground(
            GOLEM_RUINS.x + a.cos() * 11.0,
            GOLEM_RUINS.y + a.sin() * 11.0,
        ) - Vec3::Y * 0.3;
        let height = [6.0, 2.5, 5.0, 1.5, 6.5, 3.5, 4.0, 2.0, 5.5][i];
        b.cylinder(p, Vec3::Y * height, 0.8, 8, stone);
        if height > 5.0 {
            b.block(
                p + Vec3::Y * (height + 0.25),
                vec3(1.1, 0.25, 1.1),
                a,
                stone,
            );
        }
    }
}
