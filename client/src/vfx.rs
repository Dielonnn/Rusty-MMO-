//! Spell effects: missiles shaped by their school (fireballs trailing
//! flames, frost shards, shadow bolts, arrows, bullets...), impact bursts,
//! shockwaves, pillars of holy light, healing spirals, drain beams, melee
//! slashes, casting circles, and what auras look like on whoever has them
//! (shield bubbles, stun stars, roots, flames, frost...).
//!
//! The glowing parts are textured sprites, ribbons and ground decals cut
//! from one atlas (`client/assets/fx/spell_fx.png`), tinted with the
//! spell's school color and added onto the scene so they light up whatever
//! is behind them. Solid parts (arrows, ice shards, leaves, vines, dust)
//! still go through the ordinary batch.

use std::cell::OnceCell;
use std::f32::consts::TAU;

use macroquad::miniquad::{BlendFactor, BlendState, Equation};
use macroquad::prelude::*;
use shared::data::{
    Ability, AbilityId, AuraKind, Class, Effect, MELEE_RANGE, School, Targeting, ability,
};
use shared::protocol::{AuraView, EntityId};
use shared::world::*;

use crate::gfx::{Batch, mix, texture};
use crate::hud::school_color;

/// Where an entity is, for effects that follow it.
#[derive(Clone, Copy)]
pub struct Anchor {
    pub pos: Vec3,
    pub height: f32,
    pub yaw: f32,
}

/// The effect atlas, painted by `client/assets/fx/make_spell_fx.py`.
const ATLAS: &[u8] = include_bytes!("../assets/fx/spell_fx.png");
const ATLAS_COLS: usize = 4;
const ATLAS_ROWS: usize = 2;

/// The pictures in the atlas, in the order the script paints them.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Cell {
    /// A soft round glow.
    Glow,
    /// A four-pointed star with a bright core.
    Flare,
    /// A tongue of flame, tip up.
    Flame,
    /// A ragged puff of smoke or mist.
    Wisp,
    /// A bright ring.
    Ring,
    /// A circle of runes around a six-pointed star.
    Runes,
    /// A streak running left to right that tiles along its length.
    Band,
    /// A six-armed ice crystal.
    Shard,
}

impl Cell {
    #[cfg(test)]
    const ALL: [Cell; 8] = [
        Cell::Glow,
        Cell::Flare,
        Cell::Flame,
        Cell::Wisp,
        Cell::Ring,
        Cell::Runes,
        Cell::Band,
        Cell::Shard,
    ];

    /// The cell's top-left and bottom-right texture coordinates, pulled in
    /// by a texel so its neighbors never bleed in.
    fn uv(self) -> (Vec2, Vec2) {
        let i = self as usize;
        let (cols, rows) = (ATLAS_COLS as f32, ATLAS_ROWS as f32);
        let (col, row) = ((i % ATLAS_COLS) as f32, (i / ATLAS_COLS) as f32);
        let inset = vec2(1.0 / (cols * 128.0), 1.0 / (rows * 128.0));
        (
            vec2(col / cols, row / rows) + inset,
            vec2((col + 1.0) / cols, (row + 1.0) / rows) - inset,
        )
    }
}

const GLOW_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;

varying lowp vec4 color;
varying mediump vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

/// The atlas's alpha is how brightly a pixel glows: tinted by the vertex
/// color, going white-hot where it's brightest, and added onto the scene.
const GLOW_FRAGMENT: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying mediump vec2 uv;

uniform sampler2D Texture;

void main() {
    float g = texture2D(Texture, uv).a * color.a;
    vec3 rgb = color.rgb * g + vec3(g * g * g) * 0.55;
    gl_FragColor = vec4(rgb, 0.0);
}"#;

struct GlowPass {
    material: Material,
    atlas: Texture2D,
}

fn load_glow_pass() -> Option<GlowPass> {
    let image = Image::from_file_with_format(ATLAS, None).ok()?;
    let atlas = texture::atlas(
        image.width as usize,
        image.height as usize,
        image.bytes.as_chunks::<4>().0,
    );
    let material = load_material(
        ShaderSource::Glsl {
            vertex: GLOW_VERTEX,
            fragment: GLOW_FRAGMENT,
        },
        MaterialParams {
            pipeline_params: PipelineParams {
                // Seen through nothing solid, but never hiding anything.
                depth_write: false,
                depth_test: Comparison::LessOrEqual,
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::One,
                    BlendFactor::One,
                )),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .ok()?;
    Some(GlowPass { material, atlas })
}

/// Collects this frame's glowing sprites, ribbons and decals.
#[derive(Default)]
struct Glows {
    batch: Batch,
    camera: Vec3,
}

fn tint(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, c.a * a.clamp(0.0, 1.0))
}

impl Glows {
    /// A quad showing part of a cell: `frac` is each corner's place in it
    /// (0,0 top-left, 1,1 bottom-right).
    fn quad_frac(&mut self, cell: Cell, corners: [Vec3; 4], frac: [Vec2; 4], color: Color) {
        if color.a <= 0.004 {
            return;
        }
        let (a, b) = cell.uv();
        let uvs = frac.map(|f| a + (b - a) * f);
        self.batch.quad_uv(corners, uvs, Vec3::Y, color);
    }

    /// A quad showing a whole cell, corners from its bottom-left going
    /// anticlockwise.
    fn quad(&mut self, cell: Cell, corners: [Vec3; 4], color: Color) {
        let frac = [
            vec2(0.0, 1.0),
            vec2(1.0, 1.0),
            vec2(1.0, 0.0),
            vec2(0.0, 0.0),
        ];
        self.quad_frac(cell, corners, frac, color);
    }

    /// A sprite `size` across, facing the camera and turned by `spin`.
    fn sprite(&mut self, cell: Cell, at: Vec3, size: f32, spin: f32, color: Color) {
        if size <= 0.0 {
            return;
        }
        let to_camera = (self.camera - at).normalize_or(Vec3::Z);
        let right = Vec3::Y.cross(to_camera).normalize_or(Vec3::X);
        let up = to_camera.cross(right);
        let (s, c) = spin.sin_cos();
        let h = size * 0.5;
        let (r, u) = ((right * c + up * s) * h, (up * c - right * s) * h);
        self.quad(
            cell,
            [at - r - u, at + r - u, at + r + u, at - r + u],
            color,
        );
    }

    /// A ribbon from `a` to `b`, turned to face the camera, with the cell's
    /// left edge at `a`.
    fn streak(&mut self, cell: Cell, a: Vec3, b: Vec3, half_width: f32, color: Color) {
        let to_camera = self.camera - (a + b) * 0.5;
        let side = (b - a).cross(to_camera).normalize_or_zero() * half_width;
        if side == Vec3::ZERO {
            return;
        }
        self.quad(cell, [a - side, b - side, b + side, a + side], color);
    }

    /// A cell lying flat, `radius` out from `center` and turned by `spin`:
    /// draped over the ground, or level in the air.
    fn decal(
        &mut self,
        cell: Cell,
        center: Vec3,
        radius: f32,
        spin: f32,
        color: Color,
        ground: bool,
    ) {
        const RINGS: usize = 4;
        const SIDES: usize = 24;
        if color.a <= 0.004 || radius <= 0.0 {
            return;
        }
        let (a, b) = cell.uv();
        let half = (b - a) * 0.5;
        let mid = a + half;
        let place = |p: Vec3| {
            if ground {
                vec3(
                    p.x,
                    terrain_height(p.x, p.z).max(center.y - 0.5) + 0.09,
                    p.z,
                )
            } else {
                p
            }
        };
        let base = self
            .batch
            .begin_shape(1 + RINGS * SIDES, SIDES * 3 + (RINGS - 1) * SIDES * 6);
        self.batch.vertex_uv(place(center), mid, Vec3::Y, color);
        for ring in 1..=RINGS {
            let k = ring as f32 / RINGS as f32;
            for side in 0..SIDES {
                let ang = TAU * side as f32 / SIDES as f32;
                let p = center + vec3(ang.cos(), 0.0, ang.sin()) * radius * k;
                let (ts, tc) = (ang + spin).sin_cos();
                let uv = mid + vec2(tc, ts) * half * k;
                self.batch.vertex_uv(place(p), uv, Vec3::Y, color);
            }
        }
        let at = |ring: usize, side: usize| base + 1 + (ring * SIDES + side % SIDES) as u16;
        for side in 0..SIDES {
            self.batch.index(&[base, at(0, side), at(0, side + 1)]);
        }
        for ring in 0..RINGS - 1 {
            for side in 0..SIDES {
                let (a, b) = (at(ring, side), at(ring, side + 1));
                let (c, d) = (at(ring + 1, side), at(ring + 1, side + 1));
                self.batch.index(&[a, c, b, b, c, d]);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Missile {
    Orb,
    Fire,
    Frost,
    Shadow,
    Arcane,
    Holy,
    Nature,
    Arrow,
    Bullet,
}

#[derive(Clone, Copy)]
enum Kind {
    Missile {
        from: Vec3,
        to: EntityId,
        style: Missile,
        school: School,
        speed: f32,
    },
    /// Sparks flying out from a point (or an entity's chest).
    Burst { size: f32, sparks: u32 },
    /// A shockwave on the ground.
    Shockwave { radius: f32 },
    /// A column of light from the sky.
    Pillar,
    /// Motes spiralling up around someone.
    Spiral,
    /// A beam flowing from one entity to another.
    Beam { to: EntityId },
    /// A swipe in front of an attacker.
    Slash { flip: bool },
    /// Dust kicked up by a charge or a leap.
    Dust,
}

#[derive(Clone, Copy)]
struct Fx {
    kind: Kind,
    /// Who it's on (or comes from).
    on: Option<EntityId>,
    /// Where it is when it isn't on anyone.
    at: Vec3,
    color: Color,
    age: f32,
    duration: f32,
    seed: f32,
}

#[derive(Default)]
pub struct Vfx {
    fx: Vec<Fx>,
    seed: f32,
    glows: Glows,
    /// The glow material and atlas, made on the first frame (they need
    /// the graphics context); `None` if the shader wouldn't compile.
    pass: OnceCell<Option<GlowPass>>,
}

const MISSILE_SPEED: f32 = 32.0;

fn missile_for(a: &Ability, class: Option<Class>) -> Missile {
    match a.school {
        School::Fire => Missile::Fire,
        School::Frost => Missile::Frost,
        School::Shadow => Missile::Shadow,
        School::Arcane => Missile::Arcane,
        School::Holy => Missile::Holy,
        School::Nature => Missile::Nature,
        School::Physical => match class {
            Some(Class::Artificer) => Missile::Bullet,
            Some(Class::Ranger) | None => Missile::Arrow,
            _ => Missile::Orb,
        },
    }
}

impl Vfx {
    fn push(&mut self, kind: Kind, on: Option<EntityId>, at: Vec3, color: Color, duration: f32) {
        self.seed += 1.37;
        self.fx.push(Fx {
            kind,
            on,
            at,
            color,
            age: 0.0,
            duration,
            seed: self.seed,
        });
        if self.fx.len() > 300 {
            self.fx.remove(0);
        }
    }

    /// Someone used an ability: launch its missile, swing its slash, ring
    /// its shockwave, and so on.
    pub fn ability_used(
        &mut self,
        id: AbilityId,
        caster: EntityId,
        target: Option<EntityId>,
        class: Option<Class>,
        anchor: impl Fn(EntityId) -> Option<Anchor>,
    ) {
        let a = ability(id);
        let color = school_color(a.school);
        let Some(me) = anchor(caster) else { return };
        if a.projectile
            && let Some(to) = target
        {
            let style = missile_for(a, class);
            let speed = match style {
                Missile::Arrow | Missile::Bullet => MISSILE_SPEED * 1.6,
                _ => MISSILE_SPEED,
            };
            let hand = me.pos + Vec3::Y * me.height * 0.7 + forward(me.yaw) * 0.5;
            self.push(
                Kind::Missile {
                    from: hand,
                    to,
                    style,
                    school: a.school,
                    speed,
                },
                Some(caster),
                hand,
                color,
                2.0,
            );
        }
        let melee = a.range <= MELEE_RANGE && !a.projectile && a.targeting.needs_enemy();
        if melee {
            self.push(
                Kind::Slash {
                    flip: (self.seed as u32).is_multiple_of(2),
                },
                Some(caster),
                me.pos,
                color,
                0.3,
            );
        }
        match a.targeting {
            Targeting::AroundCaster(radius) => {
                self.push(Kind::Shockwave { radius }, Some(caster), me.pos, color, 0.6);
                self.push(
                    Kind::Burst {
                        size: 1.0,
                        sparks: 16,
                    },
                    Some(caster),
                    me.pos,
                    color,
                    0.5,
                );
            }
            Targeting::AroundTarget(radius) => {
                if let Some(t) = target {
                    self.push(Kind::Shockwave { radius }, Some(t), me.pos, color, 0.6);
                }
            }
            _ => {}
        }
        for effect in a.effects {
            let on = target.unwrap_or(caster);
            match effect {
                Effect::Heal { .. } => {
                    if a.cast_time > 0.0 {
                        self.push(Kind::Pillar, Some(on), me.pos, color, 1.0);
                    }
                    self.push(Kind::Spiral, Some(on), me.pos, color, 1.0);
                }
                Effect::Aura { kind, .. } if !kind.harmful() => {
                    self.push(Kind::Spiral, Some(on), me.pos, color, 0.9);
                }
                Effect::Drain { .. } => {
                    if let Some(t) = target {
                        self.push(
                            Kind::Beam { to: caster },
                            Some(t),
                            me.pos,
                            Color::new(0.5, 1.0, 0.4, 1.0),
                            0.9,
                        );
                    }
                }
                Effect::Charge | Effect::Leap(_) => {
                    self.push(
                        Kind::Dust,
                        None,
                        me.pos,
                        Color::new(0.75, 0.68, 0.55, 1.0),
                        0.8,
                    );
                }
                Effect::RestorePower(_) => {
                    self.push(
                        Kind::Spiral,
                        Some(caster),
                        me.pos,
                        Color::new(0.4, 0.6, 1.0, 1.0),
                        1.2,
                    );
                }
                _ => {}
            }
        }
    }

    /// Something was hit by an ability: a burst where it landed.
    pub fn hit(&mut self, target: EntityId, id: Option<AbilityId>, crit: bool) {
        let Some(id) = id else { return };
        let a = ability(id);
        // Missiles burst when they arrive instead, and damage over time
        // ticks show on the target's aura.
        let direct = a.effects.iter().any(|e| {
            matches!(
                e,
                Effect::Damage { .. } | Effect::Drain { .. } | Effect::Finisher { .. }
            )
        });
        if a.projectile || !direct {
            return;
        }
        let size = if crit { 1.4 } else { 0.9 };
        self.push(
            Kind::Burst {
                size,
                sparks: if crit { 18 } else { 10 },
            },
            Some(target),
            Vec3::ZERO,
            school_color(a.school),
            0.45,
        );
    }

    pub fn level_up(&mut self, id: EntityId) {
        let gold = Color::new(1.0, 0.85, 0.3, 1.0);
        self.push(Kind::Pillar, Some(id), Vec3::ZERO, gold, 1.6);
        self.push(Kind::Spiral, Some(id), Vec3::ZERO, gold, 1.6);
        self.push(
            Kind::Shockwave { radius: 4.0 },
            Some(id),
            Vec3::ZERO,
            gold,
            0.8,
        );
    }

    /// Ages everything; missiles that arrive burst on their target.
    pub fn update(&mut self, dt: f32, anchor: impl Fn(EntityId) -> Option<Anchor>) {
        let mut bursts = Vec::new();
        for f in &mut self.fx {
            f.age += dt;
            if let Kind::Missile {
                from,
                to,
                speed,
                school,
                ..
            } = f.kind
            {
                match anchor(to) {
                    Some(t) => {
                        let end = t.pos + Vec3::Y * t.height * 0.6;
                        if from.distance(end) <= f.age * speed {
                            f.age = f.duration;
                            bursts.push((to, school));
                        }
                    }
                    None => f.age = f.duration,
                }
            }
        }
        self.fx.retain(|f| f.age < f.duration);
        for (to, school) in bursts {
            let color = school_color(school);
            self.push(
                Kind::Burst {
                    size: 1.1,
                    sparks: 14,
                },
                Some(to),
                Vec3::ZERO,
                color,
                0.5,
            );
        }
    }

    /// Call at the start of a frame's 3D pass, before anything is drawn,
    /// with where the camera is (glowing sprites turn to face it).
    pub fn begin(&mut self, camera: Vec3) {
        self.glows.camera = camera;
        if let Some(pass) = self.pass.get_or_init(load_glow_pass) {
            self.glows.batch.set_texture(Some(pass.atlas.clone()));
        }
    }

    /// Draws this frame's glows on top of the scene: call after everything
    /// solid and see-through (water) has been drawn. It leaves the glow
    /// material on, so end the 3D pass afterwards.
    pub fn draw_glows(&mut self) {
        if let Some(Some(pass)) = self.pass.get() {
            gl_use_material(&pass.material);
        }
        self.glows.batch.flush();
    }

    pub fn draw(&mut self, b: &mut Batch, time: f32, anchor: impl Fn(EntityId) -> Option<Anchor>) {
        let g = &mut self.glows;
        for f in &self.fx {
            let t = (f.age / f.duration).clamp(0.0, 1.0);
            let on = f.on.and_then(&anchor);
            let base = on.map_or(f.at, |a| a.pos);
            match f.kind {
                Kind::Missile {
                    from,
                    to,
                    style,
                    speed,
                    ..
                } => {
                    let Some(target) = anchor(to) else { continue };
                    let end = target.pos + Vec3::Y * target.height * 0.6;
                    let dist = from.distance(end).max(0.1);
                    let k = (f.age * speed / dist).min(1.0);
                    let at = from.lerp(end, k);
                    let dir = (end - from).normalize_or_zero();
                    missile(b, g, style, at, dir, f.color, time, f.seed);
                }
                Kind::Burst { size, sparks } => {
                    let center = on.map_or(f.at, |a| a.pos + Vec3::Y * a.height * 0.55);
                    let fade = 1.0 - t;
                    // A star-shaped flash and a puff, then sparks streaking
                    // out and falling.
                    g.sprite(
                        Cell::Flare,
                        center,
                        size * (1.2 + 1.6 * t.sqrt()),
                        f.seed,
                        tint(f.color, fade * fade * 1.2),
                    );
                    g.sprite(
                        Cell::Glow,
                        center,
                        size * 2.0 * (1.0 - t * 0.4),
                        0.0,
                        tint(f.color, fade),
                    );
                    g.sprite(
                        Cell::Wisp,
                        center,
                        size * (1.0 + t * 1.6),
                        f.seed + t,
                        tint(f.color, 0.55 * fade),
                    );
                    for i in 0..sparks {
                        let a = i as f32 * 2.399 + f.seed;
                        let up = ((i * 7) % 5) as f32 / 5.0 - 0.3;
                        let d = vec3(a.cos(), up, a.sin()).normalize();
                        let p = center + d * size * 1.6 * t.sqrt() - Vec3::Y * t * t * 0.8;
                        let tail = (d * (1.0 - t) - Vec3::Y * t).normalize_or(d) * size * 0.35;
                        g.streak(
                            Cell::Glow,
                            p - tail,
                            p,
                            0.06 * size,
                            tint(mix(f.color, WHITE, 0.35), fade),
                        );
                    }
                }
                Kind::Shockwave { radius } => {
                    let r = (radius * t.sqrt()).max(0.1);
                    let fade = 1.0 - t;
                    g.decal(Cell::Ring, base, r * 1.1, f.seed, tint(f.color, fade), true);
                    g.decal(
                        Cell::Glow,
                        base,
                        r * 1.2,
                        0.0,
                        tint(f.color, 0.35 * fade),
                        true,
                    );
                    // Glints thrown up around the edge.
                    for i in 0..12 {
                        let a = i as f32 * std::f32::consts::FRAC_PI_6 + f.seed;
                        let p = base + vec3(a.cos() * r, 0.3 + (t * 3.0).sin() * 0.6, a.sin() * r);
                        g.sprite(Cell::Flare, p, 0.45 * fade, a, tint(f.color, fade));
                    }
                }
                Kind::Pillar => {
                    let k = if t < 0.2 {
                        t / 0.2
                    } else {
                        1.0 - (t - 0.2) / 0.8
                    };
                    let top = base + Vec3::Y * 9.0;
                    g.streak(Cell::Band, base, top, 1.0 * k, tint(f.color, 0.5 * k));
                    g.streak(Cell::Band, base, top, 0.35 * k, tint(WHITE, 0.3 * k));
                    g.sprite(
                        Cell::Glow,
                        base + Vec3::Y * 0.8,
                        2.6,
                        0.0,
                        tint(f.color, 0.6 * k),
                    );
                    g.decal(Cell::Glow, base, 1.8, 0.0, tint(f.color, 0.5 * k), true);
                    g.decal(Cell::Ring, base, 1.5, time, tint(f.color, k), true);
                    // Motes drifting down the column.
                    for i in 0..6 {
                        let s = (1.0 - (time * 0.7 + i as f32 / 6.0) % 1.0) * 8.0;
                        let a = i as f32 * 1.05 + f.seed;
                        let p = base + vec3(a.cos() * 0.4, s, a.sin() * 0.4);
                        g.sprite(Cell::Flare, p, 0.35, time + a, tint(WHITE, 0.8 * k));
                    }
                }
                Kind::Spiral => {
                    let h = on.map_or(2.0, |a| a.height);
                    for i in 0..10 {
                        let k = (t + i as f32 * 0.1) % 1.0;
                        let a = k * 9.0 + i as f32 * 0.63 + f.seed;
                        let p = base + vec3(a.cos() * 0.75, k * h * 1.2, a.sin() * 0.75);
                        let fade = (1.0 - t) * (1.0 - k);
                        g.sprite(Cell::Glow, p, 0.4, 0.0, tint(f.color, fade));
                        g.sprite(
                            Cell::Flare,
                            p,
                            0.3 * (1.0 - k * 0.5),
                            a,
                            tint(f.color, fade),
                        );
                    }
                    g.decal(
                        Cell::Ring,
                        base + Vec3::Y * (0.1 + t * h),
                        1.05,
                        time,
                        tint(f.color, 0.9 * (1.0 - t)),
                        false,
                    );
                }
                Kind::Beam { to } => {
                    let (Some(from), Some(dest)) = (on, anchor(to)) else {
                        continue;
                    };
                    let a = from.pos + Vec3::Y * from.height * 0.55;
                    let e = dest.pos + Vec3::Y * dest.height * 0.6;
                    let k = 1.0 - t;
                    // A wavering stream with motes flowing along it.
                    let side = (e - a).cross(Vec3::Y).normalize_or_zero();
                    let mut prev = a;
                    for i in 1..=8 {
                        let s = i as f32 / 8.0;
                        let wobble =
                            (s * 9.0 + time * 12.0).sin() * 0.15 * (1.0 - (s - 0.5).abs() * 2.0);
                        let p = a.lerp(e, s) + side * wobble;
                        g.streak(Cell::Band, prev, p, 0.2 * k + 0.06, tint(f.color, k));
                        prev = p;
                    }
                    for i in 0..5 {
                        let s = (time * 1.8 + i as f32 * 0.2) % 1.0;
                        g.sprite(Cell::Glow, a.lerp(e, s), 0.45, 0.0, tint(f.color, k));
                    }
                    g.sprite(Cell::Glow, a, 0.9, 0.0, tint(f.color, 0.8 * k));
                    g.sprite(Cell::Flare, e, 0.8, time * 2.0, tint(f.color, k));
                }
                Kind::Slash { flip } => {
                    let Some(me) = on else { continue };
                    // A streak of light sweeping across in front, brightest
                    // at its leading edge.
                    let fwd = forward(me.yaw);
                    let side = vec3(-fwd.z, 0.0, fwd.x) * if flip { -1.0 } else { 1.0 };
                    let center = me.pos + Vec3::Y * me.height * 0.55;
                    let sweep = t * 2.6 - 1.3;
                    let segments = 8;
                    let dir =
                        |a: f32| (fwd * a.cos() + side * a.sin() + Vec3::Y * a * 0.25).normalize();
                    for i in 0..segments {
                        let a0 = sweep - 0.9 + i as f32 * 0.15;
                        let (d0, d1) = (dir(a0), dir(a0 + 0.15));
                        let (u0, u1) =
                            (i as f32 / segments as f32, (i + 1) as f32 / segments as f32);
                        let c = tint(mix(f.color, WHITE, 0.4), u1 * (1.0 - t) * 1.3);
                        g.quad_frac(
                            Cell::Band,
                            [
                                center + d0 * 0.6,
                                center + d1 * 0.6,
                                center + d1 * 1.7,
                                center + d0 * 1.7,
                            ],
                            [vec2(u0, 1.0), vec2(u1, 1.0), vec2(u1, 0.0), vec2(u0, 0.0)],
                            c,
                        );
                    }
                }
                Kind::Dust => {
                    for i in 0..10 {
                        let a = i as f32 * 0.628 + f.seed;
                        let p = f.at + vec3(a.cos() * t * 1.8, 0.2 + t * 0.8, a.sin() * t * 1.8);
                        b.sphere(p, 0.25 + t * 0.4, tint(f.color, 0.5 * (1.0 - t)));
                    }
                }
            }
        }
    }

    /// What lingering auras look like on someone: shields, stuns, roots,
    /// burning, poison, frost, healing, buffs and curses.
    pub fn draw_auras(&mut self, b: &mut Batch, a: Anchor, auras: &[AuraView], time: f32) {
        let g = &mut self.glows;
        let h = a.height;
        for (i, aura) in auras.iter().enumerate() {
            let ab = ability(aura.ability);
            let color = school_color(ab.school);
            let Some(kind) = ab.effects.iter().find_map(|e| match e {
                Effect::Aura { kind, .. } => Some(*kind),
                _ => None,
            }) else {
                continue;
            };
            let phase = time + i as f32;
            match kind {
                AuraKind::Absorb(_) => {
                    let pulse = 0.04 * (phase * 3.0).sin();
                    b.lit(|b| {
                        b.ellipsoid(
                            a.pos + Vec3::Y * h * 0.5,
                            vec3(0.95, h * 0.6, 0.95) * (1.0 + pulse),
                            tint(color, 0.12),
                        )
                    });
                    g.decal(
                        Cell::Ring,
                        a.pos + Vec3::Y * (h * 0.5 + (phase * 2.0).sin() * h * 0.4),
                        1.1,
                        phase,
                        tint(color, 0.7),
                        false,
                    );
                    // Glints wandering over the bubble.
                    for k in 0..4 {
                        let ang = phase * 0.9 + k as f32 * 1.57;
                        let y = (phase * 0.7 + k as f32 * 0.8).sin() * 0.8;
                        let p = a.pos
                            + Vec3::Y * h * 0.5
                            + vec3(ang.cos() * 0.95, y * h * 0.6, ang.sin() * 0.95)
                                * (1.0 - y * y * 0.5).sqrt();
                        g.sprite(Cell::Flare, p, 0.35, phase + k as f32, tint(WHITE, 0.6));
                    }
                }
                AuraKind::Stun => {
                    for k in 0..3 {
                        let ang = phase * 4.0 + k as f32 * 2.09;
                        let p = a.pos + vec3(ang.cos() * 0.35, h + 0.25, ang.sin() * 0.35);
                        let yellow = Color::new(1.0, 0.92, 0.3, 1.0);
                        g.sprite(Cell::Flare, p, 0.4, phase * 3.0, yellow);
                        g.sprite(Cell::Glow, p, 0.3, 0.0, tint(yellow, 0.7));
                    }
                }
                AuraKind::Root => {
                    for k in 0..6 {
                        let ang = k as f32 * 1.047 + 0.3;
                        let base = a.pos + vec3(ang.cos() * 0.45, 0.0, ang.sin() * 0.45);
                        let tip = a.pos
                            + vec3(
                                ang.cos() * 0.15,
                                0.7 + (k % 2) as f32 * 0.3,
                                ang.sin() * 0.15,
                            );
                        b.beam(base, tip, 0.04, Vec3::Y, Color::new(0.3, 0.5, 0.2, 1.0));
                        b.sphere(tip, 0.06, Color::new(0.4, 0.75, 0.3, 1.0));
                    }
                    g.decal(Cell::Wisp, a.pos, 0.9, 0.0, tint(color, 0.45), true);
                }
                AuraKind::Slow(_) => {
                    let ice = Color::new(0.7, 0.9, 1.0, 1.0);
                    for k in 0..5 {
                        let ang = k as f32 * 1.257 + phase * 0.5;
                        let p = a.pos + vec3(ang.cos() * 0.45, 0.05, ang.sin() * 0.45);
                        b.lit(|b| b.cone(p, vec3(0.0, 0.35, 0.0), 0.08, 0.0, 4, tint(ice, 0.85)));
                    }
                    // Frost spreading underfoot, and flakes drifting down.
                    g.decal(Cell::Shard, a.pos, 0.9, 0.4, tint(ice, 0.6), true);
                    for k in 0..3 {
                        let t = (phase * 0.4 + k as f32 / 3.0) % 1.0;
                        let ang = k as f32 * 2.09 + phase * 0.3;
                        let p = a.pos + vec3(ang.cos() * 0.5, h * (1.0 - t), ang.sin() * 0.5);
                        g.sprite(Cell::Shard, p, 0.22, phase + k as f32, tint(ice, 1.0 - t));
                    }
                }
                AuraKind::Dot { .. } => {
                    // Burning, poisoned, bleeding or shadow-eaten.
                    for k in 0..4 {
                        let t = (phase * 1.3 + k as f32 * 0.25) % 1.0;
                        let ang = k as f32 * 1.57 + phase;
                        let p = a.pos + vec3(ang.cos() * 0.3, h * (0.4 + t * 0.6), ang.sin() * 0.3);
                        let fade = 1.0 - t;
                        match ab.school {
                            School::Fire => {
                                let c = Color::new(1.0, 0.55 - t * 0.3, 0.12, 1.0);
                                g.sprite(
                                    Cell::Flame,
                                    p,
                                    0.55 * (1.0 - t * 0.4),
                                    0.0,
                                    tint(c, fade),
                                );
                            }
                            School::Physical => {
                                // Blood drips fall, and don't glow.
                                let p = a.pos
                                    + vec3(ang.cos() * 0.25, h * (0.7 - t * 0.6), ang.sin() * 0.25);
                                b.sphere(
                                    p,
                                    0.06 * (1.0 - t * 0.5),
                                    Color::new(0.6, 0.05, 0.05, 0.9 * fade),
                                );
                            }
                            School::Nature => {
                                let c = Color::new(0.5, 1.0, 0.3, 1.0);
                                g.sprite(Cell::Wisp, p, 0.4, phase + k as f32, tint(c, 0.8 * fade));
                            }
                            _ => {
                                let c = Color::new(color.r * 0.7, color.g * 0.5, color.b, 1.0);
                                g.sprite(
                                    Cell::Wisp,
                                    p,
                                    0.45,
                                    phase + k as f32,
                                    tint(c, 0.8 * fade),
                                );
                            }
                        }
                    }
                }
                AuraKind::Hot { .. } => {
                    let green = Color::new(0.5, 1.0, 0.45, 1.0);
                    for k in 0..4 {
                        let t = (phase * 0.8 + k as f32 * 0.25) % 1.0;
                        let ang = k as f32 * 1.57 + t * 3.0;
                        let p = a.pos + vec3(ang.cos() * 0.5, t * h, ang.sin() * 0.5);
                        g.sprite(Cell::Glow, p, 0.35, 0.0, tint(green, 0.9 * (1.0 - t)));
                        g.sprite(Cell::Flare, p, 0.25, ang, tint(green, 1.0 - t));
                    }
                }
                AuraKind::Speed(_) => {
                    for k in 0..3 {
                        let t = (phase * 3.0 + k as f32 * 0.33) % 1.0;
                        let back = -forward(a.yaw);
                        let side = vec3(back.z, 0.0, -back.x) * (k as f32 - 1.0) * 0.3;
                        let p = a.pos + Vec3::Y * (0.3 + k as f32 * 0.4) + side + back * t;
                        g.streak(
                            Cell::Band,
                            p,
                            p + back * 0.7,
                            0.08,
                            Color::new(0.9, 0.95, 1.0, 0.6 * (1.0 - t)),
                        );
                    }
                }
                AuraKind::DamageTaken(f) if f < 1.0 => {
                    // Toughened: a golden shimmer.
                    let gold = Color::new(1.0, 0.85, 0.35, 1.0);
                    g.decal(Cell::Ring, a.pos, 1.05, phase * 0.3, tint(gold, 0.8), true);
                    let y = (phase * 1.5).sin() * 0.5 + 0.5;
                    g.decal(
                        Cell::Ring,
                        a.pos + Vec3::Y * h * y,
                        0.9,
                        phase,
                        tint(gold, 0.6),
                        false,
                    );
                }
                AuraKind::DamageDone(f) if f > 1.0 => {
                    // Empowered: red flames licking up from the feet.
                    for k in 0..6 {
                        let t = (phase * 1.5 + k as f32 / 6.0) % 1.0;
                        let ang = k as f32 * 1.047;
                        g.sprite(
                            Cell::Flame,
                            a.pos + vec3(ang.cos() * 0.5, 0.2 + t * 1.0, ang.sin() * 0.5),
                            0.4 * (1.0 - t * 0.5),
                            0.0,
                            Color::new(1.0, 0.25, 0.15, 0.9 * (1.0 - t)),
                        );
                    }
                }
                AuraKind::DamageTaken(_) | AuraKind::DamageDone(_) => {
                    // Cursed or exposed: a dark mark above the head, wreathed
                    // in shadow.
                    let bob = (phase * 2.0).sin() * 0.05;
                    let p = a.pos + Vec3::Y * (h + 0.35 + bob);
                    b.lit(|b| b.sphere(p, 0.08, Color::new(0.1, 0.0, 0.15, 1.0)));
                    g.sprite(Cell::Wisp, p, 0.55, phase, Color::new(0.55, 0.2, 0.8, 0.9));
                    g.sprite(Cell::Glow, p, 0.5, 0.0, Color::new(0.55, 0.2, 0.8, 0.5));
                }
            }
        }
    }

    /// A glowing rune circle under someone casting, turning slowly, with a
    /// ring closing in as the cast fills and motes rising from it.
    pub fn draw_casting(&mut self, a: Anchor, id: AbilityId, progress: f32, time: f32) {
        let g = &mut self.glows;
        let color = school_color(ability(id).school);
        g.decal(
            Cell::Runes,
            a.pos,
            1.25,
            time * 0.6,
            tint(color, 0.95),
            true,
        );
        g.decal(Cell::Glow, a.pos, 1.3, 0.0, tint(color, 0.3), true);
        g.decal(
            Cell::Ring,
            a.pos,
            1.4 * progress.clamp(0.05, 1.0),
            -time,
            tint(color, 0.7),
            true,
        );
        for k in 0..5 {
            let t = (time * 0.9 + k as f32 * 0.2) % 1.0;
            let ang = k as f32 * 1.257 + time;
            let p = a.pos + vec3(ang.cos() * 0.8, t * 1.6, ang.sin() * 0.8);
            g.sprite(Cell::Flare, p, 0.25, ang, tint(color, 1.0 - t));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn missile(
    b: &mut Batch,
    g: &mut Glows,
    style: Missile,
    at: Vec3,
    dir: Vec3,
    color: Color,
    time: f32,
    seed: f32,
) {
    let side = dir.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = side.cross(dir).normalize_or(Vec3::Y);
    // A glowing comet tail streaming out behind.
    let tail = |g: &mut Glows, length: f32, width: f32, c: Color| {
        g.streak(Cell::Glow, at - dir * length, at + dir * 0.15, width, c);
    };
    // Sprites trailing behind, shrinking and fading.
    let trail = |g: &mut Glows, cell: Cell, n: usize, spacing: f32, size: f32, c: Color| {
        for i in 1..=n {
            let k = i as f32 / n as f32;
            let jitter = (side * (time * 20.0 + i as f32 + seed).sin()
                + up * (time * 17.0 + i as f32).cos())
                * 0.08
                * k;
            g.sprite(
                cell,
                at - dir * spacing * i as f32 + jitter,
                size * (1.0 - k * 0.6),
                time * 3.0 + i as f32 * 1.7 + seed,
                tint(c, 1.0 - k),
            );
        }
    };
    let orbit = |i: usize, n: usize, speed: f32, radius: f32| {
        let a = time * speed + i as f32 * TAU / n as f32 + seed;
        at + (side * a.cos() + up * a.sin()) * radius
    };
    match style {
        Missile::Fire => {
            let orange = Color::new(1.0, 0.5, 0.12, 1.0);
            g.sprite(Cell::Glow, at, 1.0, 0.0, orange);
            g.sprite(
                Cell::Flare,
                at,
                0.7,
                time * 3.0,
                Color::new(1.0, 0.85, 0.5, 0.9),
            );
            tail(g, 1.8, 0.28, tint(orange, 0.8));
            trail(g, Cell::Wisp, 7, 0.2, 0.6, Color::new(1.0, 0.38, 0.06, 0.9));
            // Smoke left behind doesn't glow.
            for i in 1..=4 {
                let k = i as f32 / 4.0;
                b.sphere(
                    at - dir * 0.4 * i as f32,
                    0.12 * (1.0 - k * 0.7),
                    Color::new(0.3, 0.25, 0.25, 0.6 * (1.0 - k)),
                );
            }
        }
        Missile::Frost => {
            // An icy shard, spinning, in a cold glow.
            let spin = time * 12.0 + seed;
            let r = side * spin.cos() + up * spin.sin();
            b.lit(|b| {
                b.cone_ref(
                    at - dir * 0.3,
                    dir * 0.8,
                    r,
                    0.14,
                    0.0,
                    4,
                    Color::new(0.75, 0.92, 1.0, 0.95),
                );
                b.cone_ref(
                    at - dir * 0.3,
                    -dir * 0.3,
                    r,
                    0.14,
                    0.0,
                    4,
                    Color::new(0.5, 0.8, 1.0, 0.9),
                );
            });
            let ice = Color::new(0.6, 0.88, 1.0, 1.0);
            g.sprite(Cell::Glow, at, 0.8, 0.0, tint(ice, 0.8));
            g.sprite(Cell::Shard, at - dir * 0.2, 0.6, spin * 0.2, ice);
            tail(g, 1.5, 0.18, tint(ice, 0.7));
            trail(g, Cell::Shard, 5, 0.28, 0.25, ice);
        }
        Missile::Shadow => {
            b.lit(|b| b.sphere(at, 0.16, Color::new(0.12, 0.02, 0.18, 0.95)));
            let purple = Color::new(0.6, 0.25, 0.85, 1.0);
            g.sprite(Cell::Glow, at, 0.9, 0.0, purple);
            for i in 0..3 {
                g.sprite(Cell::Wisp, orbit(i, 3, 10.0, 0.3), 0.45, time * 5.0, purple);
            }
            tail(g, 1.6, 0.24, tint(purple, 0.7));
            trail(
                g,
                Cell::Wisp,
                6,
                0.25,
                0.5,
                Color::new(0.4, 0.12, 0.55, 0.8),
            );
        }
        Missile::Arcane => {
            let violet = Color::new(0.85, 0.55, 1.0, 1.0);
            for i in 0..3 {
                g.sprite(
                    Cell::Flare,
                    orbit(i, 3, 14.0, 0.25),
                    0.45,
                    time * 6.0,
                    violet,
                );
            }
            g.sprite(Cell::Glow, at, 0.8, 0.0, violet);
            g.sprite(Cell::Glow, at, 0.35, 0.0, WHITE);
            tail(g, 1.4, 0.16, tint(violet, 0.7));
            trail(g, Cell::Flare, 5, 0.25, 0.3, violet);
        }
        Missile::Holy => {
            let gold = Color::new(1.0, 0.88, 0.5, 1.0);
            g.sprite(Cell::Glow, at, 0.9, 0.0, gold);
            g.sprite(Cell::Flare, at, 1.0, time * 1.5, tint(WHITE, 0.9));
            for i in 0..4 {
                g.sprite(Cell::Flare, orbit(i, 4, 8.0, 0.35), 0.2, 0.0, gold);
            }
            tail(g, 1.4, 0.2, tint(gold, 0.7));
            trail(g, Cell::Glow, 5, 0.25, 0.35, gold);
        }
        Missile::Nature => {
            for i in 0..4 {
                let a = time * 9.0 + i as f32 * 1.57 + seed;
                b.block(
                    orbit(i, 4, 9.0, 0.3),
                    vec3(0.08, 0.015, 0.05),
                    a,
                    Color::new(0.35, 0.75, 0.25, 1.0),
                );
            }
            let green = Color::new(0.5, 0.95, 0.35, 1.0);
            g.sprite(Cell::Glow, at, 0.8, 0.0, green);
            g.sprite(Cell::Wisp, at, 0.55, time * 2.0, green);
            tail(g, 1.4, 0.18, tint(green, 0.7));
            trail(g, Cell::Wisp, 5, 0.22, 0.4, green);
        }
        Missile::Arrow => {
            b.beam(
                at - dir * 0.7,
                at,
                0.02,
                side,
                Color::new(0.45, 0.32, 0.18, 1.0),
            );
            b.cone_ref(
                at,
                dir * 0.15,
                side,
                0.04,
                0.0,
                4,
                Color::new(0.75, 0.77, 0.8, 1.0),
            );
            for s in [-1.0, 1.0] {
                b.quad(
                    [
                        at - dir * 0.7,
                        at - dir * 0.55,
                        at - dir * 0.6 + side * s * 0.07,
                        at - dir * 0.75 + side * s * 0.07,
                    ],
                    Vec3::Y,
                    Color::new(0.9, 0.2, 0.15, 1.0),
                );
            }
            g.streak(
                Cell::Band,
                at - dir * 1.8,
                at - dir * 0.6,
                0.05,
                Color::new(1.0, 1.0, 1.0, 0.3),
            );
        }
        Missile::Bullet => {
            let flash = Color::new(1.0, 0.95, 0.7, 1.0);
            g.sprite(Cell::Glow, at, 0.35, 0.0, flash);
            g.streak(
                Cell::Band,
                at - dir * 1.6,
                at,
                0.07,
                Color::new(0.6, 0.85, 1.0, 0.8),
            );
        }
        Missile::Orb => {
            g.sprite(Cell::Glow, at, 0.75, 0.0, color);
            g.sprite(Cell::Flare, at, 0.5, time * 2.0, color);
            tail(g, 1.2, 0.16, tint(color, 0.7));
            trail(g, Cell::Glow, 4, 0.25, 0.3, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::data::ids;

    #[test]
    fn missiles_look_like_their_school() {
        assert_eq!(
            missile_for(ability(ids::FIREBALL), Some(Class::Mage)),
            Missile::Fire
        );
        assert_eq!(
            missile_for(ability(ids::FROSTBOLT), Some(Class::Mage)),
            Missile::Frost
        );
        assert_eq!(
            missile_for(ability(ids::STEADY_SHOT), Some(Class::Ranger)),
            Missile::Arrow
        );
    }

    #[test]
    fn missiles_burst_when_they_arrive() {
        let mut v = Vfx::default();
        let anchor = |id: EntityId| {
            Some(Anchor {
                pos: if id == 1 {
                    Vec3::ZERO
                } else {
                    vec3(10.0, 0.0, 0.0)
                },
                height: 2.0,
                yaw: 0.0,
            })
        };
        v.ability_used(ids::FIREBALL, 1, Some(2), Some(Class::Mage), anchor);
        assert_eq!(v.fx.len(), 1);
        for _ in 0..10 {
            v.update(0.05, anchor);
        }
        assert!(v.fx.iter().any(|f| matches!(f.kind, Kind::Burst { .. })));
        assert!(!v.fx.iter().any(|f| matches!(f.kind, Kind::Missile { .. })));
    }

    #[test]
    fn atlas_cells_match_the_painted_atlas() {
        let image = Image::from_file_with_format(ATLAS, None).unwrap();
        let size = 128;
        assert_eq!(image.width as usize, size * ATLAS_COLS);
        assert_eq!(image.height as usize, size * ATLAS_ROWS);
        let alpha = |x: usize, y: usize| image.bytes[(y * image.width as usize + x) * 4 + 3];
        for cell in Cell::ALL {
            let (a, b) = cell.uv();
            assert!(a.x < b.x && a.y < b.y && a.min_element() >= 0.0 && b.max_element() <= 1.0);
            let i = cell as usize;
            let (x0, y0) = ((i % ATLAS_COLS) * size, (i / ATLAS_COLS) * size);
            assert!((a.x * image.width as f32) as usize - x0 <= 1);
            // Each cell has a picture in it. All but the band (which tiles
            // sideways) fade out before the edges, so their neighbors
            // don't bleed into them.
            let brightest = (0..size * size)
                .map(|i| alpha(x0 + i % size, y0 + i / size))
                .max();
            assert!(brightest > Some(200), "{cell:?}");
            if cell != Cell::Band {
                for k in 0..size {
                    assert!(alpha(x0 + k, y0) < 8, "{cell:?}");
                    assert!(alpha(x0, y0 + k) < 8, "{cell:?}");
                }
            }
        }
    }
}
