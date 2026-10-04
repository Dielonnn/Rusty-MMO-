//! 3D drawing: a triangle batcher with warm dusk lighting and distance fog,
//! the static scenery of Amberfall Vale (autumn hills, farms, the town of
//! Hearthmere, bandit camps, golem ruins) and the character models.

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::models::{Mesh, Vertex, draw_mesh};
use macroquad::prelude::*;
use shared::data::{Appearance, Class, ItemId, MobKind, Slot, item, items};
use shared::protocol::EntityKind;
use shared::world::*;

pub const ZONE_NAME: &str = "Amberfall Vale";
pub const ZONE_SUBTITLE: &str = "Human starting area";
pub const TOWN_NAME: &str = "Hearthmere";

const MAX_VERTICES: usize = 60_000;
const MAX_INDICES: usize = 180_000;

/// Where the low dusk sun is (the direction light comes from).
pub fn sun_dir() -> Vec3 {
    vec3(-0.75, 0.28, 0.45).normalize()
}

pub const SKY_TOP: Color = Color::new(0.16, 0.15, 0.34, 1.0);
pub const SKY_MID: Color = Color::new(0.55, 0.36, 0.5, 1.0);
pub const SKY_HORIZON: Color = Color::new(0.98, 0.6, 0.36, 1.0);
pub const FOG: Color = Color::new(0.74, 0.5, 0.44, 1.0);

/// Lights a surface facing `normal`: a warm low sun plus cool sky light.
fn shade(color: Color, normal: Vec3) -> [u8; 4] {
    let n = normal.normalize_or_zero();
    let d = n.dot(sun_dir()).max(0.0);
    let sky = 0.5 + 0.5 * n.y;
    let light = vec3(1.0, 0.76, 0.55) * d * 0.95 + vec3(0.42, 0.4, 0.52) * (0.6 + 0.4 * sky);
    [
        (color.r * light.x * 255.0).clamp(0.0, 255.0) as u8,
        (color.g * light.y * 255.0).clamp(0.0, 255.0) as u8,
        (color.b * light.z * 255.0).clamp(0.0, 255.0) as u8,
        (color.a * 255.0).clamp(0.0, 255.0) as u8,
    ]
}

/// Full brightness, for things that glow.
fn glow(color: Color) -> [u8; 4] {
    [
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
        (color.a * 255.0) as u8,
    ]
}

fn mix(a: Color, b: Color, t: f32) -> Color {
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

/// Collects triangles and draws them in as few calls as possible.
/// In recording mode it keeps the meshes instead, for static scenery.
pub struct Batch {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    recorded: Option<Vec<Mesh>>,
    /// Skip lighting: everything drawn is emissive.
    pub glowing: bool,
}

impl Batch {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            recorded: None,
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
            shade(color, normal)
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

    /// An ellipsoid with radii along X, Y and Z.
    pub fn ellipsoid(&mut self, center: Vec3, radii: Vec3, color: Color) {
        const SLICES: usize = 12;
        const STACKS: usize = 8;
        let base = self.reserve((SLICES + 1) * (STACKS + 1), SLICES * STACKS * 6);
        for j in 0..=STACKS {
            let phi = std::f32::consts::PI * j as f32 / STACKS as f32;
            for i in 0..=SLICES {
                let theta = std::f32::consts::TAU * i as f32 / SLICES as f32;
                let n = vec3(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
                let c = self.color(color, n / radii);
                self.vertex(center + n * radii, c);
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

    pub fn sphere(&mut self, center: Vec3, radius: f32, color: Color) {
        self.ellipsoid(center, Vec3::splat(radius), color);
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
                let c = self.color(color, n);
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
        let was = self.glowing;
        self.glowing = true;
        self.sphere(center, radius, color);
        self.glowing = was;
    }
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

    pub fn sphere(&self, b: &mut Batch, center: Vec3, radius: f32, color: Color) {
        b.sphere(self.p(center), radius * self.scale, color);
    }

    pub fn ellipsoid(&self, b: &mut Batch, center: Vec3, radii: Vec3, color: Color) {
        // Only exact for upright frames; good enough for round shapes.
        b.ellipsoid(self.p(center), radii * self.scale, color);
    }

    pub fn glow(&self, b: &mut Batch, center: Vec3, radius: f32, color: Color) {
        b.glow_sphere(self.p(center), radius * self.scale, color);
    }

    pub fn cone(&self, b: &mut Batch, base: Vec3, height: f32, radius: f32, color: Color) {
        b.cone(
            self.p(base),
            self.u * height * self.scale,
            radius * self.scale,
            0.0,
            10,
            color,
        );
    }

    /// A cone pointing along a local direction.
    pub fn cone_dir(&self, b: &mut Batch, base: Vec3, dir: Vec3, radius: f32, color: Color) {
        b.cone(
            self.p(base),
            self.dir(dir) * self.scale,
            radius * self.scale,
            0.0,
            6,
            color,
        );
    }

    pub fn cylinder(&self, b: &mut Batch, base: Vec3, height: f32, radius: f32, color: Color) {
        b.cylinder(
            self.p(base),
            self.u * height * self.scale,
            radius * self.scale,
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

/// How tall something is, for nameplates and picking.
pub fn model_height(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::Player(_) => 2.1,
        EntityKind::Mob { kind, .. } => match kind {
            MobKind::Wolf => 1.3,
            MobKind::Boar => 1.25,
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

pub fn draw_model(b: &mut Batch, look: &Look, pos: Vec3, yaw: f32, pose: Pose) {
    match look.kind {
        EntityKind::Player(class) => {
            let gear = match class {
                Class::Warrior => Gear::Warrior,
                Class::Mage => Gear::Mage,
                Class::Cleric => Gear::Cleric,
                Class::Rogue => Gear::Rogue,
            };
            humanoid(b, pos, yaw, 1.0, gear, look, pose);
        }
        EntityKind::Mob { kind, .. } => match kind {
            MobKind::Wolf => wolf(b, pos, yaw, look.seed, pose),
            MobKind::Boar => boar(b, pos, yaw, look.seed, pose),
            MobKind::Bandit => humanoid(b, pos, yaw, 1.0, Gear::Bandit, look, pose),
            MobKind::BanditMystic => humanoid(b, pos, yaw, 1.0, Gear::Mystic, look, pose),
            MobKind::Golem => humanoid(b, pos, yaw, 2.2, Gear::Golem, look, pose),
        },
    }
}

pub fn skin_color(i: u8) -> Color {
    [
        Color::new(0.98, 0.84, 0.72, 1.0),
        Color::new(0.93, 0.74, 0.58, 1.0),
        Color::new(0.8, 0.6, 0.44, 1.0),
        Color::new(0.6, 0.42, 0.3, 1.0),
        Color::new(0.4, 0.27, 0.19, 1.0),
    ][i as usize % 5]
}

pub fn hair_color(i: u8) -> Color {
    [
        Color::new(0.12, 0.09, 0.07, 1.0),
        Color::new(0.38, 0.22, 0.12, 1.0),
        Color::new(0.85, 0.68, 0.35, 1.0),
        Color::new(0.62, 0.22, 0.1, 1.0),
        Color::new(0.75, 0.75, 0.75, 1.0),
        Color::new(0.95, 0.92, 0.85, 1.0),
    ][i as usize % 6]
}

#[derive(Clone, Copy, PartialEq)]
enum Gear {
    Warrior,
    Mage,
    Cleric,
    Rogue,
    Bandit,
    Mystic,
    Golem,
}

/// Darker version of a color.
fn dark(c: Color, f: f32) -> Color {
    Color::new(c.r * f, c.g * f, c.b * f, c.a)
}

fn humanoid(b: &mut Batch, pos: Vec3, yaw: f32, scale: f32, gear: Gear, look: &Look, pose: Pose) {
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, scale, false)
    } else {
        Frame::upright(pos, yaw, scale)
    };
    let steel = Color::new(0.72, 0.74, 0.78, 1.0);
    let gold = Color::new(0.92, 0.74, 0.3, 1.0);
    let wood = Color::new(0.42, 0.28, 0.16, 1.0);
    let leather = Color::new(0.42, 0.28, 0.17, 1.0);
    let a = look.appearance;
    let mob_skin = skin_color((look.seed % 4) as u8);
    let (skin, hair) = match gear {
        Gear::Bandit | Gear::Mystic => (mob_skin, hair_color((look.seed / 3 % 4) as u8)),
        _ => (skin_color(a.skin), hair_color(a.hair_color)),
    };
    let slender = a.body == 1 && !matches!(gear, Gear::Bandit | Gear::Mystic | Gear::Golem);
    let (mut torso, mut legs, mut arms, mut boots) = match gear {
        Gear::Warrior => (
            Color::new(0.62, 0.17, 0.14, 1.0),
            Color::new(0.33, 0.29, 0.27, 1.0),
            steel,
            Color::new(0.3, 0.22, 0.16, 1.0),
        ),
        Gear::Mage => (
            Color::new(0.34, 0.24, 0.72, 1.0),
            Color::new(0.28, 0.2, 0.62, 1.0),
            Color::new(0.34, 0.24, 0.72, 1.0),
            Color::new(0.25, 0.18, 0.4, 1.0),
        ),
        Gear::Cleric => (
            Color::new(0.95, 0.93, 0.86, 1.0),
            Color::new(0.9, 0.88, 0.8, 1.0),
            Color::new(0.95, 0.93, 0.86, 1.0),
            Color::new(0.55, 0.45, 0.3, 1.0),
        ),
        Gear::Rogue => (
            Color::new(0.2, 0.2, 0.22, 1.0),
            Color::new(0.17, 0.17, 0.19, 1.0),
            Color::new(0.2, 0.2, 0.22, 1.0),
            Color::new(0.14, 0.12, 0.11, 1.0),
        ),
        Gear::Bandit => (
            Color::new(0.46, 0.32, 0.2, 1.0),
            Color::new(0.26, 0.23, 0.2, 1.0),
            Color::new(0.46, 0.32, 0.2, 1.0),
            Color::new(0.22, 0.16, 0.12, 1.0),
        ),
        Gear::Mystic => (
            Color::new(0.32, 0.12, 0.36, 1.0),
            Color::new(0.26, 0.1, 0.3, 1.0),
            Color::new(0.32, 0.12, 0.36, 1.0),
            Color::new(0.18, 0.1, 0.18, 1.0),
        ),
        Gear::Golem => {
            let stone = Color::new(0.5, 0.49, 0.46, 1.0);
            (
                stone,
                Color::new(0.42, 0.41, 0.39, 1.0),
                Color::new(0.46, 0.45, 0.43, 1.0),
                Color::new(0.38, 0.37, 0.35, 1.0),
            )
        }
    };
    // Worn armor shows on the body.
    let worn = |s: Slot| look.gear[s.index()].map(|id| rgb(item(id).color));
    let mut hands = if gear == Gear::Golem { legs } else { skin };
    if let Some(c) = worn(Slot::Chest) {
        torso = c;
        arms = dark(c, 0.92);
    }
    if let Some(c) = worn(Slot::Legs) {
        legs = c;
    }
    if let Some(c) = worn(Slot::Feet) {
        boots = c;
    }
    if let Some(c) = worn(Slot::Hands) {
        hands = c;
    }
    let robed =
        matches!(gear, Gear::Mage | Gear::Cleric | Gear::Mystic) && worn(Slot::Chest).is_none();

    let moving = pose.moving && !pose.dead;
    let stride = if moving { pose.walk.sin() * 0.65 } else { 0.0 };
    // Legs: thigh, shin with a bending knee, and a boot.
    for (side, phase) in [(-1.0f32, 0.0f32), (1.0, std::f32::consts::PI)] {
        let swing = if moving {
            (pose.walk + phase).sin() * 0.65
        } else {
            0.0
        };
        let bend = if moving {
            ((pose.walk + phase + 1.2).sin()).max(0.0) * 0.9
        } else {
            0.0
        };
        let hip = vec3(0.13 * side * if slender { 1.1 } else { 1.0 }, 0.95, 0.0);
        let knee = fr.limb(b, hip, swing, 0.1, 0.46, legs);
        let ankle = fr.limb(b, knee, swing - bend, 0.085, 0.45, legs);
        fr.cube(
            b,
            ankle + vec3(0.0, -0.01, 0.07),
            vec3(0.085, 0.06, 0.15),
            boots,
        );
    }
    if robed {
        // A robe skirt that sways a little.
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

    // Body.
    let bob = if moving {
        (pose.walk * 2.0).sin().abs() * 0.03
    } else {
        0.0
    };
    let breathe = (pose.time * 1.6).sin() * 0.01;
    let y = bob;
    let hips_w = if slender { 0.25 } else { 0.24 };
    let chest_w = if slender { 0.25 } else { 0.3 };
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
    // Belt with a buckle.
    fr.cube(
        b,
        vec3(0.0, 1.07 + y, 0.0),
        vec3(hips_w + 0.01, 0.04, 0.155),
        Color::new(0.25, 0.17, 0.1, 1.0),
    );
    fr.cube(b, vec3(0.0, 1.07 + y, 0.16), vec3(0.045, 0.035, 0.01), gold);
    // Neck and head.
    fr.cylinder(b, vec3(0.0, 1.53 + y, 0.0), 0.1, 0.075, skin);
    let head = vec3(0.0, 1.76 + y, 0.01);
    if gear == Gear::Golem {
        fr.cube(b, head + vec3(0.0, 0.03, 0.0), vec3(0.2, 0.19, 0.19), torso);
        fr.cube(
            b,
            head + vec3(0.0, 0.17, -0.02),
            vec3(0.16, 0.05, 0.15),
            dark(torso, 0.85),
        );
        let eye = Color::new(0.4, 0.95, 1.0, 1.0);
        fr.glow(b, head + vec3(-0.08, 0.05, 0.19), 0.045, eye);
        fr.glow(b, head + vec3(0.08, 0.05, 0.19), 0.045, eye);
    } else {
        fr.ellipsoid(b, head, vec3(0.19, 0.21, 0.2), skin);
        // Face.
        let white = Color::new(0.95, 0.95, 0.95, 1.0);
        let pupil = Color::new(0.12, 0.1, 0.1, 1.0);
        for sx in [-1.0, 1.0] {
            fr.sphere(b, head + vec3(0.07 * sx, 0.03, 0.17), 0.035, white);
            fr.sphere(b, head + vec3(0.07 * sx, 0.03, 0.195), 0.018, pupil);
            fr.cube(
                b,
                head + vec3(0.07 * sx, 0.085, 0.18),
                vec3(0.04, 0.012, 0.012),
                dark(hair, 0.9),
            );
            fr.sphere(b, head + vec3(0.19 * sx, 0.0, 0.0), 0.04, skin);
        }
        fr.cube(
            b,
            head + vec3(0.0, -0.015, 0.205),
            vec3(0.022, 0.04, 0.025),
            dark(skin, 0.9),
        );
        fr.cube(
            b,
            head + vec3(0.0, -0.09, 0.18),
            vec3(0.045, 0.01, 0.01),
            Color::new(0.55, 0.28, 0.25, 1.0),
        );
    }

    // Hair (players pick a style; mobs vary by seed).
    let style = match gear {
        Gear::Golem | Gear::Mystic => 0,
        Gear::Bandit => 1 + (look.seed % 2) as u8,
        _ => a.hair_style,
    };
    let headgear = worn(Slot::Head);
    if headgear.is_none() {
        let top = head + vec3(0.0, 0.17, -0.01);
        match style {
            1 => {
                fr.cube(b, top, vec3(0.2, 0.06, 0.2), hair);
                fr.cube(
                    b,
                    head + vec3(0.0, 0.05, -0.13),
                    vec3(0.2, 0.13, 0.08),
                    hair,
                );
            }
            2 => {
                fr.cube(b, top, vec3(0.21, 0.06, 0.21), hair);
                fr.cube(
                    b,
                    head + vec3(0.0, -0.08, -0.14),
                    vec3(0.21, 0.26, 0.07),
                    hair,
                );
                for sx in [-1.0, 1.0] {
                    fr.cube(
                        b,
                        head + vec3(0.19 * sx, -0.04, -0.03),
                        vec3(0.03, 0.2, 0.09),
                        hair,
                    );
                }
            }
            3 => {
                fr.cube(b, top, vec3(0.2, 0.06, 0.2), hair);
                fr.cube(
                    b,
                    head + vec3(0.0, 0.05, -0.14),
                    vec3(0.19, 0.12, 0.07),
                    hair,
                );
                let sway = (pose.time * 2.0).sin() * 0.1 + stride * 0.2;
                let tie = head + vec3(0.0, 0.05, -0.21);
                fr.sphere(b, tie, 0.05, dark(hair, 0.8));
                fr.limb(b, tie, -0.35 + sway, 0.045, 0.32, hair);
            }
            4 => {
                fr.cube(
                    b,
                    head + vec3(0.0, 0.2, -0.02),
                    vec3(0.035, 0.08, 0.19),
                    hair,
                );
            }
            _ => {}
        }
    }
    match headgear {
        Some(c) => {
            let hood = look.gear[Slot::Head.index()] == Some(items::LINEN_HOOD);
            if hood {
                fr.cube(b, head + vec3(0.0, 0.08, -0.03), vec3(0.23, 0.18, 0.21), c);
                fr.cone(b, head + vec3(0.0, 0.2, -0.06), 0.22, 0.16, c);
            } else {
                fr.cube(b, head + vec3(0.0, 0.15, 0.0), vec3(0.215, 0.08, 0.215), c);
                fr.cube(
                    b,
                    head + vec3(0.0, 0.09, 0.2),
                    vec3(0.2, 0.025, 0.04),
                    dark(c, 0.8),
                );
            }
        }
        None => match gear {
            Gear::Warrior => {
                // A leather headband, so the hair you picked shows.
                fr.cube(
                    b,
                    head + vec3(0.0, 0.1, 0.0),
                    vec3(0.205, 0.025, 0.205),
                    Color::new(0.55, 0.12, 0.1, 1.0),
                );
            }
            Gear::Mage => {
                let hat = Color::new(0.27, 0.17, 0.6, 1.0);
                fr.cylinder(b, head + vec3(0.0, 0.14, 0.0), 0.04, 0.38, hat);
                fr.cone(b, head + vec3(0.0, 0.17, 0.0), 0.65, 0.23, hat);
                fr.cylinder(b, head + vec3(0.0, 0.18, 0.0), 0.05, 0.235, gold);
            }
            Gear::Cleric => {
                fr.cylinder(b, head + vec3(0.0, 0.18, 0.0), 0.035, 0.205, gold);
                fr.glow(
                    b,
                    head + vec3(0.0, 0.2, 0.2),
                    0.03,
                    Color::new(1.0, 0.95, 0.6, 1.0),
                );
            }
            Gear::Rogue => {
                // A mask over the lower face.
                let mask = Color::new(0.12, 0.12, 0.14, 1.0);
                fr.cube(
                    b,
                    head + vec3(0.0, -0.075, 0.12),
                    vec3(0.195, 0.065, 0.1),
                    mask,
                );
                fr.cube(
                    b,
                    head + vec3(0.0, -0.1, -0.02),
                    vec3(0.2, 0.05, 0.17),
                    mask,
                );
            }
            Gear::Bandit => {
                fr.cube(
                    b,
                    head + vec3(0.0, -0.06, 0.15),
                    vec3(0.19, 0.07, 0.07),
                    Color::new(0.65, 0.12, 0.1, 1.0),
                );
                if look.seed.is_multiple_of(3) {
                    fr.cube(
                        b,
                        head + vec3(0.0, 0.14, -0.02),
                        vec3(0.22, 0.07, 0.22),
                        Color::new(0.3, 0.22, 0.14, 1.0),
                    );
                }
            }
            Gear::Mystic => {
                let hood = Color::new(0.22, 0.07, 0.25, 1.0);
                fr.cube(
                    b,
                    head + vec3(0.0, 0.06, -0.04),
                    vec3(0.23, 0.2, 0.21),
                    hood,
                );
                fr.cone(b, head + vec3(0.0, 0.22, -0.08), 0.32, 0.17, hood);
                let eye = Color::new(0.85, 0.4, 1.0, 1.0);
                fr.glow(b, head + vec3(-0.07, 0.03, 0.2), 0.03, eye);
                fr.glow(b, head + vec3(0.07, 0.03, 0.2), 0.03, eye);
            }
            Gear::Golem => {}
        },
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
        fr.sphere(b, shoulder, 0.1, arms);
        let el = fr.limb(b, shoulder, angle, 0.08, 0.34, arms);
        let wrist = fr.limb(
            b,
            el,
            angle + elbow,
            0.07,
            0.32,
            if robed { arms } else { dark(arms, 0.95) },
        );
        fr.sphere(b, wrist + vec3(0.0, -0.04, 0.0), 0.075, hands);
        hand_pos[i] = wrist + vec3(0.0, -0.05, 0.0);
    }
    let [lhand, rhand] = hand_pos;
    let weapon_angle = right + r_elbow + 1.55;

    match gear {
        Gear::Warrior => {
            // Shoulder plates, a cape, sword and shield.
            for sx in [-1.0, 1.0] {
                fr.ellipsoid(
                    b,
                    vec3(sw * sx, shoulder_y + 0.06, 0.0),
                    vec3(0.15, 0.08, 0.15),
                    steel,
                );
            }
            fr.tilted(
                b,
                vec3(0.0, 1.15 + y, -0.21),
                vec3(0.26, 0.42, 0.02),
                -0.1 - stride.abs() * 0.15,
                Color::new(0.6, 0.12, 0.1, 1.0),
            );
            let guard = fr.limb(b, rhand, weapon_angle, 0.025, 0.12, wood);
            fr.limb(b, guard, weapon_angle, 0.13, 0.04, gold);
            fr.limb(b, guard, weapon_angle, 0.04, 0.9, steel);
            fr.cube(
                b,
                lhand + vec3(-0.07, 0.05, 0.13),
                vec3(0.04, 0.33, 0.27),
                Color::new(0.5, 0.14, 0.11, 1.0),
            );
            fr.cube(
                b,
                lhand + vec3(-0.11, 0.05, 0.13),
                vec3(0.015, 0.34, 0.03),
                gold,
            );
            fr.cube(
                b,
                lhand + vec3(-0.11, 0.05, 0.13),
                vec3(0.015, 0.03, 0.28),
                gold,
            );
        }
        Gear::Mage => {
            fr.tilted(
                b,
                vec3(0.0, 1.12 + y, -0.21),
                vec3(0.25, 0.45, 0.02),
                -0.08 - stride.abs() * 0.12,
                Color::new(0.24, 0.14, 0.5, 1.0),
            );
            fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.04, 0.27, 0.01), gold);
            fr.cube(b, rhand + vec3(0.0, 0.3, 0.0), vec3(0.03, 0.95, 0.03), wood);
            fr.glow(
                b,
                rhand + vec3(0.0, 1.32, 0.0),
                0.1,
                Color::new(0.55, 0.85, 1.0, 1.0),
            );
            fr.cone_dir(
                b,
                rhand + vec3(0.0, 1.2, 0.0),
                vec3(0.0, 0.15, 0.0),
                0.06,
                gold,
            );
        }
        Gear::Cleric => {
            fr.tilted(
                b,
                vec3(0.0, 1.12 + y, -0.21),
                vec3(0.25, 0.45, 0.02),
                -0.08 - stride.abs() * 0.12,
                Color::new(0.9, 0.8, 0.45, 1.0),
            );
            fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.07, 0.3, 0.01), gold);
            fr.cube(b, vec3(0.0, 1.38 + y, 0.18), vec3(0.14, 0.035, 0.01), gold);
            let head = fr.limb(b, rhand, weapon_angle, 0.03, 0.6, wood);
            fr.sphere(b, head, 0.11, Color::new(0.82, 0.82, 0.85, 1.0));
            for d in [
                vec3(0.12, 0.0, 0.0),
                vec3(-0.12, 0.0, 0.0),
                vec3(0.0, 0.0, 0.12),
                vec3(0.0, 0.0, -0.12),
            ] {
                fr.sphere(b, head + d, 0.04, gold);
            }
        }
        Gear::Rogue => {
            fr.cube(
                b,
                vec3(0.0, 1.36 + y, 0.0),
                vec3(chest_w + 0.01, 0.025, 0.175),
                leather,
            );
            fr.tilted(
                b,
                vec3(0.0, 1.38 + y, 0.0),
                vec3(0.02, 0.2, 0.18),
                0.6,
                leather,
            );
            for (hand, angle) in [(rhand, weapon_angle), (lhand, left + l_elbow + 1.55)] {
                let hilt = fr.limb(b, hand, angle, 0.025, 0.1, Color::new(0.2, 0.12, 0.08, 1.0));
                fr.limb(b, hilt, angle, 0.08, 0.03, steel);
                fr.limb(
                    b,
                    hilt,
                    angle,
                    0.03,
                    0.42,
                    Color::new(0.82, 0.84, 0.88, 1.0),
                );
            }
        }
        Gear::Bandit => {
            fr.cube(
                b,
                vec3(0.18, 1.0 + y, 0.12),
                vec3(0.06, 0.07, 0.05),
                leather,
            );
            let hilt = fr.limb(b, rhand, weapon_angle, 0.025, 0.1, wood);
            if look.seed.is_multiple_of(2) {
                fr.limb(
                    b,
                    hilt,
                    weapon_angle,
                    0.07,
                    0.6,
                    Color::new(0.35, 0.24, 0.14, 1.0),
                );
            } else {
                fr.limb(b, hilt, weapon_angle, 0.03, 0.45, steel);
            }
        }
        Gear::Mystic => {
            let pulse = 0.08 + (pose.time * 3.0).sin().abs() * 0.03;
            fr.glow(
                b,
                rhand + vec3(0.0, 0.05, 0.12),
                pulse,
                Color::new(0.75, 0.35, 1.0, 1.0),
            );
            fr.glow(
                b,
                lhand + vec3(0.0, 0.05, 0.12),
                pulse * 0.7,
                Color::new(0.75, 0.35, 1.0, 1.0),
            );
            fr.cube(
                b,
                vec3(0.0, 1.27 + y, 0.175),
                vec3(0.03, 0.27, 0.01),
                Color::new(0.8, 0.6, 0.2, 1.0),
            );
        }
        Gear::Golem => {
            fr.cube(b, lhand, vec3(0.16, 0.16, 0.16), legs);
            fr.cube(b, rhand, vec3(0.16, 0.16, 0.16), legs);
            for sx in [-1.0, 1.0] {
                fr.cube(
                    b,
                    vec3(sw * sx, shoulder_y + 0.08, 0.0),
                    vec3(0.16, 0.12, 0.16),
                    dark(legs, 0.9),
                );
            }
            let moss = Color::new(0.32, 0.45, 0.2, 1.0);
            fr.cube(
                b,
                vec3(-0.15, shoulder_y + 0.21, -0.05),
                vec3(0.12, 0.03, 0.1),
                moss,
            );
            fr.cube(b, vec3(0.0, 1.45 + y, -0.17), vec3(0.2, 0.15, 0.03), moss);
            let rune = Color::new(0.35, 0.9, 1.0, 0.6 + 0.4 * (pose.time * 2.0).sin().abs());
            fr.glow(b, vec3(0.0, 1.4 + y, 0.18), 0.07, rune);
            for (x, yy) in [(-0.15, 1.25), (0.15, 1.25), (0.0, 1.17)] {
                fr.cube(b, vec3(x, yy + y, 0.172), vec3(0.03, 0.03, 0.005), rune);
            }
        }
    }
    if look.gear[Slot::Chest.index()] == Some(items::GOLEMHEART_CHESTGUARD) {
        fr.glow(
            b,
            vec3(0.0, 1.42 + y, 0.18),
            0.06,
            Color::new(0.35, 0.9, 1.0, 1.0),
        );
        for sx in [-1.0, 1.0] {
            fr.ellipsoid(
                b,
                vec3(sw * sx, shoulder_y + 0.06, 0.0),
                vec3(0.15, 0.09, 0.15),
                Color::new(0.45, 0.47, 0.52, 1.0),
            );
        }
    }
}

fn wolf(b: &mut Batch, pos: Vec3, yaw: f32, seed: u32, pose: Pose) {
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, 1.0, true)
    } else {
        Frame::upright(pos, yaw, 1.0)
    };
    let tint = (seed % 3) as f32 * 0.04;
    let fur = Color::new(0.5 + tint, 0.48 + tint, 0.47, 1.0);
    let back = Color::new(0.33 + tint, 0.31 + tint, 0.31, 1.0);
    let belly = Color::new(0.72, 0.7, 0.66, 1.0);
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
    // Body: deep chest, slimmer hips, darker back.
    fr.cube(b, vec3(0.0, y, 0.22), vec3(0.23, 0.24, 0.32), fur);
    fr.cube(b, vec3(0.0, y + 0.02, -0.32), vec3(0.19, 0.2, 0.28), fur);
    fr.cube(b, vec3(0.0, y + 0.23, -0.05), vec3(0.17, 0.05, 0.55), back);
    fr.cube(b, vec3(0.0, y - 0.2, 0.15), vec3(0.17, 0.06, 0.3), belly);
    // Ruff around the neck.
    fr.cube(b, vec3(0.0, y + 0.08, 0.5), vec3(0.25, 0.25, 0.1), back);
    // Legs with paws.
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
    // Head.
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
    fr.sphere(
        b,
        head + vec3(0.0, -0.01, 0.38),
        0.04,
        Color::new(0.08, 0.07, 0.07, 1.0),
    );
    let tooth = Color::new(0.95, 0.94, 0.88, 1.0);
    for sx in [-1.0, 1.0] {
        fr.glow(
            b,
            head + vec3(0.08 * sx, 0.05, 0.16),
            0.032,
            Color::new(1.0, 0.82, 0.25, 1.0),
        );
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
            tooth,
        );
    }
    // Bushy tail.
    let wag = (pose.time * 4.0).sin() * 0.2;
    let t1 = fr.limb(b, vec3(0.0, y + 0.12, -0.58), 2.2 + wag, 0.07, 0.25, fur);
    let t2 = fr.limb(b, t1, 2.6 + wag, 0.09, 0.25, back);
    fr.sphere(b, t2, 0.07, belly);
}

fn boar(b: &mut Batch, pos: Vec3, yaw: f32, seed: u32, pose: Pose) {
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, 1.0, true)
    } else {
        Frame::upright(pos, yaw, 1.0)
    };
    let tint = (seed % 3) as f32 * 0.03;
    let hide = Color::new(0.43 + tint, 0.29, 0.19, 1.0);
    let dark_hide = Color::new(0.27, 0.18, 0.12, 1.0);
    let snout = Color::new(0.78, 0.52, 0.47, 1.0);
    let tusk = Color::new(0.96, 0.93, 0.83, 1.0);
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
    fr.ellipsoid(b, vec3(0.0, y, 0.0), vec3(0.34, 0.33, 0.62), hide);
    fr.ellipsoid(b, vec3(0.0, y + 0.05, 0.38), vec3(0.32, 0.33, 0.3), hide);
    // Bristly ridge along the back.
    for i in 0..6 {
        let z = 0.45 - i as f32 * 0.17;
        fr.cone_dir(
            b,
            vec3(0.0, y + 0.3 - (i as f32 - 2.0).abs() * 0.02, z),
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
        fr.cube(
            b,
            ankle,
            vec3(0.07, 0.04, 0.07),
            Color::new(0.15, 0.12, 0.1, 1.0),
        );
    }
    let head = vec3(0.0, y - 0.02, 0.72 + lunge);
    fr.cube(b, head, vec3(0.19, 0.18, 0.2), hide);
    fr.cylinder(b, head + vec3(0.0, -0.11, 0.21), 0.14, 0.09, snout);
    fr.sphere(
        b,
        head + vec3(-0.035, -0.04, 0.29),
        0.02,
        Color::new(0.2, 0.1, 0.1, 1.0),
    );
    fr.sphere(
        b,
        head + vec3(0.035, -0.04, 0.29),
        0.02,
        Color::new(0.2, 0.1, 0.1, 1.0),
    );
    for sx in [-1.0, 1.0] {
        fr.sphere(
            b,
            head + vec3(0.12 * sx, 0.07, 0.17),
            0.03,
            Color::new(0.08, 0.06, 0.05, 1.0),
        );
        fr.cone_dir(
            b,
            head + vec3(0.13 * sx, 0.15, -0.05),
            vec3(0.06 * sx, 0.12, -0.05),
            0.06,
            dark_hide,
        );
        let base = head + vec3(0.12 * sx, -0.12, 0.2);
        fr.cone_dir(b, base, vec3(0.05 * sx, 0.1, 0.1), 0.03, tusk);
    }
    // Curly tail.
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

// ---- Scenery ----

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
const PUMPKIN_FIELD: Vec2 = Vec2::new(38.0, -30.0);
const WHEAT_FIELD: Vec2 = Vec2::new(-36.0, -32.0);

const AUTUMN_LEAVES: [Color; 5] = [
    Color::new(0.9, 0.46, 0.12, 1.0),
    Color::new(0.75, 0.2, 0.1, 1.0),
    Color::new(0.93, 0.72, 0.2, 1.0),
    Color::new(0.82, 0.34, 0.1, 1.0),
    Color::new(0.6, 0.3, 0.12, 1.0),
];

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

/// Everything that never moves, prebuilt into meshes, plus the fog material.
pub struct Scene {
    meshes: Vec<Mesh>,
    water: Vec<Mesh>,
    chimneys: Vec<Vec3>,
    lamps: Vec<Vec3>,
    fires: Vec<Vec3>,
    fog: Option<Material>,
}

impl Scene {
    pub fn new() -> Self {
        let mut b = Batch::recording();
        let mut chimneys = Vec::new();
        let mut lamps = Vec::new();
        let mut fires = Vec::new();
        terrain(&mut b);
        grass_and_leaves(&mut b);
        trees_and_rocks(&mut b);
        farms(&mut b);
        town(&mut b, &mut chimneys, &mut lamps);
        camps(&mut b, &mut fires);
        let meshes = b.finish();

        let mut w = Batch::recording();
        let s = WORLD_HALF_SIZE;
        let c = glow(Color::new(0.32, 0.36, 0.55, 0.78));
        let base = w.reserve(4, 6);
        for (x, z) in [(-s, -s), (s, -s), (s, s), (-s, s)] {
            w.vertex(vec3(x, WATER_LEVEL, z), c);
        }
        w.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        let water = w.finish();

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
            meshes,
            water,
            chimneys,
            lamps,
            fires,
            fog,
        }
    }

    /// Call after `set_camera` for a 3D pass: turns on the fog.
    pub fn begin_3d(&self) {
        if let Some(fog) = &self.fog {
            fog.set_uniform("FogColor", vec4(FOG.r, FOG.g, FOG.b, 1.0));
            fog.set_uniform("FogNear", 60.0f32);
            fog.set_uniform("FogFar", 260.0f32);
            gl_use_material(fog);
        }
    }

    pub fn end_3d(&self) {
        gl_use_default_material();
    }

    pub fn draw(&self) {
        for m in &self.meshes {
            draw_mesh(m);
        }
    }

    /// Smoke, flickering fires and lamp glows.
    pub fn draw_effects(&self, b: &mut Batch, time: f32) {
        for (i, c) in self.chimneys.iter().enumerate() {
            for k in 0..5 {
                let t = (time * 0.25 + k as f32 / 5.0 + i as f32 * 0.37) % 1.0;
                let p = *c + vec3((t * 7.0 + i as f32).sin() * 0.3 + t * 1.2, t * 4.5, t * 0.6);
                b.sphere(
                    p,
                    0.25 + t * 0.6,
                    Color::new(0.55, 0.52, 0.55, 0.55 * (1.0 - t)),
                );
            }
        }
        for l in &self.lamps {
            let flicker = 0.9 + (time * 9.0 + l.x).sin() * 0.05;
            b.glow_sphere(*l, 0.22 * flicker, Color::new(1.0, 0.85, 0.45, 1.0));
            b.glow_sphere(*l, 0.45 * flicker, Color::new(1.0, 0.7, 0.3, 0.25));
        }
        for f in &self.fires {
            for k in 0..4 {
                let t = (time * 1.5 + k as f32 * 0.25) % 1.0;
                let p = *f + vec3((time * 5.0 + k as f32).sin() * 0.08, 0.3 + t * 1.0, 0.0);
                let c = mix(
                    Color::new(1.0, 0.85, 0.3, 0.95),
                    Color::new(0.9, 0.25, 0.05, 0.0),
                    t,
                );
                b.glow_sphere(p, 0.3 * (1.0 - t * 0.6), c);
            }
        }
    }

    /// Draw last: it's see-through.
    pub fn draw_water(&self) {
        for m in &self.water {
            draw_mesh(m);
        }
    }
}

/// The dusk sky, drawn in screen space before the 3D pass.
pub fn draw_sky(cam: &Camera3D, project: impl Fn(Vec3) -> Option<Vec2>) {
    // Also clears the depth buffer for the 3D pass.
    clear_background(FOG);
    let (w, h) = (screen_width(), screen_height());
    let look = (cam.target - cam.position).normalize_or_zero();
    let flat = vec3(look.x, 0.0, look.z).normalize_or_zero();
    let horizon = project(cam.position + flat * 1000.0).map_or(h * 0.5, |p| p.y);
    let bands = 160;
    for i in 0..bands {
        let y0 = h * i as f32 / bands as f32;
        let y1 = h * (i + 1) as f32 / bands as f32;
        let t = ((y0 + y1) * 0.5 - horizon) / h; // negative above the horizon
        let c = if t < 0.0 {
            let up = (-t * 2.2).min(1.0);
            if up < 0.35 {
                mix(SKY_HORIZON, SKY_MID, up / 0.35)
            } else {
                mix(SKY_MID, SKY_TOP, (up - 0.35) / 0.65)
            }
        } else {
            mix(SKY_HORIZON, FOG, t * 3.0)
        };
        draw_rectangle(0.0, y0, w, y1 - y0 + 1.0, c);
    }
    // A few early stars high up.
    let mut rng = Scatter(77);
    for _ in 0..60 {
        let dir = vec3(
            rng.range(-1.0, 1.0),
            rng.range(0.35, 1.0),
            rng.range(-1.0, 1.0),
        )
        .normalize();
        if let Some(p) = project(cam.position + dir * 1000.0) {
            let alpha = ((horizon - p.y) / h).clamp(0.0, 0.6);
            draw_circle(p.x, p.y, 1.2, Color::new(1.0, 0.95, 0.9, alpha));
        }
    }
    // The setting sun.
    if let Some(p) = project(cam.position + sun_dir() * 1000.0) {
        for (r, a) in [(120.0, 0.08), (70.0, 0.15), (40.0, 0.35)] {
            draw_circle(p.x, p.y, r, Color::new(1.0, 0.7, 0.35, a));
        }
        draw_circle(p.x, p.y, 26.0, Color::new(1.0, 0.88, 0.6, 1.0));
    }
}

fn noise(x: f32, z: f32) -> f32 {
    ((x * 0.37).sin() * (z * 0.41).cos()
        + (x * 0.11 + z * 0.13).sin()
        + (x * 0.023 - z * 0.031).sin())
        / 3.0
}

fn terrain_color(x: f32, z: f32, h: f32, slope: f32) -> Color {
    let n = noise(x, z);
    let golden = Color::new(0.6 + n * 0.06, 0.52 + n * 0.05, 0.25, 1.0);
    let rust = Color::new(0.58, 0.38, 0.18, 1.0);
    let olive = Color::new(0.42, 0.45, 0.22, 1.0);
    let mut grass = mix(golden, rust, (n * 3.0 + 0.2).max(0.0));
    grass = mix(grass, olive, (-n * 3.0 - 0.3).max(0.0));
    let sand = Color::new(0.72, 0.62, 0.45, 1.0);
    let rock = Color::new(0.46, 0.42, 0.4, 1.0);
    let snow = Color::new(0.92, 0.9, 0.94, 1.0);
    let cobble = Color::new(0.56 + n * 0.05, 0.52 + n * 0.05, 0.48, 1.0);
    let mut c = mix(sand, grass, (h - WATER_LEVEL - 0.3) / 1.2);
    c = mix(c, rock, (slope - 0.45) * 3.0);
    c = mix(c, rock, (h - 12.0) / 3.0);
    c = mix(c, snow, (h - 18.0) / 2.0);
    let d = (x * x + z * z).sqrt();
    // Dirt roads out of town along the axes.
    let road = (1.0 - x.abs().min(z.abs()) / 3.0).clamp(0.0, 1.0)
        * (1.0 - (d - 70.0) / 20.0).clamp(0.0, 1.0);
    c = mix(c, Color::new(0.5, 0.4, 0.28, 1.0), road * 0.85);
    // Plowed fields.
    for f in [PUMPKIN_FIELD, WHEAT_FIELD] {
        let inside = (1.0 - (vec2(x, z).distance(f) - 11.0) / 2.0).clamp(0.0, 1.0);
        let rows = ((x - f.x) * 1.6).sin() * 0.5 + 0.5;
        c = mix(
            c,
            mix(
                Color::new(0.38, 0.27, 0.17, 1.0),
                Color::new(0.48, 0.35, 0.22, 1.0),
                rows,
            ),
            inside,
        );
    }
    mix(c, cobble, (TOWN_RADIUS - d) / 3.0)
}

fn terrain(b: &mut Batch) {
    let step = 2.0;
    let n = (WORLD_HALF_SIZE * 2.0 / step) as usize;
    let rows_per_strip = 50;
    let mut row = 0;
    while row < n {
        let rows = rows_per_strip.min(n - row);
        let base = b.reserve((rows + 1) * (n + 1), rows * n * 6);
        for j in row..=row + rows {
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
        let stride = n as u16 + 1;
        for j in 0..rows as u16 {
            for i in 0..n as u16 {
                let a = base + j * stride + i;
                let c = a + stride;
                b.indices.extend_from_slice(&[a, c, a + 1, a + 1, c, c + 1]);
            }
        }
        b.flush();
        row += rows;
    }
}

fn near_landmark(x: f32, z: f32) -> bool {
    let p = Vec2::new(x, z);
    let d = p.length();
    d < TOWN_RADIUS + 8.0
        || x.abs() < 5.0 && d < 90.0
        || z.abs() < 5.0 && d < 90.0
        || BANDIT_CAMPS.iter().any(|c| c.distance(p) < 14.0)
        || GOLEM_RUINS.distance(p) < 16.0
        || [PUMPKIN_FIELD, WHEAT_FIELD]
            .iter()
            .any(|f| f.distance(p) < 15.0)
}

/// Grass tufts and drifts of fallen leaves.
fn grass_and_leaves(b: &mut Batch) {
    let mut rng = Scatter(0xBEEF);
    for _ in 0..6000 {
        let x = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let z = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let kind = rng.next();
        let h = terrain_height(x, z);
        let d = (x * x + z * z).sqrt();
        if d < TOWN_RADIUS + 2.0 || !(WATER_LEVEL + 0.3..14.0).contains(&h) {
            continue;
        }
        let base = vec3(x, h, z);
        if kind < 0.65 {
            let shade_n = rng.range(-0.06, 0.06);
            let c = Color::new(0.66 + shade_n, 0.56 + shade_n, 0.28, 1.0);
            for k in 0..3 {
                let a = rng.range(0.0, std::f32::consts::TAU) + k as f32;
                let side = vec3(a.cos(), 0.0, a.sin()) * 0.12;
                let tip = base + vec3(a.cos() * 0.15, rng.range(0.35, 0.6), a.sin() * 0.15);
                b.triangle(base - side, base + side, tip, c);
            }
        } else {
            // A little drift of fallen leaves.
            for k in 0..5 {
                let c = AUTUMN_LEAVES[(rng.next() * 5.0) as usize % 5];
                let a = rng.range(0.0, std::f32::consts::TAU);
                let r = rng.range(0.15, 0.3);
                let o =
                    vec3((k as f32 * 2.4).cos(), 0.0, (k as f32 * 2.4).sin()) * rng.range(0.0, 0.8);
                let at = base + o;
                let at = vec3(at.x, terrain_height(at.x, at.z) + 0.04, at.z);
                let u = vec3(a.cos(), 0.0, a.sin()) * r;
                let v = vec3(-a.sin(), 0.0, a.cos()) * r * 0.6;
                b.quad([at - u, at - v, at + u, at + v], Vec3::Y, c);
            }
        }
    }
}

fn autumn_tree(b: &mut Batch, rng: &mut Scatter, p: Vec3, size: f32) {
    let trunk = Color::new(0.33, 0.22, 0.14, 1.0);
    let leaves = AUTUMN_LEAVES[(rng.next() * 5.0) as usize % 5];
    let leaves2 = AUTUMN_LEAVES[(rng.next() * 5.0) as usize % 5];
    b.cone(p, Vec3::Y * 2.4 * size, 0.3 * size, 0.18 * size, 7, trunk);
    // Branches.
    for k in 0..3 {
        let a = k as f32 * 2.1 + rng.next();
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
    // Fallen leaves around the trunk.
    for k in 0..6 {
        let a = k as f32 * 1.05 + rng.next();
        let r = rng.range(0.6, 2.2) * size;
        let c = p + vec3(a.cos() * r, 0.0, a.sin() * r);
        let c = vec3(c.x, terrain_height(c.x, c.z) + 0.05, c.z);
        let u = vec3(a.cos(), 0.0, a.sin()) * 0.25;
        let v = vec3(-a.sin(), 0.0, a.cos()) * 0.16;
        b.quad(
            [c - u, c - v, c + u, c + v],
            Vec3::Y,
            if k % 2 == 0 { leaves } else { leaves2 },
        );
    }
}

fn bare_tree(b: &mut Batch, rng: &mut Scatter, p: Vec3, size: f32) {
    let trunk = Color::new(0.3, 0.22, 0.17, 1.0);
    b.cone(p, Vec3::Y * 3.0 * size, 0.25 * size, 0.1 * size, 6, trunk);
    for k in 0..5 {
        let a = k as f32 * 1.3 + rng.next();
        let h = rng.range(1.4, 2.6) * size;
        let dir = vec3(a.cos(), rng.range(0.6, 1.2), a.sin()) * rng.range(0.8, 1.4) * size;
        b.cone(p + Vec3::Y * h, dir, 0.08 * size, 0.02 * size, 4, trunk);
    }
}

fn trees_and_rocks(b: &mut Batch) {
    let mut rng = Scatter(0x9E3779B9);
    let trunk = Color::new(0.36, 0.25, 0.16, 1.0);
    for _ in 0..1100 {
        let x = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let z = rng.range(-WORLD_HALF_SIZE + 5.0, WORLD_HALF_SIZE - 5.0);
        let kind = rng.next();
        let size = rng.range(0.8, 1.4);
        let shade_n = rng.range(-0.04, 0.04);
        let h = terrain_height(x, z);
        if near_landmark(x, z) || !(WATER_LEVEL + 0.4..=16.0).contains(&h) {
            continue;
        }
        // Clump trees into woods: skip most of them in open meadows.
        let forest = ((x * 0.02).sin() + (z * 0.025).cos()) * 0.5;
        if kind < 0.8 && forest < -0.1 && rng.next() < 0.8 {
            continue;
        }
        let p = vec3(x, h - 0.2, z);
        if kind < 0.28 {
            // Evergreen.
            b.cylinder(p, Vec3::Y * 1.6 * size, 0.22 * size, 6, trunk);
            let green = Color::new(0.14 + shade_n, 0.3 + shade_n, 0.2, 1.0);
            for (y, r, hgt) in [
                (1.0, 1.6, 2.2),
                (2.1, 1.3, 2.0),
                (3.2, 1.0, 1.8),
                (4.2, 0.7, 1.5),
            ] {
                b.cone(
                    p + Vec3::Y * y * size,
                    Vec3::Y * hgt * size,
                    r * size,
                    0.0,
                    8,
                    green,
                );
            }
        } else if kind < 0.7 {
            autumn_tree(b, &mut rng, p, size);
        } else if kind < 0.78 {
            bare_tree(b, &mut rng, p, size);
        } else if kind < 0.9 {
            // Mossy rocks.
            let yaw = rng.range(0.0, std::f32::consts::TAU);
            let grey = Color::new(0.5 + shade_n, 0.48 + shade_n, 0.46, 1.0);
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
            b.block(
                p + vec3(-0.2, 1.02, -0.1) * size,
                vec3(0.55, 0.05, 0.4) * size,
                yaw,
                Color::new(0.4, 0.45, 0.22, 1.0),
            );
        } else if kind < 0.96 {
            // Shrub with berries.
            let c = AUTUMN_LEAVES[(rng.next() * 5.0) as usize % 5];
            b.ellipsoid(
                p + Vec3::Y * 0.5 * size,
                vec3(0.8, 0.55, 0.8) * size,
                dark(c, 0.85),
            );
            for k in 0..4 {
                let a = k as f32 * 1.6;
                b.sphere(
                    p + vec3(a.cos() * 0.6, 0.75, a.sin() * 0.6) * size,
                    0.07 * size,
                    Color::new(0.75, 0.1, 0.12, 1.0),
                );
            }
        } else {
            // Mushrooms.
            for k in 0..3 {
                let o = vec3((k as f32 * 2.3).cos(), 0.0, (k as f32 * 2.3).sin()) * 0.35;
                let s = rng.range(0.6, 1.0);
                b.cylinder(
                    p + o,
                    Vec3::Y * 0.35 * s,
                    0.05,
                    6,
                    Color::new(0.9, 0.86, 0.78, 1.0),
                );
                b.ellipsoid(
                    p + o + Vec3::Y * 0.38 * s,
                    vec3(0.18, 0.08, 0.18) * s,
                    Color::new(0.75, 0.18, 0.1, 1.0),
                );
            }
        }
    }
}

fn fence(b: &mut Batch, from: Vec2, to: Vec2) {
    let wood = Color::new(0.45, 0.32, 0.2, 1.0);
    let len = from.distance(to);
    let posts = (len / 2.5).ceil() as usize;
    let yaw = (to.x - from.x).atan2(to.y - from.y);
    for i in 0..=posts {
        let p = from.lerp(to, i as f32 / posts as f32);
        let g = ground(p.x, p.y);
        b.block(g + Vec3::Y * 0.55, vec3(0.08, 0.6, 0.08), yaw, wood);
        if i < posts {
            let q = from.lerp(to, (i as f32 + 0.5) / posts as f32);
            let gq = ground(q.x, q.y);
            for y in [0.45, 0.85] {
                b.block(
                    gq + Vec3::Y * y,
                    vec3(0.04, 0.05, len / posts as f32 * 0.5),
                    yaw,
                    wood,
                );
            }
        }
    }
}

fn farms(b: &mut Batch) {
    let mut rng = Scatter(0xFA12);
    // Pumpkin patch.
    for i in 0..9 {
        for j in 0..9 {
            if rng.next() < 0.45 {
                continue;
            }
            let x = PUMPKIN_FIELD.x - 8.0 + i as f32 * 2.0 + rng.range(-0.3, 0.3);
            let z = PUMPKIN_FIELD.y - 8.0 + j as f32 * 2.0 + rng.range(-0.3, 0.3);
            let p = ground(x, z);
            let s = rng.range(0.7, 1.2);
            let orange = Color::new(0.95, 0.5 + rng.range(-0.05, 0.05), 0.1, 1.0);
            b.ellipsoid(p + Vec3::Y * 0.3 * s, vec3(0.45, 0.33, 0.45) * s, orange);
            b.cylinder(
                p + Vec3::Y * 0.6 * s,
                Vec3::Y * 0.18,
                0.05,
                5,
                Color::new(0.3, 0.4, 0.15, 1.0),
            );
            b.ellipsoid(
                p + vec3(0.4, 0.05, 0.2) * s,
                vec3(0.3, 0.05, 0.2),
                Color::new(0.3, 0.42, 0.17, 1.0),
            );
        }
    }
    // Wheat sheaves and hay bales.
    for i in 0..7 {
        for j in 0..6 {
            let x = WHEAT_FIELD.x - 8.0 + i as f32 * 2.6 + rng.range(-0.4, 0.4);
            let z = WHEAT_FIELD.y - 7.0 + j as f32 * 2.6 + rng.range(-0.4, 0.4);
            let p = ground(x, z);
            let wheat = Color::new(0.88, 0.72, 0.36, 1.0);
            if (i + j) % 4 == 0 {
                b.cylinder(
                    p + vec3(-0.6, 0.6, 0.0),
                    Vec3::X * 1.2,
                    0.6,
                    10,
                    Color::new(0.85, 0.7, 0.35, 1.0),
                );
            } else {
                b.cone(p, Vec3::Y * 1.3, 0.35, 0.15, 7, wheat);
                b.cone(
                    p + Vec3::Y * 1.3,
                    Vec3::Y * 0.4,
                    0.3,
                    0.0,
                    7,
                    dark(wheat, 0.9),
                );
            }
        }
    }
    // A scarecrow watching over each field.
    for f in [PUMPKIN_FIELD, WHEAT_FIELD] {
        let p = ground(f.x + 1.0, f.y + 1.0);
        let wood = Color::new(0.45, 0.32, 0.2, 1.0);
        b.cylinder(p, Vec3::Y * 2.6, 0.08, 6, wood);
        b.block(p + Vec3::Y * 1.9, vec3(0.9, 0.06, 0.06), 0.4, wood);
        b.block(
            p + Vec3::Y * 1.75,
            vec3(0.38, 0.4, 0.18),
            0.4,
            Color::new(0.45, 0.25, 0.4, 1.0),
        );
        b.sphere(p + Vec3::Y * 2.45, 0.28, Color::new(0.9, 0.78, 0.5, 1.0));
        b.cone(
            p + Vec3::Y * 2.6,
            Vec3::Y * 0.5,
            0.45,
            0.0,
            10,
            Color::new(0.72, 0.6, 0.3, 1.0),
        );
    }
    // Fences around both fields.
    for f in [PUMPKIN_FIELD, WHEAT_FIELD] {
        let r = 11.5;
        let corners = [vec2(-r, -r), vec2(r, -r), vec2(r, r), vec2(-r, r)];
        for k in 0..4 {
            // Leave a gate on the side facing town.
            let (a, c) = (f + corners[k], f + corners[(k + 1) % 4]);
            let towards_town = (a + c).length() < (f * 2.0).length();
            if towards_town {
                let mid = a.lerp(c, 0.5);
                fence(b, a, a.lerp(mid, 0.8));
                fence(b, c.lerp(mid, 0.8), c);
            } else {
                fence(b, a, c);
            }
        }
    }
}

fn house(
    b: &mut Batch,
    center: Vec3,
    yaw: f32,
    wall: Color,
    roof: Color,
    chimneys: &mut Vec<Vec3>,
    lamps: &mut Vec<Vec3>,
) {
    let half = vec3(3.0, 1.8, 2.4);
    let f = forward(yaw);
    let r = vec3(-f.z, 0.0, f.x);
    // Stone footing and plaster walls.
    b.block(
        center + Vec3::Y * 0.25,
        vec3(half.x + 0.15, 0.25, half.z + 0.15),
        yaw,
        Color::new(0.5, 0.48, 0.45, 1.0),
    );
    b.block(center + Vec3::Y * half.y, half, yaw, wall);
    // Timber framing.
    let beam = Color::new(0.32, 0.2, 0.11, 1.0);
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
    // Gabled roof running along the house's width.
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
        // Shingle rows.
        for k in 1..4 {
            let t = k as f32 / 4.0;
            let a = corner(-1.0, sz).lerp(ridge(-1.0), t) + n * 0.03;
            let c = corner(1.0, sz).lerp(ridge(1.0), t) + n * 0.03;
            let d = (ridge(-1.0) - corner(-1.0, sz)).normalize() * 0.06;
            b.quad([a, c, c + d, a + d], n, dark(roof, 0.8));
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
    // Chimney with smoke.
    let chim = center + r * (half.x - 0.8) - f * 0.6;
    b.block(
        chim + Vec3::Y * (base_y + 1.6),
        vec3(0.35, 1.6, 0.35),
        yaw,
        Color::new(0.48, 0.4, 0.36, 1.0),
    );
    chimneys.push(chim + Vec3::Y * (base_y + 3.3));
    // Door, lit windows with frames and shutters, and a lantern by the door.
    let front = center + f * (half.z + 0.02);
    b.block(
        front + Vec3::Y * 1.05,
        vec3(0.65, 1.05, 0.04),
        yaw,
        Color::new(0.36, 0.22, 0.12, 1.0),
    );
    b.block(
        front + Vec3::Y * 1.05 + r * 0.4 + f * 0.04,
        vec3(0.06, 0.06, 0.03),
        yaw,
        Color::new(0.8, 0.65, 0.25, 1.0),
    );
    let was = b.glowing;
    for sx in [-1.0, 1.0] {
        let wp = front + r * sx * 1.9 + Vec3::Y * 2.2;
        b.block(wp, vec3(0.5, 0.45, 0.04), yaw, beam);
        b.glowing = true;
        b.block(
            wp + f * 0.02,
            vec3(0.4, 0.36, 0.04),
            yaw,
            Color::new(1.0, 0.78, 0.4, 1.0),
        );
        b.glowing = was;
        b.block(wp + f * 0.05, vec3(0.03, 0.36, 0.03), yaw, beam);
        for so in [-1.0, 1.0] {
            b.block(
                wp + r * so * 0.62 + f * 0.04,
                vec3(0.13, 0.45, 0.03),
                yaw,
                Color::new(0.3, 0.45, 0.35, 1.0),
            );
        }
    }
    let lamp = front + r * 1.0 + Vec3::Y * 2.3 + f * 0.3;
    b.block(lamp + Vec3::Y * 0.3, vec3(0.03, 0.2, 0.2), yaw, beam);
    lamps.push(lamp);
}

fn market_stall(b: &mut Batch, p: Vec3, yaw: f32, awning: Color) {
    let wood = Color::new(0.45, 0.31, 0.19, 1.0);
    let f = forward(yaw);
    let r = vec3(-f.z, 0.0, f.x);
    b.block(p + Vec3::Y * 0.5, vec3(1.4, 0.5, 0.6), yaw, wood);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        b.block(
            p + r * sx * 1.35 + f * sz * 0.55 + Vec3::Y * 1.2,
            vec3(0.06, 1.2, 0.06),
            yaw,
            wood,
        );
    }
    // Striped awning.
    for k in 0..6 {
        let x = -1.5 + k as f32 * 0.5 + 0.25;
        let c = if k % 2 == 0 {
            awning
        } else {
            Color::new(0.92, 0.88, 0.8, 1.0)
        };
        let a = p + r * (x - 0.25) + Vec3::Y * 2.45 - f * 0.7;
        let bb = p + r * (x + 0.25) + Vec3::Y * 2.45 - f * 0.7;
        let cc = p + r * (x + 0.25) + Vec3::Y * 2.1 + f * 0.9;
        let d = p + r * (x - 0.25) + Vec3::Y * 2.1 + f * 0.9;
        b.quad([a, bb, cc, d], (Vec3::Y + f * 0.3).normalize(), c);
    }
    // Wares: pumpkins and apples.
    for k in 0..3 {
        b.sphere(
            p + r * (-0.8 + k as f32 * 0.8) + Vec3::Y * 1.15,
            0.22,
            Color::new(0.95, 0.5, 0.1, 1.0),
        );
    }
    for k in 0..5 {
        b.sphere(
            p + r * (-1.0 + k as f32 * 0.5) + f * 0.35 + Vec3::Y * 1.05,
            0.1,
            Color::new(0.8, 0.15, 0.1, 1.0),
        );
    }
}

fn town(b: &mut Batch, chimneys: &mut Vec<Vec3>, lamps: &mut Vec<Vec3>) {
    let walls = [
        Color::new(0.9, 0.84, 0.7, 1.0),
        Color::new(0.86, 0.78, 0.66, 1.0),
        Color::new(0.93, 0.88, 0.78, 1.0),
    ];
    let roofs = [
        Color::new(0.62, 0.22, 0.15, 1.0),
        Color::new(0.3, 0.33, 0.45, 1.0),
        Color::new(0.48, 0.3, 0.18, 1.0),
    ];
    for (i, deg) in [28.0f32, 62.0, 118.0, 152.0, 208.0, 242.0, 298.0, 332.0]
        .into_iter()
        .enumerate()
    {
        let a = deg.to_radians();
        let p = vec3(a.cos() * 20.0, 0.0, a.sin() * 20.0);
        let yaw = yaw_towards(p, Vec3::ZERO);
        house(b, p, yaw, walls[i % 3], roofs[i % 3], chimneys, lamps);
    }
    // Fountain in the square.
    let stone = Color::new(0.62, 0.6, 0.57, 1.0);
    b.cylinder(Vec3::ZERO, Vec3::Y * 0.7, 3.0, 20, stone);
    b.cylinder(Vec3::Y * 0.7, Vec3::Y * 0.12, 3.15, 20, dark(stone, 0.85));
    b.cylinder(
        Vec3::Y * 0.72,
        Vec3::Y * 0.02,
        2.7,
        20,
        Color::new(0.3, 0.42, 0.6, 1.0),
    );
    b.cylinder(Vec3::ZERO, Vec3::Y * 2.2, 0.35, 10, stone);
    b.cylinder(Vec3::Y * 2.2, Vec3::Y * 0.25, 1.0, 14, stone);
    b.sphere(Vec3::Y * 2.75, 0.35, Color::new(0.55, 0.7, 0.9, 1.0));
    // Market stalls on the square.
    market_stall(
        b,
        vec3(8.0, 0.0, 6.0),
        yaw_towards(vec3(8.0, 0.0, 6.0), Vec3::ZERO),
        Color::new(0.75, 0.25, 0.15, 1.0),
    );
    market_stall(
        b,
        vec3(-8.0, 0.0, 6.5),
        yaw_towards(vec3(-8.0, 0.0, 6.5), Vec3::ZERO),
        Color::new(0.85, 0.6, 0.15, 1.0),
    );
    // Barrels and crates.
    let wood = Color::new(0.45, 0.31, 0.19, 1.0);
    for (x, z) in [
        (11.0, 3.0),
        (11.6, 4.0),
        (-11.0, 4.5),
        (5.0, 12.0),
        (-5.0, 12.5),
    ] {
        b.cylinder(vec3(x, 0.0, z), Vec3::Y * 1.0, 0.4, 10, wood);
        b.cylinder(
            vec3(x, 0.3, z),
            Vec3::Y * 0.06,
            0.42,
            10,
            Color::new(0.3, 0.3, 0.32, 1.0),
        );
        b.cylinder(
            vec3(x, 0.75, z),
            Vec3::Y * 0.06,
            0.42,
            10,
            Color::new(0.3, 0.3, 0.32, 1.0),
        );
    }
    for (x, z, s) in [(12.0, 6.0, 0.5), (12.3, 6.1, 0.35), (-12.0, 2.8, 0.45)] {
        b.block(vec3(x, s, z), Vec3::splat(s), 0.3, wood);
    }
    // A small graveyard beside where players appear.
    let grave = Color::new(0.58, 0.58, 0.6, 1.0);
    for row in 0..2 {
        for i in 0..3 {
            let x = -9.0 - i as f32 * 2.2;
            let z = GRAVEYARD.y + 4.0 - row as f32 * 3.0;
            b.block(vec3(x, 0.6, z), vec3(0.12, 0.6, 0.45), 0.0, grave);
            b.block(vec3(x, 1.0, z), vec3(0.13, 0.08, 0.25), 0.0, grave);
            b.block(
                vec3(x + 0.9, 0.05, z),
                vec3(0.8, 0.05, 0.45),
                0.0,
                Color::new(0.35, 0.28, 0.2, 1.0),
            );
        }
    }
    // Lamp posts at the edge of town on each road.
    for (x, z) in [
        (4.0, 26.0),
        (-4.0, -26.0),
        (26.0, -4.0),
        (-26.0, 4.0),
        (4.0, -26.0),
        (-4.0, 26.0),
        (26.0, 4.0),
        (-26.0, -4.0),
    ] {
        b.cylinder(
            vec3(x, 0.0, z),
            Vec3::Y * 3.5,
            0.12,
            6,
            Color::new(0.2, 0.2, 0.22, 1.0),
        );
        b.block(
            vec3(x, 3.55, z),
            vec3(0.25, 0.05, 0.25),
            0.0,
            Color::new(0.2, 0.2, 0.22, 1.0),
        );
        lamps.push(vec3(x, 3.85, z));
    }
}

fn camps(b: &mut Batch, fires: &mut Vec<Vec3>) {
    let canvas = Color::new(0.58, 0.48, 0.33, 1.0);
    let wood = Color::new(0.4, 0.27, 0.16, 1.0);
    for c in BANDIT_CAMPS {
        for i in 0..4 {
            let a = i as f32 * 1.6 + 0.4;
            let x = c.x + a.cos() * 7.0;
            let z = c.y + a.sin() * 7.0;
            let p = ground(x, z) - Vec3::Y * 0.2;
            b.cone(p, Vec3::Y * 3.0, 2.2, 0.0, 6, canvas);
            b.cylinder(p, Vec3::Y * 3.6, 0.06, 5, wood);
        }
        // Palisade arc.
        for k in 0..14 {
            let a = k as f32 * 0.22 + 3.4;
            let p = ground(c.x + a.cos() * 12.0, c.y + a.sin() * 12.0) - Vec3::Y * 0.2;
            b.cone(p, Vec3::Y * 2.6, 0.22, 0.0, 5, wood);
        }
        let fire = ground(c.x, c.y);
        for i in 0..3 {
            let a = i as f32 * 2.1;
            b.block(fire + Vec3::Y * 0.15, vec3(0.9, 0.12, 0.12), a, wood);
        }
        for k in 0..8 {
            let a = k as f32 * 0.785;
            b.sphere(
                fire + vec3(a.cos() * 1.1, 0.1, a.sin() * 1.1),
                0.22,
                Color::new(0.45, 0.43, 0.42, 1.0),
            );
        }
        fires.push(fire);
    }
    let stone = Color::new(0.52, 0.52, 0.5, 1.0);
    for i in 0..9 {
        let a = i as f32 * std::f32::consts::TAU / 9.0;
        let p = ground(
            GOLEM_RUINS.x + a.cos() * 11.0,
            GOLEM_RUINS.y + a.sin() * 11.0,
        ) - Vec3::Y * 0.3;
        let height = [6.0, 2.5, 5.0, 1.5, 6.5, 3.5, 4.0, 2.0, 5.5][i];
        b.cylinder(p, Vec3::Y * height, 0.8, 10, stone);
        b.cylinder(
            p + Vec3::Y * height * 0.5,
            Vec3::Y * 0.3,
            0.85,
            10,
            Color::new(0.38, 0.48, 0.25, 1.0),
        );
        if height > 5.0 {
            b.block(
                p + Vec3::Y * (height + 0.25),
                vec3(1.1, 0.25, 1.1),
                a,
                stone,
            );
        }
    }
    let was = b.glowing;
    b.glowing = true;
    for k in 0..6 {
        let a = k as f32 * 1.047;
        let p = ground(GOLEM_RUINS.x + a.cos() * 5.0, GOLEM_RUINS.y + a.sin() * 5.0);
        b.block(
            p + Vec3::Y * 0.06,
            vec3(0.5, 0.04, 0.12),
            a,
            Color::new(0.35, 0.85, 1.0, 1.0),
        );
    }
    b.glowing = was;
}
