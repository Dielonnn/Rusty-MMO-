//! Spell effects: missiles shaped by their school (fireballs trailing
//! flames, frost shards, shadow bolts, arrows, bullets...), impact bursts,
//! shockwaves, pillars of holy light, healing spirals, drain beams, melee
//! slashes, casting circles, and what auras look like on whoever has them
//! (shield bubbles, stun stars, roots, flames, frost...).

use macroquad::prelude::*;
use shared::data::{
    Ability, AbilityId, AuraKind, Class, Effect, MELEE_RANGE, School, Targeting, ability,
};
use shared::protocol::{AuraView, EntityId};
use shared::world::*;

use crate::gfx::{Batch, mix};
use crate::hud::school_color;

/// Where an entity is, for effects that follow it.
#[derive(Clone, Copy)]
pub struct Anchor {
    pub pos: Vec3,
    pub height: f32,
    pub yaw: f32,
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

    pub fn draw(&self, b: &mut Batch, time: f32, anchor: impl Fn(EntityId) -> Option<Anchor>) {
        for f in &self.fx {
            let t = (f.age / f.duration).clamp(0.0, 1.0);
            let on = f.on.and_then(&anchor);
            let base = on.map_or(f.at, |a| a.pos);
            let fade = |c: Color, k: f32| Color::new(c.r, c.g, c.b, c.a * k);
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
                    missile(b, style, at, dir, f.color, time, f.seed);
                }
                Kind::Burst { size, sparks } => {
                    let center = on.map_or(f.at, |a| a.pos + Vec3::Y * a.height * 0.55);
                    // A flash, then sparks flying out and falling.
                    b.glow_sphere(
                        center,
                        size * 0.5 * (1.0 - t),
                        fade(f.color, 0.7 * (1.0 - t)),
                    );
                    b.glow_sphere(
                        center,
                        size * 0.25 * (1.0 - t),
                        fade(Color::new(1.0, 1.0, 0.9, 1.0), 1.0 - t),
                    );
                    for i in 0..sparks {
                        let a = i as f32 * 2.399 + f.seed;
                        let up = ((i * 7) % 5) as f32 / 5.0 - 0.3;
                        let d = vec3(a.cos(), up, a.sin()).normalize();
                        let p = center + d * size * 1.6 * t.sqrt() - Vec3::Y * t * t * 0.8;
                        b.glow_sphere(
                            p,
                            0.07 * size * (1.0 - t),
                            fade(mix(f.color, WHITE, 0.3), 1.0 - t),
                        );
                    }
                }
                Kind::Shockwave { radius } => {
                    let r = radius * t.sqrt();
                    b.ground_ring(
                        base,
                        r.max(0.1),
                        0.6 * (1.0 - t) + 0.1,
                        fade(f.color, 0.85 * (1.0 - t)),
                    );
                    b.ground_ring(base, (r * 0.7).max(0.1), 0.25, fade(WHITE, 0.5 * (1.0 - t)));
                    // Debris thrown up around the edge.
                    for i in 0..12 {
                        let a = i as f32 * std::f32::consts::FRAC_PI_6 + f.seed;
                        let p = base + vec3(a.cos() * r, 0.3 + (t * 3.0).sin() * 0.6, a.sin() * r);
                        b.glow_sphere(p, 0.12 * (1.0 - t), fade(f.color, 1.0 - t));
                    }
                }
                Kind::Pillar => {
                    let k = if t < 0.2 {
                        t / 0.2
                    } else {
                        1.0 - (t - 0.2) / 0.8
                    };
                    let top = base + Vec3::Y * 9.0;
                    b.lit(|b| {
                        b.cone(
                            base,
                            top - base,
                            0.9 * k,
                            0.5 * k,
                            12,
                            fade(f.color, 0.28 * k),
                        );
                        b.cone(
                            base,
                            top - base,
                            0.45 * k,
                            0.25 * k,
                            10,
                            fade(WHITE, 0.35 * k),
                        );
                    });
                    b.ground_ring(base, 1.2, 0.3, fade(f.color, 0.8 * k));
                }
                Kind::Spiral => {
                    let h = on.map_or(2.0, |a| a.height);
                    for i in 0..10 {
                        let k = (t + i as f32 * 0.1) % 1.0;
                        let a = k * 9.0 + i as f32 * 0.63 + f.seed;
                        let p = base + vec3(a.cos() * 0.75, k * h * 1.2, a.sin() * 0.75);
                        b.glow_sphere(
                            p,
                            0.08 * (1.0 - k * 0.5),
                            fade(f.color, (1.0 - t) * (1.0 - k)),
                        );
                    }
                    b.air_ring(
                        base + Vec3::Y * (0.1 + t * h),
                        0.85,
                        0.1,
                        fade(f.color, 0.8 * (1.0 - t)),
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
                        b.lit(|b| {
                            b.beam(prev, p, 0.05 * k + 0.02, Vec3::Y, fade(f.color, 0.8 * k))
                        });
                        prev = p;
                    }
                    for i in 0..5 {
                        let s = (time * 1.8 + i as f32 * 0.2) % 1.0;
                        b.glow_sphere(a.lerp(e, s), 0.1, fade(f.color, k));
                    }
                }
                Kind::Slash { flip } => {
                    let Some(me) = on else { continue };
                    // An arc of light sweeping across in front.
                    let fwd = forward(me.yaw);
                    let side = vec3(-fwd.z, 0.0, fwd.x) * if flip { -1.0 } else { 1.0 };
                    let center = me.pos + Vec3::Y * me.height * 0.55;
                    let sweep = t * 2.6 - 1.3;
                    let segments = 8;
                    for i in 0..segments {
                        let a0 = sweep - 0.9 + i as f32 * 0.15;
                        let a1 = a0 + 0.15;
                        let dir = |a: f32| {
                            (fwd * a.cos() + side * a.sin() + Vec3::Y * a * 0.25).normalize()
                        };
                        let fadeseg = (i as f32 / segments as f32) * (1.0 - t);
                        let c = fade(mix(f.color, WHITE, 0.5), fadeseg);
                        let (d0, d1) = (dir(a0), dir(a1));
                        b.lit(|b| {
                            b.quad(
                                [
                                    center + d0 * 0.7,
                                    center + d1 * 0.7,
                                    center + d1 * 1.6,
                                    center + d0 * 1.6,
                                ],
                                Vec3::Y,
                                c,
                            );
                            b.quad(
                                [
                                    center + d0 * 1.6,
                                    center + d1 * 1.6,
                                    center + d1 * 0.7,
                                    center + d0 * 0.7,
                                ],
                                Vec3::Y,
                                c,
                            );
                        });
                    }
                }
                Kind::Dust => {
                    for i in 0..10 {
                        let a = i as f32 * 0.628 + f.seed;
                        let p = f.at + vec3(a.cos() * t * 1.8, 0.2 + t * 0.8, a.sin() * t * 1.8);
                        b.sphere(p, 0.25 + t * 0.4, fade(f.color, 0.5 * (1.0 - t)));
                    }
                }
            }
        }
    }
}

fn missile(b: &mut Batch, style: Missile, at: Vec3, dir: Vec3, color: Color, time: f32, seed: f32) {
    let trail = |b: &mut Batch, n: usize, spacing: f32, size: f32, c: Color| {
        for i in 1..=n {
            let k = i as f32 / n as f32;
            let jitter = vec3(
                (time * 20.0 + i as f32 + seed).sin(),
                (time * 17.0 + i as f32).cos(),
                0.0,
            ) * 0.06
                * k;
            b.glow_sphere(
                at - dir * spacing * i as f32 + jitter,
                size * (1.0 - k * 0.7),
                Color::new(c.r, c.g, c.b, c.a * (1.0 - k)),
            );
        }
    };
    match style {
        Missile::Fire => {
            b.glow_sphere(at, 0.32, Color::new(1.0, 0.55, 0.15, 0.9));
            b.glow_sphere(at, 0.18, Color::new(1.0, 0.95, 0.6, 1.0));
            trail(b, 7, 0.22, 0.25, Color::new(1.0, 0.35, 0.05, 0.8));
            trail(b, 4, 0.4, 0.12, Color::new(0.3, 0.25, 0.25, 0.6));
        }
        Missile::Frost => {
            // An icy shard, spinning.
            let spin = time * 12.0 + seed;
            let side = dir.cross(Vec3::Y).normalize_or_zero();
            let up = side.cross(dir);
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
            trail(b, 6, 0.25, 0.09, Color::new(0.8, 0.95, 1.0, 0.8));
        }
        Missile::Shadow => {
            b.glow_sphere(at, 0.3, Color::new(0.2, 0.05, 0.3, 0.9));
            b.glow_sphere(at, 0.16, Color::new(0.7, 0.4, 1.0, 1.0));
            for i in 0..3 {
                let a = time * 10.0 + i as f32 * 2.09;
                let side = dir.cross(Vec3::Y).normalize_or_zero();
                let p = at + (side * a.cos() + Vec3::Y * a.sin()) * 0.35;
                b.glow_sphere(p, 0.08, Color::new(0.55, 0.2, 0.8, 0.8));
            }
            trail(b, 6, 0.25, 0.2, Color::new(0.25, 0.08, 0.35, 0.7));
        }
        Missile::Arcane => {
            for i in 0..3 {
                let a = time * 14.0 + i as f32 * 2.09 + seed;
                let side = dir.cross(Vec3::Y).normalize_or_zero();
                let p = at + (side * a.cos() + Vec3::Y * a.sin()) * 0.25;
                b.glow_sphere(p, 0.13, Color::new(0.85, 0.55, 1.0, 0.95));
            }
            b.glow_sphere(at, 0.12, WHITE);
            trail(b, 5, 0.25, 0.12, Color::new(0.75, 0.45, 1.0, 0.7));
        }
        Missile::Holy => {
            b.glow_sphere(at, 0.28, Color::new(1.0, 0.9, 0.5, 0.85));
            b.glow_sphere(at, 0.14, WHITE);
            for i in 0..4 {
                let a = time * 8.0 + i as f32 * 1.57;
                b.glow_sphere(
                    at + vec3(a.cos(), a.sin(), 0.0) * 0.35,
                    0.05,
                    Color::new(1.0, 1.0, 0.8, 0.9),
                );
            }
            trail(b, 5, 0.25, 0.14, Color::new(1.0, 0.85, 0.4, 0.7));
        }
        Missile::Nature => {
            b.glow_sphere(at, 0.24, Color::new(0.45, 0.95, 0.35, 0.9));
            for i in 0..4 {
                let a = time * 9.0 + i as f32 * 1.57 + seed;
                let side = dir.cross(Vec3::Y).normalize_or_zero();
                let p = at + (side * a.cos() + Vec3::Y * a.sin()) * 0.3;
                b.block(
                    p,
                    vec3(0.08, 0.015, 0.05),
                    a,
                    Color::new(0.35, 0.75, 0.25, 1.0),
                );
            }
            trail(b, 5, 0.22, 0.12, Color::new(0.5, 1.0, 0.4, 0.7));
        }
        Missile::Arrow => {
            let side = dir.cross(Vec3::Y).normalize_or_zero();
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
            trail(b, 3, 0.3, 0.04, Color::new(1.0, 1.0, 1.0, 0.4));
        }
        Missile::Bullet => {
            b.glow_sphere(at, 0.08, Color::new(1.0, 0.95, 0.7, 1.0));
            let side = dir.cross(Vec3::Y).normalize_or_zero();
            b.lit(|b| {
                b.beam(
                    at - dir * 1.2,
                    at,
                    0.025,
                    side,
                    Color::new(0.6, 0.85, 1.0, 0.6),
                )
            });
        }
        Missile::Orb => {
            b.glow_sphere(at, 0.22, color);
            trail(b, 4, 0.25, 0.14, Color::new(color.r, color.g, color.b, 0.6));
        }
    }
}

/// What lingering auras look like on someone: shields, stuns, roots,
/// burning, poison, frost, healing, buffs and curses.
pub fn draw_auras(b: &mut Batch, a: Anchor, auras: &[AuraView], time: f32) {
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
                        Color::new(color.r, color.g, color.b, 0.16),
                    )
                });
                b.air_ring(
                    a.pos + Vec3::Y * (h * 0.5 + (phase * 2.0).sin() * h * 0.4),
                    0.9,
                    0.05,
                    Color::new(1.0, 1.0, 1.0, 0.5),
                );
            }
            AuraKind::Stun => {
                for k in 0..3 {
                    let ang = phase * 4.0 + k as f32 * 2.09;
                    let p = a.pos + vec3(ang.cos() * 0.35, h + 0.25, ang.sin() * 0.35);
                    b.glow_sphere(p, 0.07, Color::new(1.0, 0.95, 0.3, 1.0));
                    b.glow_sphere(p + Vec3::Y * 0.05, 0.035, WHITE);
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
            }
            AuraKind::Slow(_) => {
                for k in 0..5 {
                    let ang = k as f32 * 1.257 + phase * 0.5;
                    let p = a.pos + vec3(ang.cos() * 0.45, 0.05, ang.sin() * 0.45);
                    b.lit(|b| {
                        b.cone(
                            p,
                            vec3(0.0, 0.35, 0.0),
                            0.08,
                            0.0,
                            4,
                            Color::new(0.7, 0.9, 1.0, 0.85),
                        )
                    });
                }
            }
            AuraKind::Dot { .. } => {
                // Burning, poisoned, bleeding or shadow-eaten.
                for k in 0..4 {
                    let t = (phase * 1.3 + k as f32 * 0.25) % 1.0;
                    let ang = k as f32 * 1.57 + phase;
                    let p = a.pos + vec3(ang.cos() * 0.3, h * (0.4 + t * 0.6), ang.sin() * 0.3);
                    let c = match ab.school {
                        School::Fire => Color::new(1.0, 0.5 - t * 0.3, 0.1, 0.9 * (1.0 - t)),
                        School::Nature => Color::new(0.5, 1.0, 0.3, 0.8 * (1.0 - t)),
                        School::Physical => Color::new(0.75, 0.1, 0.1, 0.9 * (1.0 - t)),
                        _ => Color::new(color.r * 0.6, color.g * 0.4, color.b, 0.8 * (1.0 - t)),
                    };
                    let p = if ab.school == School::Physical {
                        // Blood drips fall.
                        a.pos + vec3(ang.cos() * 0.25, h * (0.7 - t * 0.6), ang.sin() * 0.25)
                    } else {
                        p
                    };
                    b.glow_sphere(p, 0.09 * (1.0 - t * 0.5), c);
                }
            }
            AuraKind::Hot { .. } => {
                for k in 0..4 {
                    let t = (phase * 0.8 + k as f32 * 0.25) % 1.0;
                    let ang = k as f32 * 1.57 + t * 3.0;
                    let p = a.pos + vec3(ang.cos() * 0.5, t * h, ang.sin() * 0.5);
                    b.glow_sphere(p, 0.06, Color::new(0.5, 1.0, 0.45, 0.9 * (1.0 - t)));
                }
            }
            AuraKind::Speed(_) => {
                for k in 0..3 {
                    let t = (phase * 3.0 + k as f32 * 0.33) % 1.0;
                    let back = -forward(a.yaw);
                    let side = vec3(back.z, 0.0, -back.x) * (k as f32 - 1.0) * 0.3;
                    let p = a.pos + Vec3::Y * (0.3 + k as f32 * 0.4) + side + back * t;
                    b.lit(|b| {
                        b.beam(
                            p,
                            p + back * 0.5,
                            0.02,
                            Vec3::Y,
                            Color::new(0.9, 0.95, 1.0, 0.5 * (1.0 - t)),
                        )
                    });
                }
            }
            AuraKind::DamageTaken(f) if f < 1.0 => {
                // Toughened: a golden shimmer.
                b.ground_ring(a.pos, 0.9, 0.12, Color::new(1.0, 0.85, 0.35, 0.7));
                let y = (phase * 1.5).sin() * 0.5 + 0.5;
                b.air_ring(
                    a.pos + Vec3::Y * h * y,
                    0.75,
                    0.05,
                    Color::new(1.0, 0.9, 0.5, 0.6),
                );
            }
            AuraKind::DamageDone(f) if f > 1.0 => {
                // Empowered: a red glow rising from the feet.
                for k in 0..6 {
                    let t = (phase * 1.5 + k as f32 / 6.0) % 1.0;
                    let ang = k as f32 * 1.047;
                    b.glow_sphere(
                        a.pos + vec3(ang.cos() * 0.5, t * 1.2, ang.sin() * 0.5),
                        0.07,
                        Color::new(1.0, 0.25, 0.15, 0.8 * (1.0 - t)),
                    );
                }
            }
            AuraKind::DamageTaken(_) | AuraKind::DamageDone(_) => {
                // Cursed or exposed: a dark mark above the head.
                let bob = (phase * 2.0).sin() * 0.05;
                b.glow_sphere(
                    a.pos + Vec3::Y * (h + 0.35 + bob),
                    0.12,
                    Color::new(0.55, 0.2, 0.8, 0.8),
                );
                b.glow_sphere(
                    a.pos + Vec3::Y * (h + 0.35 + bob),
                    0.06,
                    Color::new(0.1, 0.0, 0.15, 1.0),
                );
            }
        }
    }
}

/// A glowing rune circle under someone casting, turning slowly, with motes
/// rising from it.
pub fn draw_casting(b: &mut Batch, a: Anchor, id: AbilityId, progress: f32, time: f32) {
    let color = school_color(ability(id).school);
    let c = |k: f32| Color::new(color.r, color.g, color.b, k);
    b.ground_ring(a.pos, 1.1, 0.08, c(0.9));
    b.ground_ring(a.pos, 0.8, 0.05, c(0.6));
    b.ground_ring(a.pos, 1.1 * progress.clamp(0.05, 1.0), 0.14, c(0.35));
    for k in 0..6 {
        let ang = time * 0.8 + k as f32 * 1.047;
        let p = a.pos + vec3(ang.cos() * 0.95, 0.06, ang.sin() * 0.95);
        let q = p + vec3(ang.cos(), 0.0, ang.sin()) * 0.0;
        b.lit(|b| b.block(q, vec3(0.08, 0.01, 0.08), ang, c(0.9)));
    }
    for k in 0..5 {
        let t = (time * 0.9 + k as f32 * 0.2) % 1.0;
        let ang = k as f32 * 1.257 + time;
        b.glow_sphere(
            a.pos + vec3(ang.cos() * 0.8, t * 1.6, ang.sin() * 0.8),
            0.05,
            c(1.0 - t),
        );
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
}
