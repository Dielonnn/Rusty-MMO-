//! Character and creature models, and how they move.
//!
//! People (players, townsfolk and humanoid mobs) are rigged, animated models
//! (`rigged`): a male or female body (or a skeleton) on a shared skeleton,
//! with class gear painted on for race, class and armor, and animated by how they fight: a barbarian
//! chops with a great axe, a monk throws punches and kicks, a rogue stabs
//! with two knives, casters channel and throw spells, and so on.
//!
//! Creatures and giants are still built from shaded primitives in their own
//! `Frame`, so all of it turns together, posed by an `Anim` (angles for each
//! limb, a lean and a crouch). They become models too in a later release.

use macroquad::prelude::*;
use shared::data::{
    Appearance, Class, GiantStyle, HumanoidStyle, ItemId, MobModel, Race, Slot, item, items,
};
use shared::protocol::EntityKind;

use crate::gfx::{Batch, Frame, c, dark, mix, rgb};

mod anim;
mod creatures;
mod gear;
mod humanoid;
mod rigged;

use anim::*;
use creatures::*;
use gear::*;
use humanoid::*;

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
        EntityKind::Player(_) | EntityKind::Merchant(_) | EntityKind::QuestGiver(_) => {
            2.1 * race_shape(appearance.race).scale
        }
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
    draw_body(b, look, pos, yaw, pose);
    // After the body, so the shadow doesn't hide the bottom of the feet.
    b.blob_shadow(pos, model_radius(look.kind) * 0.9, 0.45);
}

fn draw_body(b: &mut Batch, look: &Look, pos: Vec3, yaw: f32, pose: Pose) {
    match look.kind {
        EntityKind::Player(class) => rigged::draw(b, look, Outfit::Class(class), pos, yaw, pose),
        EntityKind::Merchant(_) => rigged::draw(b, look, Outfit::Merchant, pos, yaw, pose),
        EntityKind::QuestGiver(_) => rigged::draw(b, look, Outfit::QuestGiver, pos, yaw, pose),
        EntityKind::Mob { kind, .. } => {
            let t = kind.template();
            let colors = t.colors.map(rgb);
            match t.model {
                MobModel::Wolf => wolf(b, pos, yaw, colors, look.seed, pose),
                MobModel::Boar => boar(b, pos, yaw, colors, look.seed, pose),
                MobModel::Spider => spider(b, pos, yaw, colors, look.seed, pose),
                MobModel::Scorpion => scorpion(b, pos, yaw, colors, pose),
                MobModel::Humanoid(style) => {
                    rigged::draw(b, look, Outfit::Mob(style, colors), pos, yaw, pose)
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
    /// A townsperson with quests: an officer in a tabard, with a scroll.
    QuestGiver,
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
