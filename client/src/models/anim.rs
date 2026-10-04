//! Animation: how each fighting style stands, swings and casts.

use super::*;

/// How someone fights, which decides how they stand, swing and cast.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Style {
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

pub(super) fn style_of(outfit: Outfit) -> Style {
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
        Outfit::Merchant | Outfit::QuestGiver => Style::Merchant,
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
pub(super) struct Limb {
    pub(super) swing: f32,
    pub(super) spread: f32,
    pub(super) bend: f32,
}

pub(super) const fn limb(swing: f32, spread: f32, bend: f32) -> Limb {
    Limb {
        swing,
        spread,
        bend,
    }
}

/// A whole-body pose.
#[derive(Clone, Copy, Default, Debug)]
pub(super) struct Anim {
    /// Forward lean from the feet.
    pub(super) lean: f32,
    /// How far the body sinks (bent knees).
    pub(super) crouch: f32,
    /// Bounce of the body.
    pub(super) bob: f32,
    /// Left, right.
    pub(super) arms: [Limb; 2],
    pub(super) legs: [Limb; 2],
}

/// A smooth 0-1-0 bump over `t` in `0..1`.
pub(super) fn bump(t: f32) -> f32 {
    (t.clamp(0.0, 1.0) * std::f32::consts::PI).sin()
}

/// Windup then strike: 0 to 1 over the first `split` of `t`, then back down.
pub(super) fn windup(t: f32, split: f32) -> (f32, f32) {
    if t < split {
        let k = t / split;
        (k * k * (3.0 - 2.0 * k), 0.0)
    } else {
        let k = ((t - split) / (1.0 - split)).min(1.0);
        (1.0 - k, k)
    }
}

pub(super) fn animate(style: Style, pose: Pose) -> Anim {
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
pub(super) fn limb_dir(swing: f32, spread: f32, side: f32) -> Vec3 {
    vec3(
        side * spread.sin(),
        -swing.cos() * spread.cos(),
        swing.sin() * spread.cos(),
    )
}
