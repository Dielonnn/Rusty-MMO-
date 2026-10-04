//! Game data: classes, abilities, mobs, items, recipes and the numbers that
//! balance them.

use serde::{Deserialize, Serialize};

pub const MAX_LEVEL: u8 = 10;
/// The global cooldown every ability triggers.
pub const GCD: f32 = 1.5;
pub const MELEE_RANGE: f32 = 4.0;
pub const RUN_SPEED: f32 = 7.0;
pub const BACKPEDAL_SPEED: f32 = 4.0;
pub const CRIT_CHANCE: f32 = 0.1;
pub const CRIT_MULTIPLIER: f32 = 1.5;
/// Mobs that get this far from where they spawned give up and run home.
pub const LEASH_RANGE: f32 = 45.0;
/// Mana only regenerates at full speed this long after spending it.
pub const MANA_REGEN_DELAY: f32 = 5.0;
/// Energy comes back this fast, in or out of combat.
pub const ENERGY_PER_SECOND: f32 = 10.0;
pub const MAX_COMBO_POINTS: u8 = 5;
/// Maximum number of abilities on a class's action bar.
pub const ACTION_BAR_SLOTS: usize = 6;
/// The level each action bar slot's ability is learned at.
pub const UNLOCK_LEVELS: [u8; ACTION_BAR_SLOTS] = [1, 2, 4, 6, 8, 10];
pub const BAG_SLOTS: usize = 20;
/// How close you have to be to loot a corpse.
pub const LOOT_RANGE: f32 = 6.0;
/// Extra health per point of stamina.
pub const HP_PER_STAMINA: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Class {
    Warrior,
    Mage,
    Cleric,
    Rogue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerKind {
    /// Starts empty, builds up by fighting and drains out of combat.
    Rage,
    /// Starts full and regenerates.
    Mana,
    /// Starts full and comes back quickly.
    Energy,
}

impl PowerKind {
    pub fn name(self) -> &'static str {
        match self {
            PowerKind::Rage => "rage",
            PowerKind::Mana => "mana",
            PowerKind::Energy => "energy",
        }
    }
}

/// A class's basic attack, swung automatically while attacking a target.
#[derive(Clone, Copy, Debug)]
pub struct AutoAttack {
    pub range: f32,
    pub interval: f32,
    pub min: f32,
    pub max: f32,
}

impl Class {
    pub const ALL: [Class; 4] = [Class::Warrior, Class::Mage, Class::Cleric, Class::Rogue];

    pub fn name(self) -> &'static str {
        match self {
            Class::Warrior => "Warrior",
            Class::Mage => "Mage",
            Class::Cleric => "Cleric",
            Class::Rogue => "Rogue",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Class::Warrior => {
                "A sturdy melee fighter. Builds rage by dealing and taking damage, holds enemies' attention with Taunt and Thunder Clap, and interrupts casters with Shield Bash."
            }
            Class::Mage => {
                "A fragile spellcaster with the highest damage. Burns enemies with fire, slows and roots them with frost, and shields itself with Ice Barrier."
            }
            Class::Cleric => {
                "A healer who can hold their own. Heals and shields allies, and wears enemies down with Smite and Shadow Word: Pain."
            }
            Class::Rogue => {
                "A quick melee fighter who runs on energy. Builds combo points with Sinister Strike and Backstab, then spends them on a deadly Eviscerate."
            }
        }
    }

    pub fn power_kind(self) -> PowerKind {
        match self {
            Class::Warrior => PowerKind::Rage,
            Class::Mage | Class::Cleric => PowerKind::Mana,
            Class::Rogue => PowerKind::Energy,
        }
    }

    /// Health from the class and level alone, before gear.
    pub fn max_hp(self, level: u8) -> f32 {
        let l = (level.max(1) - 1) as f32;
        match self {
            Class::Warrior => 140.0 + 35.0 * l,
            Class::Mage => 90.0 + 22.0 * l,
            Class::Cleric => 100.0 + 25.0 * l,
            Class::Rogue => 115.0 + 28.0 * l,
        }
    }

    pub fn max_power(self, level: u8) -> f32 {
        let l = (level.max(1) - 1) as f32;
        match self {
            Class::Warrior | Class::Rogue => 100.0,
            Class::Mage => 120.0 + 18.0 * l,
            Class::Cleric => 130.0 + 18.0 * l,
        }
    }

    /// What a new character's power bar starts at.
    pub fn starting_power(self, level: u8) -> f32 {
        match self.power_kind() {
            PowerKind::Rage => 0.0,
            _ => self.max_power(level),
        }
    }

    pub fn auto_attack(self) -> AutoAttack {
        match self {
            Class::Warrior => AutoAttack {
                range: MELEE_RANGE,
                interval: 2.2,
                min: 8.0,
                max: 12.0,
            },
            Class::Rogue => AutoAttack {
                range: MELEE_RANGE,
                interval: 1.6,
                min: 5.0,
                max: 8.0,
            },
            // Casters shoot a wand.
            Class::Mage | Class::Cleric => AutoAttack {
                range: 25.0,
                interval: 1.8,
                min: 3.0,
                max: 5.0,
            },
        }
    }

    /// The class's action bar, in key order. Slot `i` is learned at
    /// `UNLOCK_LEVELS[i]`.
    pub fn abilities(self) -> [AbilityId; ACTION_BAR_SLOTS] {
        use ids::*;
        match self {
            Class::Warrior => [
                HEROIC_STRIKE,
                REND,
                THUNDER_CLAP,
                SHIELD_BASH,
                TAUNT,
                SECOND_WIND,
            ],
            Class::Mage => [
                FIREBALL,
                FROSTBOLT,
                FIRE_BLAST,
                FROST_NOVA,
                ICE_BARRIER,
                EVOCATION,
            ],
            Class::Cleric => [
                SMITE,
                HEAL,
                SHADOW_WORD_PAIN,
                RENEW,
                POWER_WORD_SHIELD,
                HOLY_NOVA,
            ],
            Class::Rogue => [SINISTER_STRIKE, EVISCERATE, BACKSTAB, GOUGE, EVASION, KICK],
        }
    }

    /// The level this class learns an ability at, if it ever does.
    pub fn unlock_level(self, ability: AbilityId) -> Option<u8> {
        self.abilities()
            .iter()
            .position(|a| *a == ability)
            .map(|i| UNLOCK_LEVELS[i])
    }

    /// Abilities known at a level.
    pub fn known(self, level: u8) -> impl Iterator<Item = AbilityId> {
        self.abilities()
            .into_iter()
            .zip(UNLOCK_LEVELS)
            .filter(move |(_, l)| *l <= level)
            .map(|(a, _)| a)
    }
}

/// How much stronger everything gets per level.
pub fn level_scale(level: u8) -> f32 {
    1.0 + 0.12 * (level.max(1) - 1) as f32
}

/// Experience needed to go from `level` to the next one. Zero at the cap.
pub fn xp_to_next(level: u8) -> u32 {
    if level >= MAX_LEVEL {
        0
    } else {
        100 + 85 * (level as u32 - 1)
    }
}

/// Experience for killing a mob. Mobs far below your level are worth nothing.
pub fn kill_xp(player_level: u8, mob_level: u8, elite: bool) -> u32 {
    let diff = mob_level as i32 - player_level as i32;
    if diff <= -5 || player_level >= MAX_LEVEL {
        return 0;
    }
    let base = 40.0 + 5.0 * mob_level as f32;
    let mult = (1.0 + 0.1 * diff as f32).clamp(0.2, 1.5);
    let elite_mult = if elite { 3.0 } else { 1.0 };
    (base * mult * elite_mult).round() as u32
}

/// Physical damage taken is multiplied by this.
pub fn armor_multiplier(armor: f32, attacker_level: u8) -> f32 {
    1.0 - armor / (armor + 100.0 + 25.0 * attacker_level as f32)
}

/// "1g 20s 5c".
pub fn format_money(copper: u32) -> String {
    let (g, s, c) = (copper / 10_000, copper / 100 % 100, copper % 100);
    let mut parts = Vec::new();
    if g > 0 {
        parts.push(format!("{g}g"));
    }
    if s > 0 {
        parts.push(format!("{s}s"));
    }
    if c > 0 || parts.is_empty() {
        parts.push(format!("{c}c"));
    }
    parts.join(" ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AbilityId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Targeting {
    /// Needs a hostile target.
    Enemy,
    /// A friendly target, or yourself if your target isn't friendly.
    Friendly,
    /// Always yourself.
    Caster,
    /// Every enemy within this radius of you.
    AroundCaster(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum School {
    Physical,
    Fire,
    Frost,
    Arcane,
    Holy,
    Shadow,
    Nature,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AuraKind {
    /// Damage every `interval` seconds (per stack).
    Dot { per_tick: f32, interval: f32 },
    /// Healing every `interval` seconds.
    Hot { per_tick: f32, interval: f32 },
    /// Movement speed is multiplied by this.
    Slow(f32),
    /// Can't move, act or cast.
    Stun,
    /// Can't move.
    Root,
    /// Soaks up this much damage.
    Absorb(f32),
    /// Damage taken is multiplied by this.
    DamageTaken(f32),
}

impl AuraKind {
    pub fn harmful(self) -> bool {
        matches!(
            self,
            AuraKind::Dot { .. } | AuraKind::Slow(_) | AuraKind::Stun | AuraKind::Root
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Damage {
        min: f32,
        max: f32,
    },
    Heal {
        min: f32,
        max: f32,
    },
    Aura {
        kind: AuraKind,
        duration: f32,
    },
    /// Stops the target's current cast.
    Interrupt,
    /// Makes a mob attack you.
    Taunt,
    /// Gives back this fraction of the caster's maximum power.
    RestorePower(f32),
    /// Adds a combo point (rogues).
    ComboPoint,
    /// Spends every combo point for damage.
    Finisher {
        min: f32,
        max: f32,
        per_point: f32,
    },
}

#[derive(Debug)]
pub struct Ability {
    pub name: &'static str,
    pub description: &'static str,
    pub school: School,
    pub targeting: Targeting,
    pub range: f32,
    pub cast_time: f32,
    pub cooldown: f32,
    pub cost: f32,
    pub effects: &'static [Effect],
    /// Threat per point of damage, relative to a normal hit.
    pub threat: f32,
    /// Drawn as a missile flying from the caster to the target.
    pub projectile: bool,
    /// How many times its aura stacks on one target.
    pub max_stacks: u8,
    /// Only usable from behind the target.
    pub from_behind: bool,
}

impl Ability {
    pub fn needs_combo_points(&self) -> bool {
        self.effects
            .iter()
            .any(|e| matches!(e, Effect::Finisher { .. }))
    }
}

pub fn ability(id: AbilityId) -> &'static Ability {
    &ABILITIES[id.0 as usize]
}

pub mod ids {
    use super::AbilityId;

    pub const HEROIC_STRIKE: AbilityId = AbilityId(0);
    pub const REND: AbilityId = AbilityId(1);
    pub const SHIELD_BASH: AbilityId = AbilityId(2);
    pub const THUNDER_CLAP: AbilityId = AbilityId(3);
    pub const TAUNT: AbilityId = AbilityId(4);
    pub const SECOND_WIND: AbilityId = AbilityId(5);

    pub const FIREBALL: AbilityId = AbilityId(6);
    pub const FROSTBOLT: AbilityId = AbilityId(7);
    pub const FIRE_BLAST: AbilityId = AbilityId(8);
    pub const FROST_NOVA: AbilityId = AbilityId(9);
    pub const ICE_BARRIER: AbilityId = AbilityId(10);
    pub const EVOCATION: AbilityId = AbilityId(11);

    pub const SMITE: AbilityId = AbilityId(12);
    pub const SHADOW_WORD_PAIN: AbilityId = AbilityId(13);
    pub const HEAL: AbilityId = AbilityId(14);
    pub const RENEW: AbilityId = AbilityId(15);
    pub const POWER_WORD_SHIELD: AbilityId = AbilityId(16);
    pub const HOLY_NOVA: AbilityId = AbilityId(17);

    // Used by mobs.
    pub const SAVAGE_BITE: AbilityId = AbilityId(18);
    pub const SHADOW_BOLT: AbilityId = AbilityId(19);
    pub const GROUND_SLAM: AbilityId = AbilityId(20);

    pub const SINISTER_STRIKE: AbilityId = AbilityId(21);
    pub const EVISCERATE: AbilityId = AbilityId(22);
    pub const BACKSTAB: AbilityId = AbilityId(23);
    pub const GOUGE: AbilityId = AbilityId(24);
    pub const EVASION: AbilityId = AbilityId(25);
    pub const KICK: AbilityId = AbilityId(26);
}

#[allow(clippy::too_many_arguments)]
const fn ab(
    name: &'static str,
    description: &'static str,
    school: School,
    targeting: Targeting,
    range: f32,
    cast_time: f32,
    cooldown: f32,
    cost: f32,
    effects: &'static [Effect],
) -> Ability {
    Ability {
        name,
        description,
        school,
        targeting,
        range,
        cast_time,
        cooldown,
        cost,
        effects,
        threat: 1.0,
        projectile: false,
        max_stacks: 1,
        from_behind: false,
    }
}

const fn threat(mut a: Ability, threat: f32) -> Ability {
    a.threat = threat;
    a
}

const fn projectile(mut a: Ability) -> Ability {
    a.projectile = true;
    a
}

const fn stacks(mut a: Ability, max: u8) -> Ability {
    a.max_stacks = max;
    a
}

const fn behind(mut a: Ability) -> Ability {
    a.from_behind = true;
    a
}

use AuraKind as A;
use Effect as E;
use School as S;
use Targeting as T;

/// Every ability, indexed by `AbilityId`. Damage and healing are at level 1
/// and grow with `level_scale`.
pub static ABILITIES: [Ability; 27] = [
    // Warrior
    threat(
        ab(
            "Heroic Strike",
            "A strong attack that causes a high amount of threat.",
            S::Physical,
            T::Enemy,
            MELEE_RANGE,
            0.0,
            0.0,
            15.0,
            &[E::Damage {
                min: 14.0,
                max: 18.0,
            }],
        ),
        1.8,
    ),
    stacks(
        ab(
            "Rend",
            "Wounds the target, causing it to bleed over 15 sec. Stacks up to 3 times.",
            S::Physical,
            T::Enemy,
            MELEE_RANGE,
            0.0,
            0.0,
            10.0,
            &[E::Aura {
                kind: A::Dot {
                    per_tick: 5.0,
                    interval: 3.0,
                },
                duration: 15.0,
            }],
        ),
        3,
    ),
    ab(
        "Shield Bash",
        "Bashes the target, interrupting spellcasting and stunning it for 2 sec.",
        S::Physical,
        T::Enemy,
        MELEE_RANGE,
        0.0,
        12.0,
        10.0,
        &[
            E::Damage { min: 6.0, max: 8.0 },
            E::Interrupt,
            E::Aura {
                kind: A::Stun,
                duration: 2.0,
            },
        ],
    ),
    threat(
        ab(
            "Thunder Clap",
            "Damages and slows every enemy within 8 yards.",
            S::Physical,
            T::AroundCaster(8.0),
            0.0,
            0.0,
            6.0,
            20.0,
            &[
                E::Damage {
                    min: 9.0,
                    max: 11.0,
                },
                E::Aura {
                    kind: A::Slow(0.6),
                    duration: 8.0,
                },
            ],
        ),
        2.0,
    ),
    ab(
        "Taunt",
        "Forces the target to attack you.",
        S::Physical,
        T::Enemy,
        25.0,
        0.0,
        8.0,
        0.0,
        &[E::Taunt],
    ),
    ab(
        "Second Wind",
        "Heals you over 10 sec.",
        S::Nature,
        T::Caster,
        0.0,
        0.0,
        60.0,
        0.0,
        &[E::Aura {
            kind: A::Hot {
                per_tick: 14.0,
                interval: 2.0,
            },
            duration: 10.0,
        }],
    ),
    // Mage
    projectile(ab(
        "Fireball",
        "Hurls a fiery ball that sets the target ablaze for 6 sec.",
        S::Fire,
        T::Enemy,
        30.0,
        2.5,
        0.0,
        14.0,
        &[
            E::Damage {
                min: 22.0,
                max: 27.0,
            },
            E::Aura {
                kind: A::Dot {
                    per_tick: 3.0,
                    interval: 2.0,
                },
                duration: 6.0,
            },
        ],
    )),
    projectile(ab(
        "Frostbolt",
        "Damages the target and slows it for 6 sec.",
        S::Frost,
        T::Enemy,
        30.0,
        2.0,
        0.0,
        12.0,
        &[
            E::Damage {
                min: 15.0,
                max: 19.0,
            },
            E::Aura {
                kind: A::Slow(0.5),
                duration: 6.0,
            },
        ],
    )),
    ab(
        "Fire Blast",
        "Instantly blasts the target with fire.",
        S::Fire,
        T::Enemy,
        20.0,
        0.0,
        8.0,
        12.0,
        &[E::Damage {
            min: 15.0,
            max: 19.0,
        }],
    ),
    ab(
        "Frost Nova",
        "Damages every enemy within 10 yards and freezes them in place for 6 sec.",
        S::Frost,
        T::AroundCaster(10.0),
        0.0,
        0.0,
        20.0,
        12.0,
        &[
            E::Damage { min: 4.0, max: 6.0 },
            E::Aura {
                kind: A::Root,
                duration: 6.0,
            },
        ],
    ),
    ab(
        "Ice Barrier",
        "Shields you, absorbing damage for 30 sec.",
        S::Frost,
        T::Caster,
        0.0,
        0.0,
        30.0,
        15.0,
        &[E::Aura {
            kind: A::Absorb(45.0),
            duration: 30.0,
        }],
    ),
    ab(
        "Evocation",
        "Restores 40% of your mana.",
        S::Arcane,
        T::Caster,
        0.0,
        0.0,
        90.0,
        0.0,
        &[E::RestorePower(0.4)],
    ),
    // Cleric
    projectile(ab(
        "Smite",
        "Smites the target with holy light.",
        S::Holy,
        T::Enemy,
        30.0,
        2.0,
        0.0,
        10.0,
        &[E::Damage {
            min: 17.0,
            max: 21.0,
        }],
    )),
    ab(
        "Shadow Word: Pain",
        "Deals shadow damage over 18 sec.",
        S::Shadow,
        T::Enemy,
        30.0,
        0.0,
        0.0,
        10.0,
        &[E::Aura {
            kind: A::Dot {
                per_tick: 6.0,
                interval: 3.0,
            },
            duration: 18.0,
        }],
    ),
    ab(
        "Heal",
        "Heals a friendly target.",
        S::Holy,
        T::Friendly,
        40.0,
        2.5,
        0.0,
        18.0,
        &[E::Heal {
            min: 38.0,
            max: 46.0,
        }],
    ),
    ab(
        "Renew",
        "Heals a friendly target over 15 sec.",
        S::Holy,
        T::Friendly,
        40.0,
        0.0,
        0.0,
        12.0,
        &[E::Aura {
            kind: A::Hot {
                per_tick: 9.0,
                interval: 3.0,
            },
            duration: 15.0,
        }],
    ),
    ab(
        "Power Word: Shield",
        "Shields a friendly target, absorbing damage for 15 sec.",
        S::Holy,
        T::Friendly,
        40.0,
        0.0,
        8.0,
        15.0,
        &[E::Aura {
            kind: A::Absorb(38.0),
            duration: 15.0,
        }],
    ),
    ab(
        "Holy Nova",
        "Damages every enemy within 10 yards.",
        S::Holy,
        T::AroundCaster(10.0),
        0.0,
        0.0,
        10.0,
        16.0,
        &[E::Damage {
            min: 10.0,
            max: 12.0,
        }],
    ),
    // Mobs
    ab(
        "Savage Bite",
        "A bite that bleeds over 9 sec.",
        S::Physical,
        T::Enemy,
        MELEE_RANGE,
        0.0,
        0.0,
        0.0,
        &[
            E::Damage { min: 3.0, max: 4.0 },
            E::Aura {
                kind: A::Dot {
                    per_tick: 3.0,
                    interval: 3.0,
                },
                duration: 9.0,
            },
        ],
    ),
    projectile(ab(
        "Shadow Bolt",
        "Sends a bolt of shadow at the target.",
        S::Shadow,
        T::Enemy,
        30.0,
        2.0,
        0.0,
        0.0,
        &[E::Damage {
            min: 12.0,
            max: 15.0,
        }],
    )),
    ab(
        "Ground Slam",
        "Slams the ground, hurting everything within 9 yards.",
        S::Physical,
        T::AroundCaster(9.0),
        0.0,
        1.5,
        0.0,
        0.0,
        &[E::Damage {
            min: 22.0,
            max: 28.0,
        }],
    ),
    // Rogue
    ab(
        "Sinister Strike",
        "A quick strike that adds a combo point.",
        S::Physical,
        T::Enemy,
        MELEE_RANGE,
        0.0,
        0.0,
        40.0,
        &[
            E::Damage {
                min: 11.0,
                max: 14.0,
            },
            E::ComboPoint,
        ],
    ),
    ab(
        "Eviscerate",
        "A finishing move that spends every combo point. More points, more damage.",
        S::Physical,
        T::Enemy,
        MELEE_RANGE,
        0.0,
        0.0,
        35.0,
        &[E::Finisher {
            min: 6.0,
            max: 9.0,
            per_point: 10.0,
        }],
    ),
    behind(ab(
        "Backstab",
        "Stabs the target from behind for heavy damage. Adds a combo point.",
        S::Physical,
        T::Enemy,
        MELEE_RANGE,
        0.0,
        0.0,
        60.0,
        &[
            E::Damage {
                min: 24.0,
                max: 29.0,
            },
            E::ComboPoint,
        ],
    )),
    ab(
        "Gouge",
        "Gouges the target, interrupting it and stunning it for 4 sec. Adds a combo point.",
        S::Physical,
        T::Enemy,
        MELEE_RANGE,
        0.0,
        15.0,
        45.0,
        &[
            E::Damage { min: 3.0, max: 4.0 },
            E::Interrupt,
            E::Aura {
                kind: A::Stun,
                duration: 4.0,
            },
            E::ComboPoint,
        ],
    ),
    ab(
        "Evasion",
        "Halves the damage you take for 10 sec.",
        S::Physical,
        T::Caster,
        0.0,
        0.0,
        60.0,
        0.0,
        &[E::Aura {
            kind: A::DamageTaken(0.5),
            duration: 10.0,
        }],
    ),
    ab(
        "Kick",
        "Kicks the target, interrupting spellcasting.",
        S::Physical,
        T::Enemy,
        MELEE_RANGE,
        0.0,
        10.0,
        25.0,
        &[E::Damage { min: 4.0, max: 5.0 }, E::Interrupt],
    ),
];

// ---- Items ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ItemId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Slot {
    Head,
    Chest,
    Hands,
    Legs,
    Feet,
}

impl Slot {
    pub const ALL: [Slot; 5] = [Slot::Head, Slot::Chest, Slot::Hands, Slot::Legs, Slot::Feet];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Slot::Head => "Head",
            Slot::Chest => "Chest",
            Slot::Hands => "Hands",
            Slot::Legs => "Legs",
            Slot::Feet => "Feet",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Common,
    Uncommon,
    Rare,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemKind {
    /// Used for crafting.
    Material,
    Armor {
        slot: Slot,
        armor: f32,
        stamina: f32,
        /// Each point adds 1% to damage and healing done.
        power: f32,
    },
}

#[derive(Debug)]
pub struct Item {
    pub name: &'static str,
    pub description: &'static str,
    pub kind: ItemKind,
    pub quality: Quality,
    pub max_stack: u16,
    /// How it looks when worn (and its icon color).
    pub color: (f32, f32, f32),
}

pub fn item(id: ItemId) -> &'static Item {
    &ITEMS[id.0 as usize]
}

pub mod items {
    use super::ItemId;

    pub const LIGHT_LEATHER: ItemId = ItemId(0);
    pub const LINEN_CLOTH: ItemId = ItemId(1);
    pub const GOLEM_CORE: ItemId = ItemId(2);
    pub const LEATHER_CAP: ItemId = ItemId(3);
    pub const LEATHER_VEST: ItemId = ItemId(4);
    pub const LEATHER_GLOVES: ItemId = ItemId(5);
    pub const LEATHER_PANTS: ItemId = ItemId(6);
    pub const LEATHER_BOOTS: ItemId = ItemId(7);
    pub const LINEN_HOOD: ItemId = ItemId(8);
    pub const LINEN_ROBE: ItemId = ItemId(9);
    pub const LINEN_PANTS: ItemId = ItemId(10);
    pub const GOLEMHEART_CHESTGUARD: ItemId = ItemId(11);
}

const fn material(
    name: &'static str,
    description: &'static str,
    quality: Quality,
    color: (f32, f32, f32),
) -> Item {
    Item {
        name,
        description,
        kind: ItemKind::Material,
        quality,
        max_stack: 20,
        color,
    }
}

const fn armor(
    name: &'static str,
    slot: Slot,
    armor: f32,
    stamina: f32,
    power: f32,
    quality: Quality,
    color: (f32, f32, f32),
) -> Item {
    Item {
        name,
        description: "",
        kind: ItemKind::Armor {
            slot,
            armor,
            stamina,
            power,
        },
        quality,
        max_stack: 1,
        color,
    }
}

const LEATHER: (f32, f32, f32) = (0.5, 0.33, 0.18);
const LINEN: (f32, f32, f32) = (0.85, 0.8, 0.68);

pub static ITEMS: [Item; 12] = [
    material(
        "Light Leather",
        "Tanned hide from the beasts of the vale. Used to make leather armor.",
        Quality::Common,
        (0.6, 0.42, 0.25),
    ),
    material(
        "Linen Cloth",
        "A bolt of plain cloth. Used to make linen armor.",
        Quality::Common,
        (0.9, 0.86, 0.75),
    ),
    material(
        "Golem Core",
        "The still-warm heart of an Ancient Golem.",
        Quality::Rare,
        (0.35, 0.85, 1.0),
    ),
    armor(
        "Leather Cap",
        Slot::Head,
        6.0,
        2.0,
        0.0,
        Quality::Common,
        LEATHER,
    ),
    armor(
        "Leather Vest",
        Slot::Chest,
        12.0,
        4.0,
        0.0,
        Quality::Common,
        LEATHER,
    ),
    armor(
        "Leather Gloves",
        Slot::Hands,
        5.0,
        2.0,
        0.0,
        Quality::Common,
        LEATHER,
    ),
    armor(
        "Leather Pants",
        Slot::Legs,
        10.0,
        3.0,
        0.0,
        Quality::Common,
        LEATHER,
    ),
    armor(
        "Leather Boots",
        Slot::Feet,
        6.0,
        2.0,
        0.0,
        Quality::Common,
        LEATHER,
    ),
    armor(
        "Linen Hood",
        Slot::Head,
        2.0,
        1.0,
        2.0,
        Quality::Common,
        LINEN,
    ),
    armor(
        "Linen Robe",
        Slot::Chest,
        4.0,
        2.0,
        4.0,
        Quality::Common,
        LINEN,
    ),
    armor(
        "Linen Pants",
        Slot::Legs,
        3.0,
        1.0,
        3.0,
        Quality::Common,
        LINEN,
    ),
    armor(
        "Golemheart Chestguard",
        Slot::Chest,
        30.0,
        10.0,
        5.0,
        Quality::Rare,
        (0.42, 0.45, 0.5),
    ),
];

/// Something you can make from materials.
#[derive(Debug)]
pub struct Recipe {
    pub result: ItemId,
    pub materials: &'static [(ItemId, u16)],
}

pub static RECIPES: [Recipe; 9] = {
    use items::*;
    [
        Recipe {
            result: LEATHER_CAP,
            materials: &[(LIGHT_LEATHER, 4)],
        },
        Recipe {
            result: LEATHER_VEST,
            materials: &[(LIGHT_LEATHER, 6)],
        },
        Recipe {
            result: LEATHER_GLOVES,
            materials: &[(LIGHT_LEATHER, 3)],
        },
        Recipe {
            result: LEATHER_PANTS,
            materials: &[(LIGHT_LEATHER, 5)],
        },
        Recipe {
            result: LEATHER_BOOTS,
            materials: &[(LIGHT_LEATHER, 4)],
        },
        Recipe {
            result: LINEN_HOOD,
            materials: &[(LINEN_CLOTH, 3)],
        },
        Recipe {
            result: LINEN_ROBE,
            materials: &[(LINEN_CLOTH, 6)],
        },
        Recipe {
            result: LINEN_PANTS,
            materials: &[(LINEN_CLOTH, 4)],
        },
        Recipe {
            result: GOLEMHEART_CHESTGUARD,
            materials: &[(GOLEM_CORE, 1), (LIGHT_LEATHER, 8)],
        },
    ]
};

// ---- Mobs ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobKind {
    Wolf,
    Boar,
    Bandit,
    BanditMystic,
    Golem,
}

/// What a mob can drop: money and items.
#[derive(Debug)]
pub struct LootTable {
    /// Copper per mob level.
    pub copper_per_level: (u32, u32),
    /// Item, chance, and how many.
    pub items: &'static [(ItemId, f32, u16, u16)],
}

#[derive(Debug)]
pub struct MobTemplate {
    pub name: &'static str,
    /// Health at level 1.
    pub hp: f32,
    pub damage: (f32, f32),
    pub attack_interval: f32,
    pub speed: f32,
    /// Attacks players that come close. Otherwise only fights back.
    pub aggressive: bool,
    pub elite: bool,
    /// Calls nearby mobs from its camp for help when attacked.
    pub social: bool,
    /// Rough radius, used for melee reach and drawing.
    pub size: f32,
    /// An ability the mob uses every so many seconds.
    pub spell: Option<(AbilityId, f32)>,
    pub respawn: f32,
    pub loot: LootTable,
}

impl MobKind {
    pub fn template(self) -> &'static MobTemplate {
        use items::*;
        match self {
            MobKind::Wolf => &MobTemplate {
                name: "Gray Wolf",
                hp: 55.0,
                damage: (4.0, 6.0),
                attack_interval: 2.0,
                speed: 7.5,
                aggressive: true,
                elite: false,
                social: false,
                size: 0.9,
                spell: Some((ids::SAVAGE_BITE, 10.0)),
                respawn: 25.0,
                loot: LootTable {
                    copper_per_level: (1, 4),
                    items: &[(LIGHT_LEATHER, 0.4, 1, 1)],
                },
            },
            MobKind::Boar => &MobTemplate {
                name: "Wild Boar",
                hp: 65.0,
                damage: (4.0, 7.0),
                attack_interval: 2.2,
                speed: 6.5,
                aggressive: false,
                elite: false,
                social: false,
                size: 0.9,
                spell: None,
                respawn: 25.0,
                loot: LootTable {
                    copper_per_level: (1, 3),
                    items: &[(LIGHT_LEATHER, 0.85, 1, 2)],
                },
            },
            MobKind::Bandit => &MobTemplate {
                name: "Bandit Thug",
                hp: 70.0,
                damage: (6.0, 9.0),
                attack_interval: 2.2,
                speed: 6.5,
                aggressive: true,
                elite: false,
                social: true,
                size: 0.8,
                spell: None,
                respawn: 30.0,
                loot: LootTable {
                    copper_per_level: (6, 15),
                    items: &[(LINEN_CLOTH, 0.6, 1, 2)],
                },
            },
            MobKind::BanditMystic => &MobTemplate {
                name: "Bandit Mystic",
                hp: 55.0,
                damage: (3.0, 5.0),
                attack_interval: 2.0,
                speed: 6.5,
                aggressive: true,
                elite: false,
                social: true,
                size: 0.8,
                spell: Some((ids::SHADOW_BOLT, 5.0)),
                respawn: 30.0,
                loot: LootTable {
                    copper_per_level: (6, 15),
                    items: &[(LINEN_CLOTH, 0.75, 1, 3)],
                },
            },
            MobKind::Golem => &MobTemplate {
                name: "Ancient Golem",
                hp: 330.0,
                damage: (12.0, 17.0),
                attack_interval: 3.0,
                speed: 5.5,
                aggressive: true,
                elite: true,
                social: false,
                size: 2.2,
                spell: Some((ids::GROUND_SLAM, 12.0)),
                respawn: 120.0,
                loot: LootTable {
                    copper_per_level: (20, 40),
                    items: &[(GOLEM_CORE, 1.0, 1, 1), (LINEN_CLOTH, 1.0, 2, 4)],
                },
            },
        }
    }

    pub fn max_hp(self, level: u8) -> f32 {
        self.template().hp * (1.0 + 0.3 * (level.max(1) - 1) as f32)
    }
}

// ---- Appearance ----

/// How a character looks, chosen at character creation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appearance {
    /// 0: broad build, 1: slender build.
    pub body: u8,
    pub skin: u8,
    pub hair_style: u8,
    pub hair_color: u8,
}

impl Appearance {
    pub const BODIES: u8 = 2;
    pub const SKINS: u8 = 5;
    pub const HAIR_STYLES: u8 = 5;
    pub const HAIR_COLORS: u8 = 6;

    pub fn hair_style_name(self) -> &'static str {
        ["Bald", "Short", "Long", "Ponytail", "Mohawk"][self.hair_style as usize % 5]
    }

    /// Keeps every field in range (for data from the network or a save file).
    pub fn clamped(self) -> Self {
        Self {
            body: self.body % Self::BODIES,
            skin: self.skin % Self::SKINS,
            hair_style: self.hair_style % Self::HAIR_STYLES,
            hair_color: self.hair_color % Self::HAIR_COLORS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ability_ids_match_table() {
        for class in Class::ALL {
            for id in class.abilities() {
                assert!((id.0 as usize) < ABILITIES.len());
            }
        }
        assert_eq!(ability(ids::GROUND_SLAM).name, "Ground Slam");
        assert_eq!(ability(ids::SMITE).name, "Smite");
        assert_eq!(ability(ids::FIREBALL).name, "Fireball");
        assert_eq!(ability(ids::KICK).name, "Kick");
        assert_eq!(
            item(items::GOLEMHEART_CHESTGUARD).name,
            "Golemheart Chestguard"
        );
    }

    #[test]
    fn classes_start_with_one_ability() {
        for class in Class::ALL {
            assert_eq!(class.known(1).count(), 1, "{class:?}");
            assert_eq!(class.known(MAX_LEVEL).count(), ACTION_BAR_SLOTS);
            assert_eq!(class.unlock_level(class.abilities()[0]), Some(1));
        }
        assert_eq!(Class::Mage.unlock_level(ids::SMITE), None);
    }

    #[test]
    fn xp_curve() {
        assert_eq!(xp_to_next(1), 100);
        assert_eq!(xp_to_next(MAX_LEVEL), 0);
        // Same-level kills take a handful of mobs per level.
        let kills = xp_to_next(1) as f32 / kill_xp(1, 1, false) as f32;
        assert!((2.0..5.0).contains(&kills));
        assert_eq!(kill_xp(9, 3, false), 0);
        assert!(kill_xp(5, 5, true) > kill_xp(5, 5, false));
    }

    #[test]
    fn money_formatting() {
        assert_eq!(format_money(0), "0c");
        assert_eq!(format_money(57), "57c");
        assert_eq!(format_money(10_305), "1g 3s 5c");
    }

    #[test]
    fn recipes_make_armor_from_drops() {
        for r in &RECIPES {
            assert!(matches!(item(r.result).kind, ItemKind::Armor { .. }));
            for (m, n) in r.materials {
                assert_eq!(item(*m).kind, ItemKind::Material);
                assert!(*n <= item(*m).max_stack);
            }
        }
    }
}
