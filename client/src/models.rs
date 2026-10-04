//! Character and creature models, and how they move.
//!
//! Every model is built from shaded primitives in its own `Frame`, so all of
//! it turns together. Humanoids are posed by an `Anim` (angles for each arm
//! and leg, a lean and a crouch) worked out from what they're doing and from
//! their class: a barbarian chops overhead with both hands, a monk throws
//! alternating punches and kicks, a ranger draws a bow, a druid spreads their
//! arms to cast, and so on.

use macroquad::prelude::*;
use shared::data::{
    Appearance, Class, GiantStyle, HumanoidStyle, ItemId, MobModel, Race, Slot, item, items,
};
use shared::protocol::EntityKind;

use crate::render::{Batch, Frame, c, dark, mix, rgb};

/// How a character is moving, for animation.
#[derive(Clone, Copy, Default)]
pub struct Pose {
    /// Walk cycle phase in radians.
    pub walk: f32,
    pub moving: bool,
    pub casting: bool,
    /// Attack animation progress, 0 (start) to 1 (done).
    pub swing: f32,
    /// How many attacks so far, to alternate hands and moves.
    pub combo: u32,
    pub dead: bool,
    /// In the air (jumping or falling).
    pub airborne: bool,
    /// Flinching from a hit: 1 just hit, fading to 0.
    pub hurt: f32,
    pub time: f32,
}

impl Pose {
    fn attacking(&self) -> bool {
        self.swing > 0.0 && self.swing < 1.0 && !self.dead
    }
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
                MobModel::Spider => spider(b, pos, yaw, colors, look.seed, pose),
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

pub(crate) const STEEL: Color = c(0.72, 0.74, 0.78);
pub(crate) const GOLD: Color = c(0.92, 0.74, 0.3);
pub(crate) const WOOD: Color = c(0.42, 0.28, 0.16);
pub(crate) const LEATHER: Color = c(0.42, 0.28, 0.17);
pub(crate) const BONE: Color = c(0.88, 0.85, 0.76);

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

/// Classes in plate, with knee and elbow guards.
fn plated(class: Class) -> bool {
    matches!(class, Class::Fighter | Class::Paladin)
}

// ---- Animation ----

/// How someone fights, which decides how they stand, swing and cast.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Style {
    TwoHander,
    SwordBoard,
    Hammer,
    Fists,
    Daggers,
    Bow,
    Rifle,
    Lute,
    Staff,
    Holy,
    Nature,
    Wild,
    Fel,
    Merchant,
    Brute,
    Caster,
    Giant,
}

fn style_of(outfit: Outfit) -> Style {
    match outfit {
        Outfit::Class(class) => match class {
            Class::Barbarian => Style::TwoHander,
            Class::Fighter => Style::SwordBoard,
            Class::Paladin => Style::Hammer,
            Class::Monk => Style::Fists,
            Class::Rogue => Style::Daggers,
            Class::Ranger => Style::Bow,
            Class::Artificer => Style::Rifle,
            Class::Bard => Style::Lute,
            Class::Mage => Style::Staff,
            Class::Cleric => Style::Holy,
            Class::Druid => Style::Nature,
            Class::Sorcerer => Style::Wild,
            Class::Warlock => Style::Fel,
        },
        Outfit::Merchant => Style::Merchant,
        Outfit::Mob(style, _) => match style {
            HumanoidStyle::Mystic
            | HumanoidStyle::Trickster
            | HumanoidStyle::Necromancer
            | HumanoidStyle::Shaman
            | HumanoidStyle::TrollShaman
            | HumanoidStyle::TroggShaman => Style::Caster,
            _ => Style::Brute,
        },
        Outfit::Giant(..) => Style::Giant,
    }
}

/// One arm or leg: swung forward (radians), spread out to the side, and bent
/// at the elbow or knee.
#[derive(Clone, Copy, Default, Debug)]
struct Limb {
    swing: f32,
    spread: f32,
    bend: f32,
}

const fn limb(swing: f32, spread: f32, bend: f32) -> Limb {
    Limb {
        swing,
        spread,
        bend,
    }
}

/// A whole-body pose.
#[derive(Clone, Copy, Default, Debug)]
struct Anim {
    /// Forward lean from the feet.
    lean: f32,
    /// How far the body sinks (bent knees).
    crouch: f32,
    /// Bounce of the body.
    bob: f32,
    /// Left, right.
    arms: [Limb; 2],
    legs: [Limb; 2],
}

/// A smooth 0-1-0 bump over `t` in `0..1`.
fn bump(t: f32) -> f32 {
    (t.clamp(0.0, 1.0) * std::f32::consts::PI).sin()
}

/// Windup then strike: 0 to 1 over the first `split` of `t`, then back down.
fn windup(t: f32, split: f32) -> (f32, f32) {
    if t < split {
        let k = t / split;
        (k * k * (3.0 - 2.0 * k), 0.0)
    } else {
        let k = ((t - split) / (1.0 - split)).min(1.0);
        (1.0 - k, k)
    }
}

fn animate(style: Style, pose: Pose) -> Anim {
    let mut a = Anim::default();
    let t = pose.time;
    let moving = pose.moving && !pose.dead;
    // Walking and running.
    if moving {
        let w = pose.walk;
        for (i, phase) in [0.0f32, std::f32::consts::PI].into_iter().enumerate() {
            a.legs[i] = limb(
                (w + phase).sin() * 0.65,
                0.0,
                (w + phase + 1.2).sin().max(0.0) * 0.9,
            );
            // Arms swing against the legs.
            a.arms[i] = limb(-(w + phase).sin() * 0.7, 0.08, 0.35);
        }
        a.lean = 0.08;
        a.bob = (w * 2.0).sin().abs() * 0.035;
    } else {
        // Breathing; arms hang a little out from the body.
        let breathe = (t * 1.6).sin();
        a.arms = [limb(0.05, 0.1, 0.2), limb(0.05, 0.1, 0.2)];
        a.arms[0].swing += breathe * 0.03;
        a.arms[1].swing -= breathe * 0.03;
        a.bob = breathe * 0.006;
    }

    // Idle stances (arms only, so they hold while walking too).
    let idle = !pose.casting && !pose.attacking();
    if idle {
        match style {
            Style::TwoHander if !moving => {
                // Axe resting on the shoulder.
                a.arms[1] = limb(0.5, 0.15, 2.0);
            }
            Style::TwoHander => a.arms[1] = limb(0.4 + a.arms[1].swing * 0.3, 0.15, 1.9),
            Style::SwordBoard => {
                // Shield up.
                a.arms[0] = limb(0.85, 0.05, 0.75);
                if !moving {
                    a.arms[1] = limb(0.35, 0.15, 0.4);
                }
            }
            Style::Hammer if !moving => a.arms[1] = limb(0.4, 0.1, 0.9),
            Style::Fists => {
                // Guard up, light on the feet.
                let bounce = (t * 5.0).sin();
                a.arms = [limb(0.65, -0.1, 1.9), limb(0.55, -0.1, 2.0)];
                if !moving {
                    a.crouch = 0.05 + bounce.abs() * 0.025;
                    a.legs = [limb(0.25, 0.08, 0.5), limb(-0.15, 0.08, 0.4)];
                }
            }
            Style::Daggers => {
                // Low and ready, daggers forward.
                a.crouch += 0.06;
                a.lean += 0.1;
                a.arms = [limb(0.45, 0.15, 0.7), limb(0.5, 0.15, 0.65)];
                if !moving {
                    a.legs = [limb(0.3, 0.05, 0.6), limb(-0.1, 0.05, 0.55)];
                }
            }
            Style::Bow if !moving => a.arms[0] = limb(0.25, 0.15, 0.2),
            Style::Rifle => {
                // Rifle across the chest.
                a.arms = [limb(0.75, -0.25, 0.9), limb(0.35, 0.05, 1.3)];
            }
            Style::Lute => {
                let strum = (t * 9.0).sin() * 0.12;
                a.arms = [limb(0.7, -0.05, 1.1), limb(0.55 + strum, -0.2, 1.1)];
            }
            Style::Staff if !moving => a.arms[1] = limb(0.25, 0.12, 0.35),
            Style::Holy if !moving => {
                // Hands together in front.
                a.arms = [limb(0.5, -0.25, 1.4), limb(0.5, -0.25, 1.4)];
            }
            Style::Wild if !moving => {
                let drift = (t * 1.3).sin() * 0.1;
                a.arms = [limb(0.35 + drift, 0.35, 0.6), limb(0.35 - drift, 0.35, 0.6)];
            }
            Style::Fel if !moving => {
                a.arms[1] = limb(0.7, 0.15, 0.5);
            }
            Style::Merchant if !moving => {
                // Hands clasped over the apron, now and then a wave.
                let wave = ((t * 0.4).sin() > 0.8) as i32 as f32;
                a.arms = [
                    limb(0.4, -0.25, 1.2),
                    limb(0.4 + wave * 2.0, -0.25 + wave * 0.6, 1.2 - wave * 0.6),
                ];
            }
            Style::Giant if !moving => {
                // Heavy, slow breathing.
                let heave = (t * 0.9).sin() * 0.05;
                a.arms = [limb(0.1 + heave, 0.2, 0.3), limb(0.1 + heave, 0.2, 0.3)];
                a.bob = heave * 0.1;
            }
            _ => {}
        }
    }

    // Casting.
    if pose.casting && !pose.dead {
        let wobble = (t * 6.0).sin() * 0.1;
        match style {
            Style::Staff => {
                a.arms = [limb(1.35 + wobble, 0.1, 0.15), limb(2.6, 0.15, 0.2)];
            }
            Style::Holy | Style::Hammer => {
                // Arms raised to the sky.
                a.arms = [limb(2.6 + wobble, 0.4, 0.15), limb(2.6 - wobble, 0.4, 0.15)];
                a.lean = -0.06;
            }
            Style::Nature => {
                // Arms spread wide.
                a.arms = [limb(0.6, 1.25 + wobble, 0.2), limb(0.6, 1.25 - wobble, 0.2)];
                a.lean = -0.08;
            }
            Style::Wild => {
                let w2 = (t * 9.0).sin() * 0.2;
                a.arms = [limb(1.4 + w2, 0.25, 0.1), limb(1.4 - w2, 0.25, 0.1)];
            }
            Style::Fel => {
                // One hand clawing forward, the other at the chest.
                a.lean = 0.14;
                a.arms = [limb(0.9, -0.2, 1.6), limb(1.5 + wobble, 0.1, 0.1)];
            }
            Style::Bow => {
                // Bow drawn and held.
                a.arms = [limb(1.55, 0.1, 0.0), limb(1.45, -0.1, 2.1)];
            }
            Style::Rifle => {
                a.arms = [limb(1.4, -0.2, 0.25), limb(1.15, 0.0, 0.6)];
            }
            Style::Lute => {
                let strum = (t * 16.0).sin() * 0.2;
                a.arms = [limb(0.7, -0.05, 1.1), limb(0.6 + strum, -0.2, 1.1)];
                a.lean = -0.05;
            }
            Style::Caster => {
                a.arms = [limb(1.1 + wobble, 0.25, 0.4), limb(1.1 - wobble, 0.25, 0.4)];
            }
            _ => {
                a.arms = [limb(1.0 + wobble, 0.2, 0.5), limb(1.0 - wobble, 0.2, 0.5)];
            }
        }
    }

    // Attacking.
    if pose.attacking() {
        let s = pose.swing;
        match style {
            Style::TwoHander | Style::Giant => {
                // Both hands, high over the head and down.
                let (up, down) = windup(s, 0.4);
                let swing = 0.6 + up * 2.4 + down * 0.0 - down * 0.2;
                let arm = limb(swing, -0.15, 0.4 + up * 0.3);
                a.arms = [arm, arm];
                a.lean += down * 0.22 - up * 0.08;
                a.crouch += down * 0.05;
            }
            Style::SwordBoard | Style::Brute => {
                // A slash from high on the right across the body.
                let (up, down) = windup(s, 0.35);
                a.arms[1] = limb(0.6 + up * 1.8 + down * 0.6, 0.6 * up - 0.5 * down, 0.3);
                a.lean += down * 0.1;
            }
            Style::Hammer => {
                let (up, down) = windup(s, 0.4);
                a.arms[1] = limb(0.4 + up * 2.5 - down * 0.2, 0.1, 0.3);
                a.lean += down * 0.15;
            }
            Style::Fists => {
                let k = bump(s * 1.6);
                if pose.combo % 3 == 2 {
                    // A high kick.
                    a.legs[1] = limb(0.4 + k * 1.2, 0.0, 0.2 * (1.0 - k));
                    a.lean -= k * 0.15;
                } else {
                    // Jab with alternating fists.
                    let i = (pose.combo % 2) as usize;
                    a.arms[i] = limb(0.65 + k * 0.95, -0.1, 2.0 - k * 2.0);
                    a.lean += k * 0.08;
                }
            }
            Style::Daggers => {
                // Twin stabs, the left a beat behind.
                let r = bump(s * 1.8);
                let l = bump(s * 1.8 - 0.5);
                a.arms = [
                    limb(0.45 + l * 1.0, 0.1, 0.7 - l * 0.6),
                    limb(0.5 + r * 1.0, 0.1, 0.65 - r * 0.6),
                ];
                a.lean += 0.08;
            }
            Style::Bow => {
                // Draw, then let fly.
                let draw = (s / 0.6).min(1.0);
                let release = s > 0.6;
                a.arms = [
                    limb(1.55, 0.1, 0.0),
                    if release {
                        limb(1.5, -0.4, 0.6)
                    } else {
                        limb(1.45, -0.1, 0.6 + draw * 1.5)
                    },
                ];
            }
            Style::Rifle => {
                // Aim, then the kick of the shot.
                let kick = if s > 0.3 { bump((s - 0.3) * 2.5) } else { 0.0 };
                a.arms = [
                    limb(1.4 + kick * 0.25, -0.2, 0.25),
                    limb(1.15 + kick * 0.3, 0.0, 0.6),
                ];
                a.lean -= kick * 0.1;
            }
            Style::Lute => {
                // A flourish across the strings.
                let k = bump(s);
                a.arms[1] = limb(0.55 + k * 1.2, -0.2 + k * 0.5, 1.1 - k * 0.8);
                a.arms[0] = limb(0.7, -0.05, 1.1);
            }
            Style::Merchant => {}
            _ => {
                // Casters flick a wand.
                let k = bump(s * 1.4);
                a.arms[1] = limb(0.3 + k * 1.3, 0.15, 0.3);
            }
        }
    }

    // In the air: knees tucked, arms out for balance.
    if pose.airborne && !pose.dead {
        a.legs = [limb(0.6, 0.05, 1.2), limb(-0.1, 0.05, 0.6)];
        if idle {
            a.arms[0] = limb(0.9, 0.5, 0.4);
            a.arms[1].spread += 0.4;
        }
    }
    // Flinch from a hit.
    a.lean -= pose.hurt * 0.25;
    a
}

/// Direction of a limb in a body's local space. `side` is -1 for the left.
fn limb_dir(swing: f32, spread: f32, side: f32) -> Vec3 {
    vec3(
        side * spread.sin(),
        -swing.cos() * spread.cos(),
        swing.sin() * spread.cos(),
    )
}

// ---- Humanoids ----

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
    let style = style_of(outfit);
    let anim = animate(style, pose);
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, scale, false)
    } else {
        Frame::leaning(pos, yaw, scale, anim.lean)
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
    let has_plate = matches!(outfit, Outfit::Class(class) if plated(class));
    let goat_legs = matches!(
        outfit,
        Outfit::Mob(HumanoidStyle::Satyr | HumanoidStyle::Trickster, _)
    );
    let skeletal = matches!(
        outfit,
        Outfit::Mob(HumanoidStyle::Skeleton, _) | Outfit::Giant(GiantStyle::Bone, _)
    );
    let limb_w = shape.limbs * if skeletal { 0.55 } else { 1.0 };
    let trim = dark(torso, 0.7);

    // Legs: thigh, shin and a boot (or hoof). The body sinks by `crouch`,
    // bending the knees to keep the feet on the ground.
    let crouch = anim.crouch;
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let l = anim.legs[i];
        let sink = crouch * 2.2;
        let hip = vec3(
            0.13 * side * if slender { 1.1 } else { 1.0 },
            0.95 - crouch,
            0.0,
        );
        let thigh = l.swing + sink * 0.5;
        let knee = fr.segment(
            b,
            hip,
            limb_dir(thigh, l.spread, side),
            0.1 * limb_w,
            0.46,
            legs,
        );
        let hoof = if goat_legs { 0.6 } else { 0.0 };
        let shin = thigh - l.bend - sink + hoof;
        let ankle = fr.segment(
            b,
            knee,
            limb_dir(shin, l.spread * 0.5, side),
            0.085 * limb_w,
            0.45,
            legs,
        );
        if has_plate {
            fr.sphere(b, knee + vec3(0.0, 0.0, 0.06), 0.085, STEEL);
        }
        if goat_legs {
            fr.cube(b, ankle, vec3(0.07, 0.06, 0.08), c(0.12, 0.1, 0.08));
        } else {
            let foot = ankle + vec3(0.0, -0.01, 0.07);
            fr.cube(b, foot, vec3(0.085, 0.06, 0.15), boots);
            // Boot cuff and sole.
            fr.cube(
                b,
                ankle + vec3(0.0, 0.1, 0.0),
                vec3(0.095, 0.03, 0.095),
                dark(boots, 0.75),
            );
            fr.cube(
                b,
                foot + vec3(0.0, -0.05, 0.0),
                vec3(0.088, 0.012, 0.155),
                dark(boots, 0.5),
            );
        }
    }
    let y = anim.bob - crouch;
    if is_robed {
        let sway = (anim.legs[0].swing - anim.legs[1].swing) * 0.08;
        fr.tilted(
            b,
            vec3(0.0, 0.6 + y * 0.5, 0.0),
            vec3(0.27, 0.36, 0.2),
            sway,
            legs,
        );
        fr.tilted(
            b,
            vec3(0.0, 0.3 + y * 0.5, 0.0),
            vec3(0.3, 0.08, 0.23),
            sway * 1.5,
            dark(legs, 0.85),
        );
        // Embroidered hem.
        fr.tilted(
            b,
            vec3(0.0, 0.23 + y * 0.5, 0.0),
            vec3(0.305, 0.02, 0.235),
            sway * 1.5,
            GOLD,
        );
    }
    if goat_legs {
        fr.cube(
            b,
            vec3(0.0, 0.85 + y, 0.0),
            vec3(0.28, 0.16, 0.18),
            dark(legs, 0.85),
        );
    }

    // Body.
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
        // Collar and a seam down the front.
        fr.cube(
            b,
            vec3(0.0, 1.535 + y, 0.0),
            vec3(chest_w * 0.55, 0.025, 0.14),
            trim,
        );
        fr.cube(b, vec3(0.0, 1.3 + y, 0.172), vec3(0.012, 0.2, 0.004), trim);
        // Belt, buckle and a pouch.
        fr.cube(
            b,
            vec3(0.0, 1.07 + y, 0.0),
            vec3(hips_w + 0.01, 0.04, 0.155),
            c(0.25, 0.17, 0.1),
        );
        fr.cube(b, vec3(0.0, 1.07 + y, 0.16), vec3(0.045, 0.035, 0.01), GOLD);
        fr.cube(
            b,
            vec3(-hips_w + 0.02, 1.0 + y, 0.12),
            vec3(0.06, 0.07, 0.04),
            LEATHER,
        );
    }
    if let Some(cc) = cape.filter(|_| worn(Slot::Chest).is_none()) {
        let flap = 0.08
            + (anim.legs[0].swing.abs() + anim.legs[1].swing.abs()) * 0.12
            + (pose.time * 2.0 + seed as f32).sin() * 0.02;
        fr.tilted(
            b,
            vec3(0.0, 1.13 + y, -0.21),
            vec3(0.25, 0.44, 0.02),
            -flap,
            cc,
        );
        // Clasps at the shoulders.
        for sx in [-1.0, 1.0] {
            fr.sphere(b, vec3(0.16 * sx, 1.5 + y, -0.17), 0.035, GOLD);
        }
    }

    // Neck and head.
    let hs = shape.head;
    fr.cylinder(
        b,
        vec3(0.0, 1.53 + y, 0.0),
        0.1,
        0.075 * limb_w.max(0.8),
        skin,
    );
    let nod = if pose.hurt > 0.0 {
        -0.03 * pose.hurt
    } else {
        0.0
    };
    let head = vec3(0.0, 1.55 + 0.21 * hs + y, 0.01 + nod);
    match outfit {
        Outfit::Giant(style, colors) => giant_head(b, &fr, head, style, colors, pose),
        _ => face(b, &fr, head, hs, race, skin, hair, outfit, skeletal, pose),
    }

    // Hair (players pick a style; mobs vary by seed).
    let hair_style_id = match outfit {
        Outfit::Class(_) | Outfit::Merchant => a.hair_style,
        Outfit::Mob(HumanoidStyle::Bandit | HumanoidStyle::Raider, _) => 1 + (seed % 2) as u8,
        _ => 0,
    };
    let headgear = worn(Slot::Head);
    if headgear.is_none() && !matches!(outfit, Outfit::Giant(..)) {
        hair_style(b, &fr, head, hs, hair_style_id, hair, pose, anim.lean);
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

    // Arms.
    let shoulder_y = 1.53 + y;
    let sw = chest_w + 0.08;
    let mut hand_pos = [Vec3::ZERO; 2];
    let mut forearm = [0.0f32; 2];
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let l = anim.arms[i];
        let shoulder = vec3(sw * side, shoulder_y, 0.0);
        fr.sphere(b, shoulder, 0.1 * limb_w.max(0.7), arms);
        let el = fr.segment(
            b,
            shoulder,
            limb_dir(l.swing, l.spread, side),
            0.08 * limb_w,
            0.34,
            arms,
        );
        let fore = l.swing + l.bend;
        let sleeve = if is_robed { arms } else { dark(arms, 0.95) };
        let wrist = fr.segment(
            b,
            el,
            limb_dir(fore, l.spread * 0.7, side),
            0.07 * limb_w,
            0.32,
            sleeve,
        );
        if has_plate {
            fr.sphere(b, el, 0.075, STEEL);
        }
        if !skeletal && !bare_skin {
            // A bracer at the wrist.
            let d = limb_dir(fore, l.spread * 0.7, side);
            fr.segment(
                b,
                wrist - d * 0.1,
                d,
                0.078 * limb_w,
                0.08,
                dark(sleeve, 0.7),
            );
        }
        fr.sphere(
            b,
            wrist + vec3(0.0, -0.04, 0.0),
            0.075 * limb_w.max(0.8),
            hands,
        );
        hand_pos[i] = wrist + vec3(0.0, -0.05, 0.0);
        forearm[i] = fore;
    }
    let h = Hands {
        left: hand_pos[0],
        right: hand_pos[1],
        left_angle: forearm[0] + 1.55,
        right_angle: forearm[1] + 1.55,
        shoulder_y,
        sw,
        y,
        chest_w,
    };
    gear_and_weapons(b, &fr, outfit, &h, look, pose, legs);
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
    // Spell light gathering in the hands while casting.
    if pose.casting && !pose.dead {
        let col = cast_color(style);
        let pulse = 0.06 + (pose.time * 8.0).sin().abs() * 0.04;
        for p in hand_pos {
            fr.glow(b, p + vec3(0.0, 0.04, 0.06), pulse, col);
        }
    }
}

/// The color of the light in someone's hands while they cast.
fn cast_color(style: Style) -> Color {
    match style {
        Style::Staff => Color::new(1.0, 0.55, 0.2, 0.85),
        Style::Holy | Style::Hammer => Color::new(1.0, 0.95, 0.6, 0.85),
        Style::Nature => Color::new(0.5, 1.0, 0.4, 0.85),
        Style::Wild => Color::new(0.8, 0.4, 1.0, 0.85),
        Style::Fel => Color::new(0.4, 1.0, 0.3, 0.85),
        Style::Lute => Color::new(0.95, 0.6, 1.0, 0.7),
        Style::Rifle => Color::new(0.5, 0.85, 1.0, 0.7),
        Style::Caster => Color::new(0.7, 0.5, 1.0, 0.85),
        _ => Color::new(0.9, 0.9, 1.0, 0.6),
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
    pose: Pose,
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
        // Chin and cheeks.
        fr.ellipsoid(
            b,
            head + vec3(0.0, -0.13, 0.08) * hs,
            vec3(0.11 * jaw, 0.07, 0.1) * hs,
            skin,
        );
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
    // Blink every few seconds.
    let blink = (pose.time * 0.5 + head.x).fract() < 0.03;
    for sx in [-1.0, 1.0] {
        let eye = head + vec3(0.07 * sx, 0.03, 0.17) * hs;
        match eye_glow {
            Some(g) => {
                fr.sphere(b, eye, 0.04 * hs, c(0.08, 0.06, 0.08));
                fr.glow(b, eye + vec3(0.0, 0.0, 0.02) * hs, 0.022 * hs, g);
            }
            None if blink => {
                fr.cube(
                    b,
                    eye + vec3(0.0, 0.0, 0.02) * hs,
                    vec3(0.035, 0.006, 0.01) * hs,
                    dark(skin, 0.7),
                );
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
    // The mouth opens with a shout when hit.
    let open = 0.01 + pose.hurt * 0.025;
    fr.cube(
        b,
        head + vec3(0.0, -0.09, 0.18) * hs,
        vec3(w, open, 0.01) * hs,
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
    lean: f32,
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
            // A fringe.
            fr.cube(
                b,
                head + vec3(0.0, 0.14, 0.17) * hs,
                vec3(0.18, 0.03, 0.03) * hs,
                dark(hair, 0.9),
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
            let sway = (pose.time * 2.0).sin() * 0.1 - lean * 1.5;
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
                // A bone charm on the headband.
                fr.cone_dir(
                    b,
                    h(vec3(0.15, 0.1, 0.12)),
                    vec3(0.04, -0.08, 0.02) * hs,
                    0.015 * hs,
                    BONE,
                );
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
                fr.cube(
                    b,
                    h(vec3(0.0, 0.24, -0.02)),
                    vec3(0.02, 0.04, 0.16) * hs,
                    c(0.18, 0.3, 0.6),
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
                fr.cube(
                    b,
                    h(vec3(0.0, -0.12, -0.17)),
                    vec3(0.2, 0.1, 0.08) * hs,
                    c(0.22, 0.32, 0.18),
                );
            }
            Class::Artificer => {
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
                let hat = c(0.6, 0.18, 0.45);
                fr.cylinder(b, h(vec3(0.0, 0.14, 0.0)), 0.04 * hs, 0.3 * hs, hat);
                fr.ellipsoid(b, h(vec3(0.0, 0.22, 0.0)), vec3(0.2, 0.08, 0.2) * hs, hat);
                let bob = (pose.time * 3.0).sin() * 0.03;
                fr.beam(
                    b,
                    h(vec3(0.15, 0.2, -0.05)),
                    h(vec3(0.3, 0.55 + bob, -0.25)),
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
                    fr.sphere(
                        b,
                        h(vec3(0.2 * sx, 0.16, 0.08)),
                        0.03 * hs,
                        c(0.4, 0.65, 0.3),
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
                // Stars on the hat.
                fr.glow(b, h(vec3(0.08, 0.38, 0.12)), 0.022 * hs, c(1.0, 0.9, 0.5));
                fr.glow(b, h(vec3(-0.05, 0.5, 0.07)), 0.016 * hs, c(1.0, 0.9, 0.5));
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
                fr.glow(b, h(vec3(0.0, 0.2, 0.2)), 0.025 * hs, c(0.8, 0.4, 1.0));
            }
            Class::Warlock => {
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
                fr.cone_dir(
                    b,
                    head + vec3(0.04 * sx, -0.1, 0.2),
                    vec3(0.0, -0.06, 0.0),
                    0.015,
                    c(0.95, 0.95, 0.9),
                );
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
            for k in 0..5 {
                let a = k as f32 * 1.26 + (pose.time * 0.5).sin() * 0.05;
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
    let tip = fr.limb(b, guard, angle, 0.04, length, STEEL);
    // A bright edge along the blade.
    fr.limb(b, guard, angle, 0.012, length * 0.98, c(0.92, 0.94, 0.98));
    let _ = tip;
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
    fr.sphere(b, hand + vec3(-0.12, 0.05, 0.13), 0.06, trim);
}

#[allow(clippy::too_many_arguments)]
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
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx * 0.9, h.shoulder_y + 0.1, -0.05),
                        vec3(0.12, 0.06, 0.12),
                        c(0.65, 0.55, 0.45),
                    );
                }
                fr.cube(
                    b,
                    vec3(0.0, 1.4 + y, 0.175),
                    vec3(0.03, 0.15, 0.01),
                    LEATHER,
                );
                let haft = fr.limb(b, rhand, wa, 0.035, 1.2, WOOD);
                // A double-bitted head.
                fr.limb(
                    b,
                    haft - vec3(0.0, 0.0, 0.0),
                    wa,
                    0.04,
                    0.08,
                    dark(STEEL, 0.7),
                );
                fr.tilted(
                    b,
                    haft + vec3(0.0, 0.0, 0.0),
                    vec3(0.025, 0.2, 0.15),
                    wa - 1.57,
                    STEEL,
                );
                fr.tilted(b, haft, vec3(0.03, 0.06, 0.16), wa - 1.57, dark(STEEL, 0.8));
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
                fr.cube(b, vec3(0.0, 1.38 + y, 0.18), vec3(0.06, 0.06, 0.005), GOLD);
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
                fr.cube(b, head, vec3(0.105, 0.03, 0.165), GOLD);
                if pose.casting {
                    fr.glow(b, head, 0.18, Color::new(1.0, 0.95, 0.6, 0.6));
                }
                shield(b, fr, lhand, c(0.9, 0.88, 0.8), GOLD);
            }
            Class::Monk => {
                let red = c(0.85, 0.15, 0.12);
                fr.cube(
                    b,
                    vec3(0.0, 1.07 + y, 0.0),
                    vec3(h.chest_w - 0.04, 0.05, 0.16),
                    red,
                );
                let flutter = (pose.time * 4.0).sin() * 0.08;
                fr.tilted(
                    b,
                    vec3(0.12, 0.9 + y, 0.15),
                    vec3(0.04, 0.15, 0.01),
                    0.1 + flutter,
                    red,
                );
                fr.sphere(b, rhand, 0.085, c(0.9, 0.88, 0.8));
                fr.sphere(b, lhand, 0.085, c(0.9, 0.88, 0.8));
                // Prayer beads.
                for k in 0..7 {
                    let a = k as f32 * 0.45 - 1.35;
                    fr.sphere(
                        b,
                        vec3(a.sin() * 0.16, 1.45 + y - a.cos() * 0.06, 0.17),
                        0.022,
                        c(0.45, 0.28, 0.15),
                    );
                }
            }
            Class::Rogue => {
                fr.cube(
                    b,
                    vec3(0.0, 1.36 + y, 0.0),
                    vec3(h.chest_w + 0.01, 0.025, 0.175),
                    LEATHER,
                );
                // Throwing knives on the bandolier.
                for k in 0..3 {
                    fr.cube(
                        b,
                        vec3(-0.12 + k as f32 * 0.08, 1.36 + y, 0.18),
                        vec3(0.012, 0.05, 0.006),
                        STEEL,
                    );
                }
                for (hand, angle) in [(rhand, wa), (lhand, la)] {
                    let hilt = fr.limb(b, hand, angle, 0.025, 0.1, c(0.2, 0.12, 0.08));
                    fr.limb(b, hilt, angle, 0.08, 0.03, STEEL);
                    fr.limb(b, hilt, angle, 0.03, 0.42, c(0.82, 0.84, 0.88));
                }
            }
            Class::Ranger => {
                // A longbow in the left hand and a quiver on the back. The
                // string pulls back to the right hand while drawing.
                let top = lhand + vec3(-0.02, 0.75, 0.05);
                let bottom = lhand + vec3(-0.02, -0.75, 0.05);
                let bend = lhand + vec3(-0.02, 0.0, 0.22);
                fr.beam(b, top, bend, 0.025, WOOD);
                fr.beam(b, bend, bottom, 0.025, WOOD);
                let drawing = pose.casting || (pose.swing > 0.0 && pose.swing < 0.6);
                let string = if drawing {
                    rhand
                } else {
                    lhand + vec3(-0.02, 0.0, 0.0)
                };
                fr.beam(b, top, string, 0.006, c(0.9, 0.9, 0.85));
                fr.beam(b, string, bottom, 0.006, c(0.9, 0.9, 0.85));
                if drawing {
                    let tip = rhand + (lhand - rhand).normalize_or_zero() * 0.9;
                    fr.beam(b, rhand, tip, 0.012, WOOD);
                    fr.cone_dir(
                        b,
                        tip,
                        (lhand - rhand).normalize_or_zero() * 0.08,
                        0.025,
                        STEEL,
                    );
                }
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
                    // Puffs of steam from the pipes.
                    let t = (pose.time * 0.8 + sx).fract();
                    fr.glow(
                        b,
                        vec3(0.12 * sx, 1.85 + y + t * 0.4, -0.3),
                        0.04 + t * 0.06,
                        Color::new(0.9, 0.9, 0.9, 0.5 * (1.0 - t)),
                    );
                }
                fr.glow(b, vec3(0.0, 1.35 + y, -0.38), 0.06, c(0.5, 0.85, 1.0));
                let stock = fr.limb(b, rhand, wa, 0.05, 0.25, WOOD);
                let muzzle = fr.limb(b, stock, wa, 0.03, 0.55, c(0.35, 0.35, 0.38));
                fr.limb(b, stock, wa, 0.045, 0.12, c(0.7, 0.55, 0.25));
                if pose.swing > 0.3 && pose.swing < 0.45 {
                    fr.glow(b, muzzle, 0.12, c(0.6, 0.9, 1.0));
                }
            }
            Class::Bard => {
                // A lute held across the body.
                let neck_end = lhand + vec3(0.25, 0.6, 0.15);
                fr.ellipsoid(
                    b,
                    rhand + vec3(-0.08, 0.05, 0.08),
                    vec3(0.16, 0.2, 0.06),
                    c(0.6, 0.38, 0.18),
                );
                fr.sphere(b, rhand + vec3(-0.08, 0.05, 0.14), 0.05, c(0.2, 0.12, 0.08));
                fr.beam(
                    b,
                    rhand + vec3(-0.08, 0.2, 0.1),
                    neck_end,
                    0.025,
                    c(0.35, 0.22, 0.12),
                );
                fr.cube(b, vec3(0.0, 1.3 + y, 0.175), vec3(0.04, 0.2, 0.01), GOLD);
                // Notes drifting up while playing.
                if pose.casting || (pose.swing > 0.0 && pose.swing < 1.0) {
                    for k in 0..3 {
                        let t = (pose.time * 0.7 + k as f32 / 3.0).fract();
                        let p = rhand + vec3((t * 9.0).sin() * 0.2, 0.3 + t * 1.2, 0.2);
                        fr.glow(b, p, 0.04, Color::new(0.95, 0.6, 1.0, 1.0 - t));
                    }
                }
            }
            Class::Cleric => {
                fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.07, 0.3, 0.01), GOLD);
                fr.cube(b, vec3(0.0, 1.38 + y, 0.18), vec3(0.14, 0.035, 0.01), GOLD);
                let head = fr.limb(b, rhand, wa, 0.03, 0.6, WOOD);
                fr.sphere(b, head, 0.11, c(0.82, 0.82, 0.85));
                for k in 0..4 {
                    let a = k as f32 * 1.57;
                    fr.cone_dir(
                        b,
                        head,
                        vec3(a.cos() * 0.15, 0.0, a.sin() * 0.15),
                        0.03,
                        c(0.82, 0.82, 0.85),
                    );
                }
            }
            Class::Druid => {
                fr.cube(
                    b,
                    rhand + vec3(0.0, 0.25, 0.0),
                    vec3(0.035, 0.95, 0.035),
                    c(0.45, 0.32, 0.2),
                );
                for k in 0..4 {
                    let a = k as f32 * 1.57 + pose.time * 0.3;
                    fr.sphere(
                        b,
                        rhand + vec3(a.cos() * 0.08, 1.2, a.sin() * 0.08),
                        0.07,
                        c(0.35, 0.65, 0.25),
                    );
                }
                fr.glow(b, rhand + vec3(0.0, 1.25, 0.0), 0.04, c(0.8, 1.0, 0.6));
                // Leaves swirling while casting.
                if pose.casting {
                    for k in 0..6 {
                        let a = pose.time * 2.5 + k as f32 * 1.05;
                        let p = vec3(
                            a.cos() * 0.7,
                            0.8 + y + (a * 0.7).sin() * 0.5,
                            a.sin() * 0.7,
                        );
                        fr.cube(b, p, vec3(0.05, 0.01, 0.03), c(0.4, 0.75, 0.3));
                    }
                }
            }
            Class::Mage => {
                fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.04, 0.27, 0.01), GOLD);
                fr.cube(b, rhand + vec3(0.0, 0.3, 0.0), vec3(0.03, 0.95, 0.03), WOOD);
                fr.cube(
                    b,
                    rhand + vec3(0.0, 1.18, 0.0),
                    vec3(0.06, 0.04, 0.06),
                    GOLD,
                );
                let orb = if pose.casting { 0.14 } else { 0.1 };
                fr.glow(b, rhand + vec3(0.0, 1.32, 0.0), orb, c(0.55, 0.85, 1.0));
            }
            Class::Sorcerer => {
                fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.04, 0.27, 0.01), GOLD);
                // Orbs of wild magic circling, faster while casting.
                let speed = if pose.casting { 6.0 } else { 2.0 };
                for k in 0..3 {
                    let a = pose.time * speed + k as f32 * 2.09;
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
                fr.cube(b, vec3(0.18, 1.0 + y, 0.13), vec3(0.06, 0.07, 0.06), BONE);
                // A grimoire on a chain at the hip.
                fr.cube(
                    b,
                    vec3(-0.22, 0.98 + y, 0.05),
                    vec3(0.03, 0.1, 0.08),
                    c(0.3, 0.1, 0.25),
                );
                let flame = Color::new(0.4, 1.0, 0.3, 0.9);
                let flicker = (pose.time * 7.0).sin().abs();
                fr.glow(
                    b,
                    rhand + vec3(0.0, 0.05, 0.1),
                    0.08 + flicker * 0.02,
                    flame,
                );
                fr.glow(
                    b,
                    rhand + vec3(0.0, 0.17 + flicker * 0.05, 0.1),
                    0.04,
                    flame,
                );
                fr.glow(b, lhand + vec3(0.0, 0.05, 0.1), 0.06, flame);
            }
        },
        Outfit::Merchant => {
            fr.cube(
                b,
                vec3(0.0, 1.05 + y, 0.17),
                vec3(0.2, 0.3, 0.01),
                c(0.88, 0.85, 0.75),
            );
            fr.sphere(b, vec3(0.2, 1.0 + y, 0.12), 0.07, c(0.55, 0.4, 0.2));
            fr.cube(
                b,
                vec3(-0.12, 1.12 + y, 0.18),
                vec3(0.04, 0.05, 0.005),
                c(0.6, 0.6, 0.6),
            );
        }
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Bandit | HumanoidStyle::Raider | HumanoidStyle::Trogg => {
                let hilt = fr.limb(b, rhand, wa, 0.025, 0.1, WOOD);
                if style == HumanoidStyle::Trogg || look.seed.is_multiple_of(2) {
                    let end = fr.limb(b, hilt, wa, 0.07, 0.6, c(0.35, 0.24, 0.14));
                    fr.sphere(b, end, 0.08, c(0.3, 0.2, 0.12));
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
                    // Cracks.
                    fr.cube(
                        b,
                        vec3(0.1, 1.05 + y, 0.155),
                        vec3(0.008, 0.08, 0.004),
                        dark(colors[0], 0.5),
                    );
                }
                GiantStyle::Treant => {
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
                    // Icicles in the fur.
                    for k in 0..4 {
                        fr.cone_dir(
                            b,
                            vec3(-0.15 + k as f32 * 0.1, 1.1 + y, 0.2),
                            vec3(0.0, -0.12, 0.0),
                            0.02,
                            c(0.8, 0.9, 1.0),
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
            // A slam shakes up dust.
            if pose.swing > 0.4 && pose.swing < 0.75 {
                let t = (pose.swing - 0.4) / 0.35;
                for k in 0..6 {
                    let a = k as f32 * 1.05;
                    let p = vec3(a.cos(), 0.1, a.sin() + 0.8) * (0.3 + t * 0.6);
                    fr.glow(
                        b,
                        p,
                        0.1 + t * 0.1,
                        Color::new(0.7, 0.65, 0.55, 0.6 * (1.0 - t)),
                    );
                }
            }
        }
    }
}

// ---- Creatures ----

/// An animal's frame: fallen when dead, rearing (pitched) when attacking.
fn beast_frame(pos: Vec3, yaw: f32, pose: Pose, rear: f32) -> Frame {
    if pose.dead {
        Frame::fallen(pos, yaw, 1.0, true)
    } else {
        Frame::leaning(pos, yaw, 1.0, -rear - pose.hurt * 0.12)
    }
}

fn wolf(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], seed: u32, pose: Pose) {
    let bite = if pose.attacking() {
        bump(pose.swing)
    } else {
        0.0
    };
    let fr = beast_frame(pos, yaw, pose, bite * 0.12);
    let tint = (seed % 3) as f32 * 0.04;
    let fur = Color::new(colors[0].r + tint, colors[0].g + tint, colors[0].b, 1.0);
    let back = colors[1];
    let belly = mix(colors[0], c(0.85, 0.82, 0.78), 0.5);
    let moving = pose.moving && !pose.dead;
    let gait = if moving { pose.walk } else { 0.0 };
    let lunge = bite * 0.3;
    let breathe = (pose.time * 2.0).sin() * 0.01;
    let y = 0.78
        + if moving {
            (gait * 2.0).sin().abs() * 0.05
        } else {
            breathe
        };
    fr.cube(b, vec3(0.0, y, 0.22), vec3(0.23, 0.24 + breathe, 0.32), fur);
    fr.cube(b, vec3(0.0, y + 0.02, -0.32), vec3(0.19, 0.2, 0.28), fur);
    fr.cube(b, vec3(0.0, y + 0.23, -0.05), vec3(0.17, 0.05, 0.55), back);
    fr.cube(b, vec3(0.0, y - 0.2, 0.15), vec3(0.17, 0.06, 0.3), belly);
    fr.cube(b, vec3(0.0, y + 0.08, 0.5), vec3(0.25, 0.25, 0.1), back);
    // A shaggy mane.
    for k in 0..5 {
        let z = 0.55 - k as f32 * 0.12;
        fr.cone_dir(
            b,
            vec3(0.0, y + 0.25, z),
            vec3(0.0, 0.12, -0.1),
            0.06,
            dark(back, 0.85),
        );
        for sx in [-1.0, 1.0] {
            fr.cone_dir(
                b,
                vec3(0.2 * sx, y + 0.12, z),
                vec3(0.08 * sx, 0.02, -0.1),
                0.05,
                dark(fur, 0.9),
            );
        }
    }
    // Front legs move together at a gallop, the hind legs half a beat behind.
    for (x, z, phase) in [
        (-1.0, 1.0, 0.0),
        (1.0, 1.0, 0.5),
        (-1.0, -1.0, 3.1),
        (1.0, -1.0, 3.6),
    ] {
        let swing = if moving {
            (gait + phase).sin() * 0.6
        } else {
            0.0
        };
        let bend = if moving {
            (gait + phase + 1.0).sin().max(0.0) * 0.7
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
        for k in 0..3 {
            let cx = (k as f32 - 1.0) * 0.03;
            fr.cone_dir(
                b,
                ankle + vec3(cx, -0.02, 0.11),
                vec3(0.0, -0.02, 0.04),
                0.01,
                c(0.15, 0.13, 0.12),
            );
        }
    }
    // The head sniffs about when idle.
    let sniff = if moving || pose.dead {
        0.0
    } else {
        ((pose.time * 0.6 + seed as f32).sin() * 3.0).clamp(-1.0, 0.0) * 0.12
    };
    let head = vec3(0.0, y + 0.22 + sniff + bite * 0.04, 0.72 + lunge);
    fr.cube(b, head, vec3(0.16, 0.15, 0.17), fur);
    fr.cube(
        b,
        head + vec3(0.0, -0.03, 0.24),
        vec3(0.08, 0.05, 0.13),
        fur,
    );
    // The jaw opens to bite.
    let jaw = 0.25 + bite * 0.6;
    fr.tilted(
        b,
        head + vec3(0.0, -0.1, 0.2),
        vec3(0.07, 0.025, 0.12),
        jaw,
        belly,
    );
    fr.sphere(b, head + vec3(0.0, 0.0, 0.38), 0.04, c(0.08, 0.07, 0.07));
    if bite > 0.2 {
        fr.cube(
            b,
            head + vec3(0.0, -0.08, 0.26),
            vec3(0.05, 0.02, 0.06),
            c(0.6, 0.2, 0.25),
        );
    }
    for sx in [-1.0, 1.0] {
        fr.glow(b, head + vec3(0.08 * sx, 0.05, 0.16), 0.032, colors[2]);
        let ear_twitch = if (pose.time * 1.3 + sx).sin() > 0.95 {
            0.04
        } else {
            0.0
        };
        fr.cone_dir(
            b,
            head + vec3(0.09 * sx, 0.12, -0.04),
            vec3(0.03 * sx, 0.17, -0.03 - ear_twitch),
            0.06,
            back,
        );
        fr.cone_dir(
            b,
            head + vec3(0.05 * sx, -0.08, 0.33),
            vec3(0.0, -0.06, 0.0),
            0.015,
            c(0.95, 0.94, 0.88),
        );
    }
    let wag = (pose.time * if moving { 8.0 } else { 4.0 }).sin() * 0.25;
    let t1 = fr.limb(b, vec3(0.0, y + 0.12, -0.58), 2.2 + wag, 0.07, 0.25, fur);
    let t2 = fr.limb(b, t1, 2.6 + wag * 1.3, 0.09, 0.25, back);
    fr.sphere(b, t2, 0.07, belly);
}

fn boar(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], seed: u32, pose: Pose) {
    let toss = if pose.attacking() {
        bump(pose.swing)
    } else {
        0.0
    };
    let fr = beast_frame(pos, yaw, pose, toss * 0.1);
    let tint = (seed % 3) as f32 * 0.03;
    let hide = Color::new(colors[0].r + tint, colors[0].g, colors[0].b, 1.0);
    let dark_hide = colors[1];
    let snout = mix(hide, c(0.85, 0.55, 0.5), 0.6);
    let tusk = colors[2];
    let moving = pose.moving && !pose.dead;
    let gait = if moving { pose.walk * 1.3 } else { 0.0 };
    let lunge = toss * 0.3;
    let breathe = (pose.time * 1.8).sin() * 0.012;
    let y = 0.68
        + if moving {
            (gait * 2.0).sin().abs() * 0.03
        } else {
            breathe
        };
    // The body is built along the boar's own axes, so it turns with the boar.
    fr.ellipsoid(b, vec3(0.0, y, 0.0), vec3(0.34, 0.33 + breathe, 0.62), hide);
    fr.ellipsoid(b, vec3(0.0, y + 0.05, 0.38), vec3(0.32, 0.33, 0.3), hide);
    // Darker patches.
    fr.ellipsoid(
        b,
        vec3(0.12, y + 0.1, -0.15),
        vec3(0.24, 0.22, 0.3),
        dark(hide, 0.85),
    );
    for i in 0..8 {
        let z = 0.5 - i as f32 * 0.13;
        let top = y + 0.3 - (i as f32 - 2.5).abs() * 0.015;
        fr.cone_dir(
            b,
            vec3(0.0, top, z),
            vec3(0.0, 0.16, -0.07),
            0.055,
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
        // Split hooves.
        for sx in [-1.0, 1.0] {
            fr.cube(
                b,
                ankle + vec3(0.03 * sx, 0.0, 0.01),
                vec3(0.03, 0.04, 0.06),
                c(0.15, 0.12, 0.1),
            );
        }
    }
    // Rooting about with the snout when idle.
    let root = if moving || pose.dead {
        0.0
    } else {
        (((pose.time * 0.5 + seed as f32 * 0.3).sin() * 2.0).clamp(-1.0, 0.0)) * 0.15
    };
    let head = vec3(0.0, y - 0.02 + root + toss * 0.15, 0.72 + lunge);
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
            vec3(0.05 * sx, 0.1 + toss * 0.05, 0.1),
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

fn spider(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], seed: u32, pose: Pose) {
    let rear = if pose.attacking() {
        bump(pose.swing)
    } else {
        0.0
    };
    let fr = beast_frame(pos, yaw, pose, rear * 0.35);
    let moving = pose.moving && !pose.dead;
    let lunge = rear * 0.2;
    let breathe = (pose.time * 2.2).sin() * 0.015;
    let y = 0.6
        + if moving {
            (pose.walk * 3.0).sin().abs() * 0.03
        } else {
            0.0
        };
    fr.ellipsoid(
        b,
        vec3(0.0, y, 0.15 + lunge),
        vec3(0.3, 0.22, 0.32),
        colors[0],
    );
    fr.ellipsoid(
        b,
        vec3(0.0, y + 0.15, -0.5),
        vec3(0.45, 0.38 + breathe, 0.55),
        colors[1],
    );
    // Markings and bristles on the abdomen.
    fr.ellipsoid(
        b,
        vec3(0.0, y + 0.5, -0.5),
        vec3(0.12, 0.04, 0.2),
        colors[2],
    );
    for k in 0..6 {
        let a = k as f32 * 1.05;
        fr.cone_dir(
            b,
            vec3(a.cos() * 0.3, y + 0.35, -0.5 + a.sin() * 0.35),
            vec3(a.cos() * 0.05, 0.1, a.sin() * 0.05),
            0.02,
            dark(colors[1], 0.6),
        );
    }
    for k in 0..4 {
        for sx in [-1.0f32, 1.0] {
            let phase = k as f32 * 1.6 + if sx > 0.0 { 3.1 } else { 0.0 };
            let lift = if moving {
                (pose.walk * 1.5 + phase).sin().max(0.0) * 0.2
            } else {
                // An idle twitch now and then.
                ((pose.time * 1.7 + phase + seed as f32).sin() - 0.9).max(0.0) * 1.2
            };
            let z = 0.35 - k as f32 * 0.22;
            // The front legs rise to strike.
            let raise = if k == 0 { rear * 0.6 } else { 0.0 };
            let hip = vec3(0.22 * sx, y, z + lunge);
            let knee = vec3(
                0.75 * sx,
                y + 0.45 + lift + raise,
                z * 1.4 + 0.05 + raise * 0.3,
            );
            let foot = vec3(1.05 * sx, raise * 1.2, z * 1.7 + 0.1 + raise * 0.6);
            fr.beam(b, hip, knee, 0.045, colors[0]);
            fr.beam(b, knee, foot, 0.035, colors[1]);
            fr.sphere(b, knee, 0.05, dark(colors[0], 0.8));
        }
    }
    let chew = (pose.time * 6.0).sin() * 0.02 + rear * 0.04;
    for sx in [-1.0, 1.0] {
        fr.glow(b, vec3(0.08 * sx, y + 0.12, 0.45 + lunge), 0.04, colors[2]);
        fr.glow(b, vec3(0.16 * sx, y + 0.08, 0.4 + lunge), 0.03, colors[2]);
        fr.glow(b, vec3(0.05 * sx, y + 0.18, 0.42 + lunge), 0.022, colors[2]);
        fr.cone_dir(
            b,
            vec3((0.07 + chew) * sx, y - 0.05, 0.45 + lunge),
            vec3(-chew * sx, -0.15, 0.08),
            0.03,
            c(0.1, 0.08, 0.08),
        );
    }
}

fn scorpion(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], pose: Pose) {
    let strike = if pose.attacking() {
        bump(pose.swing)
    } else {
        0.0
    };
    let fr = beast_frame(pos, yaw, pose, 0.0);
    let moving = pose.moving && !pose.dead;
    let y = 0.45;
    fr.ellipsoid(b, vec3(0.0, y, 0.0), vec3(0.35, 0.16, 0.55), colors[0]);
    for k in 0..4 {
        let kf = k as f32;
        fr.cube(
            b,
            vec3(0.0, y + 0.12, 0.36 - kf * 0.24),
            vec3(0.3 - kf * 0.03, 0.04, 0.09),
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
    // Pincers snap open and shut.
    let snap = ((pose.time * 2.0).sin() * 0.5 + 0.5) * 0.06 + strike * 0.08;
    for sx in [-1.0f32, 1.0] {
        let elbow = vec3(0.4 * sx, y + 0.1, 0.65);
        let claw = vec3(0.3 * sx, y + 0.1 + strike * 0.1, 1.0 + strike * 0.25);
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
            claw + vec3((0.04 + snap) * sx, 0.0, 0.22),
            vec3(-0.04 * sx, 0.0, 0.15),
            0.04,
            colors[1],
        );
        fr.cone_dir(
            b,
            claw + vec3((0.0 - snap) * sx, 0.0, 0.22),
            vec3(0.03 * sx, 0.0, 0.13),
            0.03,
            dark(colors[1], 0.8),
        );
    }
    // A tail curling over the back, swaying, then striking forward.
    let sway = if moving {
        0.0
    } else {
        (pose.time * 1.5).sin() * 0.08
    };
    let mut p = vec3(0.0, y + 0.05, -0.5);
    for k in 0..6 {
        let a = 0.2 + k as f32 * (0.42 + strike * 0.12) + sway;
        let next = p + vec3(sway * 0.1 * k as f32, a.sin() * 0.22, -a.cos() * 0.22);
        let col = if k % 2 == 0 { colors[0] } else { colors[1] };
        fr.sphere(b, next, 0.11 - k as f32 * 0.008, col);
        p = next;
    }
    fr.sphere(b, p + vec3(0.0, 0.02, 0.06), 0.08, colors[1]);
    fr.cone_dir(
        b,
        p + vec3(0.0, 0.0, 0.08),
        vec3(0.0, -0.08, 0.25),
        0.05,
        colors[2],
    );
    fr.glow(
        b,
        p + vec3(0.0, -0.06, 0.3),
        0.025,
        Color::new(0.6, 1.0, 0.3, 0.8),
    );
    for sx in [-1.0, 1.0] {
        fr.sphere(b, vec3(0.08 * sx, y + 0.12, 0.5), 0.03, c(0.05, 0.05, 0.05));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose() -> Pose {
        Pose {
            time: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn classes_attack_and_cast_in_their_own_way() {
        let styles = [
            Style::TwoHander,
            Style::SwordBoard,
            Style::Fists,
            Style::Daggers,
            Style::Bow,
            Style::Rifle,
            Style::Lute,
            Style::Staff,
        ];
        let attack = Pose {
            swing: 0.3,
            ..pose()
        };
        let mut seen: Vec<[f32; 4]> = Vec::new();
        for s in styles {
            let a = animate(s, attack);
            let sig = [a.arms[0].swing, a.arms[1].swing, a.arms[1].bend, a.lean];
            assert!(
                seen.iter()
                    .all(|o| o.iter().zip(&sig).any(|(x, y)| (x - y).abs() > 0.05)),
                "{s:?} attacks like another style"
            );
            seen.push(sig);
        }
        let cast = Pose {
            casting: true,
            ..pose()
        };
        // Clerics raise their arms, druids spread them, mages lift a staff.
        assert!(animate(Style::Holy, cast).arms[0].swing > 2.0);
        assert!(animate(Style::Nature, cast).arms[0].spread > 1.0);
        assert!(animate(Style::Staff, cast).arms[1].swing > 2.0);
    }

    #[test]
    fn monks_alternate_punches_and_kick() {
        let mut p = Pose {
            swing: 0.3,
            ..pose()
        };
        let left = animate(Style::Fists, p);
        p.combo = 1;
        let right = animate(Style::Fists, p);
        p.combo = 2;
        let kick = animate(Style::Fists, p);
        assert!(left.arms[0].swing > right.arms[0].swing);
        assert!(right.arms[1].swing > left.arms[1].swing);
        assert!(kick.legs[1].swing > 0.8);
    }
}
