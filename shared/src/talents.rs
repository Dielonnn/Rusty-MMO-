//! Talent trees: every class has three branches of three talents. You earn a
//! talent point each level from 2 on, and deeper talents in a branch open up
//! as you spend points in it.

use crate::data::ids::*;
use crate::data::{AbilityId, Class, ability};

/// Talents per class: three branches of three tiers. A character's ranks are
/// stored in this order: branch 0 tiers 0-2, branch 1 tiers 0-2, branch 2.
pub const TALENTS: usize = 9;
/// Ranks in each tier: the deeper the talent, the fewer ranks (and the
/// bigger each rank).
pub const TIER_RANKS: [u8; 3] = [3, 2, 1];
/// Points that must already be spent in a branch to open each tier.
pub const TIER_REQUIRES: [u8; 3] = [0, 2, 4];

pub type Ranks = [u8; TALENTS];

/// What a talent does, per rank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TalentEffect {
    /// More damage or healing from one ability (a fraction).
    Ability(AbilityId, f32),
    /// Seconds off an ability's cooldown.
    Cooldown(AbilityId, f32),
    /// Seconds off an ability's cast time.
    CastTime(AbilityId, f32),
    /// More damage from everything.
    Damage(f32),
    /// More healing from everything.
    Healing(f32),
    /// Added critical strike chance.
    Crit(f32),
    /// More maximum health.
    Health(f32),
    /// Less damage taken.
    Toughness(f32),
    /// Faster mana, energy or rage regeneration.
    Regen(f32),
    /// Faster running.
    Speed(f32),
    /// Heals you for a share of the damage you deal.
    Leech(f32),
}

#[derive(Clone, Copy, Debug)]
pub struct Talent {
    pub name: &'static str,
    pub effect: TalentEffect,
}

pub struct Tree {
    pub branches: [&'static str; 3],
    /// Indexed `branch * 3 + tier`.
    pub talents: [Talent; TALENTS],
}

use TalentEffect::*;

pub fn tree(class: Class) -> &'static Tree {
    match class {
        Class::Barbarian => &Tree {
            branches: ["Fury", "Brawn", "Wrath"],
            talents: [
                Talent {
                    name: "Cruelty",
                    effect: Crit(0.02),
                },
                Talent {
                    name: "Savagery",
                    effect: Ability(HEROIC_STRIKE, 0.1),
                },
                Talent {
                    name: "Rampage",
                    effect: Cooldown(WHIRLWIND, 2.0),
                },
                Talent {
                    name: "Thick Hide",
                    effect: Health(0.03),
                },
                Talent {
                    name: "Iron Will",
                    effect: Toughness(0.03),
                },
                Talent {
                    name: "Unstoppable",
                    effect: Cooldown(CHARGE, 5.0),
                },
                Talent {
                    name: "Blood Frenzy",
                    effect: Damage(0.02),
                },
                Talent {
                    name: "Deep Wounds",
                    effect: Ability(REND, 0.15),
                },
                Talent {
                    name: "Berserker",
                    effect: Cooldown(RECKLESSNESS, 20.0),
                },
            ],
        },
        Class::Fighter => &Tree {
            branches: ["Arms", "Defense", "Tactics"],
            talents: [
                Talent {
                    name: "Weapon Mastery",
                    effect: Damage(0.02),
                },
                Talent {
                    name: "Precise Strikes",
                    effect: Ability(POWER_STRIKE, 0.12),
                },
                Talent {
                    name: "Crushing Blow",
                    effect: Crit(0.05),
                },
                Talent {
                    name: "Toughness",
                    effect: Toughness(0.03),
                },
                Talent {
                    name: "Shield Mastery",
                    effect: Cooldown(SHIELD_BLOCK, 2.0),
                },
                Talent {
                    name: "Last Stand",
                    effect: Health(0.1),
                },
                Talent {
                    name: "Thunderous",
                    effect: Ability(THUNDER_CLAP, 0.15),
                },
                Talent {
                    name: "Quick Bash",
                    effect: Cooldown(SHIELD_BASH, 2.0),
                },
                Talent {
                    name: "Second Breath",
                    effect: Ability(SECOND_WIND, 0.5),
                },
            ],
        },
        Class::Paladin => &Tree {
            branches: ["Holy", "Protection", "Retribution"],
            talents: [
                Talent {
                    name: "Divine Grace",
                    effect: Healing(0.03),
                },
                Talent {
                    name: "Swift Light",
                    effect: CastTime(HOLY_LIGHT, 0.3),
                },
                Talent {
                    name: "Martyr",
                    effect: Cooldown(LAY_ON_HANDS, 30.0),
                },
                Talent {
                    name: "Devotion",
                    effect: Toughness(0.03),
                },
                Talent {
                    name: "Blessed Plate",
                    effect: Health(0.04),
                },
                Talent {
                    name: "Divine Shield",
                    effect: Cooldown(DIVINE_PROTECTION, 15.0),
                },
                Talent {
                    name: "Zeal",
                    effect: Ability(CRUSADER_STRIKE, 0.1),
                },
                Talent {
                    name: "Swift Judgment",
                    effect: Cooldown(JUDGMENT, 1.5),
                },
                Talent {
                    name: "Wrath of Heaven",
                    effect: Ability(CONSECRATION, 0.4),
                },
            ],
        },
        Class::Monk => &Tree {
            branches: ["Windwalker", "Brewmaster", "Mistweaver"],
            talents: [
                Talent {
                    name: "Fists of Fury",
                    effect: Ability(TIGER_PALM, 0.1),
                },
                Talent {
                    name: "Rising Wind",
                    effect: Cooldown(RISING_SUN_KICK, 1.5),
                },
                Talent {
                    name: "Whirling Dragon",
                    effect: Ability(SPINNING_CRANE_KICK, 0.4),
                },
                Talent {
                    name: "Stagger",
                    effect: Toughness(0.03),
                },
                Talent {
                    name: "Iron Body",
                    effect: Health(0.04),
                },
                Talent {
                    name: "Celestial Brew",
                    effect: Cooldown(FORTIFYING_BREW, 20.0),
                },
                Talent {
                    name: "Inner Peace",
                    effect: Regen(0.1),
                },
                Talent {
                    name: "Swift as Wind",
                    effect: Speed(0.04),
                },
                Talent {
                    name: "Chi Torpedo",
                    effect: Cooldown(ROLL, 4.0),
                },
            ],
        },
        Class::Rogue => &Tree {
            branches: ["Assassination", "Combat", "Subtlety"],
            talents: [
                Talent {
                    name: "Lethality",
                    effect: Crit(0.02),
                },
                Talent {
                    name: "Vile Poisons",
                    effect: Ability(EVISCERATE, 0.12),
                },
                Talent {
                    name: "Cold Blood",
                    effect: Damage(0.06),
                },
                Talent {
                    name: "Precision",
                    effect: Ability(SINISTER_STRIKE, 0.08),
                },
                Talent {
                    name: "Relentless",
                    effect: Regen(0.15),
                },
                Talent {
                    name: "Adrenaline Rush",
                    effect: Cooldown(SPRINT, 10.0),
                },
                Talent {
                    name: "Opportunity",
                    effect: Ability(BACKSTAB, 0.12),
                },
                Talent {
                    name: "Elusiveness",
                    effect: Cooldown(EVASION, 15.0),
                },
                Talent {
                    name: "Shadowstep",
                    effect: Speed(0.1),
                },
            ],
        },
        Class::Ranger => &Tree {
            branches: ["Marksmanship", "Survival", "Beast Mastery"],
            talents: [
                Talent {
                    name: "Lethal Shots",
                    effect: Crit(0.02),
                },
                Talent {
                    name: "Careful Aim",
                    effect: CastTime(STEADY_SHOT, 0.25),
                },
                Talent {
                    name: "Trick Shots",
                    effect: Ability(MULTI_SHOT, 0.4),
                },
                Talent {
                    name: "Hawk's Endurance",
                    effect: Health(0.03),
                },
                Talent {
                    name: "Natural Armor",
                    effect: Toughness(0.04),
                },
                Talent {
                    name: "Quick Escape",
                    effect: Cooldown(DISENGAGE, 4.0),
                },
                Talent {
                    name: "Venomous",
                    effect: Ability(SERPENT_STING, 0.15),
                },
                Talent {
                    name: "Killer Instinct",
                    effect: Ability(KILL_SHOT, 0.15),
                },
                Talent {
                    name: "Pathfinding",
                    effect: Speed(0.1),
                },
            ],
        },
        Class::Artificer => &Tree {
            branches: ["Artillerist", "Armorer", "Alchemist"],
            talents: [
                Talent {
                    name: "Overcharge",
                    effect: Ability(ARCANE_RIFLE, 0.1),
                },
                Talent {
                    name: "Bigger Booms",
                    effect: Ability(THUNDER_GRENADE, 0.15),
                },
                Talent {
                    name: "Arcane Firearm",
                    effect: Crit(0.05),
                },
                Talent {
                    name: "Plated",
                    effect: Health(0.03),
                },
                Talent {
                    name: "Reinforced Armor",
                    effect: Ability(ARCANE_ARMOR, 0.25),
                },
                Talent {
                    name: "Thrusters",
                    effect: Cooldown(ROCKET_BOOTS, 5.0),
                },
                Talent {
                    name: "Potent Mixtures",
                    effect: Ability(INFUSED_TONIC, 0.15),
                },
                Talent {
                    name: "Corrosive",
                    effect: Ability(ACID_FLASK, 0.15),
                },
                Talent {
                    name: "Efficient Brewing",
                    effect: Regen(0.25),
                },
            ],
        },
        Class::Bard => &Tree {
            branches: ["Lore", "Valor", "Glamour"],
            talents: [
                Talent {
                    name: "Cutting Words",
                    effect: Ability(VICIOUS_MOCKERY, 0.1),
                },
                Talent {
                    name: "Sharp Tongue",
                    effect: Damage(0.03),
                },
                Talent {
                    name: "Peerless Skill",
                    effect: Crit(0.05),
                },
                Talent {
                    name: "Battle Hymn",
                    effect: Cooldown(INSPIRE, 10.0),
                },
                Talent {
                    name: "Steady Rhythm",
                    effect: Toughness(0.04),
                },
                Talent {
                    name: "Resounding",
                    effect: Ability(THUNDERWAVE, 0.4),
                },
                Talent {
                    name: "Soothing Voice",
                    effect: Healing(0.03),
                },
                Talent {
                    name: "Swift Words",
                    effect: CastTime(HEALING_WORD, 0.25),
                },
                Talent {
                    name: "Encore",
                    effect: Cooldown(SONG_OF_REST, 15.0),
                },
            ],
        },
        Class::Cleric => &Tree {
            branches: ["Holy", "Discipline", "Shadow"],
            talents: [
                Talent {
                    name: "Spiritual Healing",
                    effect: Healing(0.03),
                },
                Talent {
                    name: "Divine Fury",
                    effect: CastTime(HEAL, 0.25),
                },
                Talent {
                    name: "Holy Word",
                    effect: Ability(HOLY_NOVA, 0.4),
                },
                Talent {
                    name: "Improved Shield",
                    effect: Ability(POWER_WORD_SHIELD, 0.1),
                },
                Talent {
                    name: "Inner Focus",
                    effect: Regen(0.15),
                },
                Talent {
                    name: "Pain Suppression",
                    effect: Toughness(0.08),
                },
                Talent {
                    name: "Darkness",
                    effect: Damage(0.02),
                },
                Talent {
                    name: "Improved Pain",
                    effect: Ability(SHADOW_WORD_PAIN, 0.15),
                },
                Talent {
                    name: "Shadow Power",
                    effect: Crit(0.05),
                },
            ],
        },
        Class::Druid => &Tree {
            branches: ["Balance", "Restoration", "Feral"],
            talents: [
                Talent {
                    name: "Moonglow",
                    effect: Ability(MOONFIRE, 0.1),
                },
                Talent {
                    name: "Starlight Wrath",
                    effect: CastTime(WRATH, 0.25),
                },
                Talent {
                    name: "Celestial Focus",
                    effect: Ability(STARFALL, 0.4),
                },
                Talent {
                    name: "Gift of Nature",
                    effect: Healing(0.03),
                },
                Talent {
                    name: "Improved Rejuvenation",
                    effect: Ability(REJUVENATION, 0.15),
                },
                Talent {
                    name: "Natural Swiftness",
                    effect: CastTime(HEALING_TOUCH, 0.5),
                },
                Talent {
                    name: "Thick Hide",
                    effect: Health(0.03),
                },
                Talent {
                    name: "Feline Swiftness",
                    effect: Speed(0.05),
                },
                Talent {
                    name: "Stampede",
                    effect: Cooldown(DASH, 10.0),
                },
            ],
        },
        Class::Mage => &Tree {
            branches: ["Fire", "Frost", "Arcane"],
            talents: [
                Talent {
                    name: "Ignite",
                    effect: Ability(FIREBALL, 0.1),
                },
                Talent {
                    name: "Impact",
                    effect: Cooldown(FIRE_BLAST, 1.5),
                },
                Talent {
                    name: "Critical Mass",
                    effect: Crit(0.06),
                },
                Talent {
                    name: "Ice Shards",
                    effect: Ability(FROSTBOLT, 0.1),
                },
                Talent {
                    name: "Improved Frostbolt",
                    effect: CastTime(FROSTBOLT, 0.25),
                },
                Talent {
                    name: "Shatter",
                    effect: Cooldown(FROST_NOVA, 6.0),
                },
                Talent {
                    name: "Arcane Mind",
                    effect: Regen(0.15),
                },
                Talent {
                    name: "Arcane Fortitude",
                    effect: Health(0.04),
                },
                Talent {
                    name: "Blink Mastery",
                    effect: Cooldown(BLINK, 5.0),
                },
            ],
        },
        Class::Sorcerer => &Tree {
            branches: ["Chaos", "Draconic", "Storm"],
            talents: [
                Talent {
                    name: "Wild Magic",
                    effect: Crit(0.02),
                },
                Talent {
                    name: "Tides of Chaos",
                    effect: Ability(CHAOS_BOLT, 0.15),
                },
                Talent {
                    name: "Surge",
                    effect: Cooldown(WILD_SURGE, 10.0),
                },
                Talent {
                    name: "Draconic Resilience",
                    effect: Health(0.04),
                },
                Talent {
                    name: "Scales",
                    effect: Toughness(0.03),
                },
                Talent {
                    name: "Elemental Affinity",
                    effect: Ability(METEOR, 0.4),
                },
                Talent {
                    name: "Tempest",
                    effect: Ability(ARCANE_BARRAGE, 0.1),
                },
                Talent {
                    name: "Quickened Spell",
                    effect: CastTime(CHAOS_BOLT, 0.25),
                },
                Talent {
                    name: "Windspeaker",
                    effect: Cooldown(MISTY_STEP, 4.0),
                },
            ],
        },
        Class::Warlock => &Tree {
            branches: ["Affliction", "Demonology", "Destruction"],
            talents: [
                Talent {
                    name: "Improved Corruption",
                    effect: Ability(CORRUPTION, 0.12),
                },
                Talent {
                    name: "Siphon Mastery",
                    effect: Leech(0.03),
                },
                Talent {
                    name: "Nightfall",
                    effect: Ability(DRAIN_LIFE, 0.4),
                },
                Talent {
                    name: "Demonic Embrace",
                    effect: Health(0.04),
                },
                Talent {
                    name: "Fel Armor",
                    effect: Toughness(0.03),
                },
                Talent {
                    name: "Soul Link",
                    effect: Leech(0.06),
                },
                Talent {
                    name: "Bane",
                    effect: Ability(ELDRITCH_BLAST, 0.1),
                },
                Talent {
                    name: "Devastation",
                    effect: Crit(0.03),
                },
                Talent {
                    name: "Ruin",
                    effect: Ability(SOUL_FIRE, 0.3),
                },
            ],
        },
    }
}

/// Talent points a character of this level has to spend.
pub fn points(level: u8) -> u8 {
    level.saturating_sub(1)
}

pub fn spent(ranks: &Ranks) -> u8 {
    ranks.iter().sum()
}

pub fn spent_in_branch(ranks: &Ranks, branch: usize) -> u8 {
    ranks[branch * 3..branch * 3 + 3].iter().sum()
}

/// Why a talent can't take another point, if it can't.
pub fn can_learn(ranks: &Ranks, level: u8, index: usize) -> Result<(), &'static str> {
    if index >= TALENTS {
        return Err("No such talent.");
    }
    let (branch, tier) = (index / 3, index % 3);
    if spent(ranks) >= points(level) {
        return Err("You have no talent points to spend.");
    }
    if ranks[index] >= TIER_RANKS[tier] {
        return Err("That talent is already at its highest rank.");
    }
    if spent_in_branch(ranks, branch) < TIER_REQUIRES[tier] {
        return Err("Spend more points in that branch first.");
    }
    Ok(())
}

/// Keeps only ranks that are legal for this level (for old or edited saves).
pub fn sanitize(ranks: &Ranks, level: u8) -> Ranks {
    let mut clean = [0; TALENTS];
    // Re-learn tier by tier so the requirements hold.
    for tier in 0..3 {
        for branch in 0..3 {
            let i = branch * 3 + tier;
            for _ in 0..ranks[i] {
                if can_learn(&clean, level, i).is_ok() {
                    clean[i] += 1;
                }
            }
        }
    }
    clean
}

impl Talent {
    /// What the talent does at a rank (shown for the next rank when
    /// learning).
    pub fn describe(&self, rank: u8) -> String {
        let r = rank.max(1) as f32;
        let pct = |f: f32| format!("{:.0}%", f * r * 100.0);
        match self.effect {
            Ability(id, f) => format!(
                "{} does {} more damage or healing.",
                ability(id).name,
                pct(f)
            ),
            Cooldown(id, s) => format!(
                "Takes {:.1} sec off {}'s cooldown.",
                s * r,
                ability(id).name
            ),
            CastTime(id, s) => format!(
                "Takes {:.2} sec off {}'s cast time.",
                s * r,
                ability(id).name
            ),
            Damage(f) => format!("You do {} more damage.", pct(f)),
            Healing(f) => format!("Your heals are {} stronger.", pct(f)),
            Crit(f) => format!("{} more chance to land a critical hit.", pct(f)),
            Health(f) => format!("{} more maximum health.", pct(f)),
            Toughness(f) => format!("You take {} less damage.", pct(f)),
            Regen(f) => format!("Your mana, energy or rage comes back {} faster.", pct(f)),
            Speed(f) => format!("You run {} faster.", pct(f)),
            Leech(f) => format!("Heals you for {} of the damage you deal.", pct(f)),
        }
    }
}

/// An ability's bonuses from talents.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AbilityBonus {
    /// Added fraction of damage or healing.
    pub power: f32,
    pub cooldown: f32,
    pub cast_time: f32,
}

/// Everything a character's talents add up to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bonuses {
    pub damage: f32,
    pub healing: f32,
    pub crit: f32,
    pub health: f32,
    pub toughness: f32,
    pub regen: f32,
    pub speed: f32,
    pub leech: f32,
    pub abilities: Vec<(AbilityId, AbilityBonus)>,
}

impl Bonuses {
    pub fn new(class: Class, ranks: &Ranks) -> Self {
        let mut b = Bonuses::default();
        for (talent, &rank) in tree(class).talents.iter().zip(ranks) {
            if rank == 0 {
                continue;
            }
            let r = rank as f32;
            let mut per_ability = |id: AbilityId, f: &dyn Fn(&mut AbilityBonus)| match b
                .abilities
                .iter_mut()
                .find(|(a, _)| *a == id)
            {
                Some((_, bonus)) => f(bonus),
                None => {
                    let mut bonus = AbilityBonus::default();
                    f(&mut bonus);
                    b.abilities.push((id, bonus));
                }
            };
            match talent.effect {
                Ability(id, f) => per_ability(id, &|a| a.power += f * r),
                Cooldown(id, s) => per_ability(id, &|a| a.cooldown += s * r),
                CastTime(id, s) => per_ability(id, &|a| a.cast_time += s * r),
                Damage(f) => b.damage += f * r,
                Healing(f) => b.healing += f * r,
                Crit(f) => b.crit += f * r,
                Health(f) => b.health += f * r,
                Toughness(f) => b.toughness += f * r,
                Regen(f) => b.regen += f * r,
                Speed(f) => b.speed += f * r,
                Leech(f) => b.leech += f * r,
            }
        }
        b
    }

    pub fn ability(&self, id: AbilityId) -> AbilityBonus {
        self.abilities
            .iter()
            .find(|(a, _)| *a == id)
            .map_or(AbilityBonus::default(), |(_, b)| *b)
    }

    /// An ability's cooldown after talents (never less than half).
    pub fn cooldown(&self, id: AbilityId) -> f32 {
        let base = ability(id).cooldown;
        (base - self.ability(id).cooldown).max(base * 0.5)
    }

    /// An ability's cast time after talents.
    pub fn cast_time(&self, id: AbilityId) -> f32 {
        let base = ability(id).cast_time;
        (base - self.ability(id).cast_time).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_has_a_full_tree_using_its_own_abilities() {
        for class in Class::ALL {
            let tree = tree(class);
            let kit = class.abilities();
            let mut names: Vec<_> = tree.talents.iter().map(|t| t.name).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), TALENTS, "{class:?} repeats a talent name");
            for talent in &tree.talents {
                if let Ability(id, _) | Cooldown(id, _) | CastTime(id, _) = talent.effect {
                    assert!(
                        kit.contains(&id),
                        "{class:?}: {} uses another class's ability",
                        talent.name
                    );
                }
                assert!(!talent.describe(1).is_empty());
            }
        }
    }

    #[test]
    fn tiers_open_as_points_are_spent() {
        let mut ranks = [0; TALENTS];
        assert!(can_learn(&ranks, 1, 0).is_err(), "no points at level 1");
        assert!(can_learn(&ranks, 10, 1).is_err(), "tier 2 is locked");
        ranks[0] = 2;
        assert!(can_learn(&ranks, 10, 1).is_ok());
        ranks[1] = 2;
        assert!(can_learn(&ranks, 10, 2).is_ok());
        assert!(
            can_learn(&ranks, 10, 4).is_err(),
            "other branches are separate"
        );
        ranks[0] = 3;
        assert!(can_learn(&ranks, 10, 0).is_err(), "rank cap");
        // Nine points at level 10, and no more.
        let full = [3, 2, 1, 3, 0, 0, 0, 0, 0];
        assert_eq!(spent(&full), 9);
        assert!(can_learn(&full, 10, 6).is_err());
        assert_eq!(sanitize(&full, 10), full);
        assert_eq!(sanitize(&full, 4), [3, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(sanitize(&[0, 2, 1, 0, 0, 0, 0, 0, 0], 10), [0; TALENTS]);
    }

    #[test]
    fn bonuses_add_up() {
        let ranks = [3, 2, 1, 1, 0, 0, 0, 0, 0];
        let b = Bonuses::new(Class::Mage, &ranks);
        assert!((b.ability(FIREBALL).power - 0.3).abs() < 1e-6);
        assert!((b.crit - 0.06).abs() < 1e-6);
        assert!(b.ability(FROSTBOLT).power > 0.0);
        let fire_blast = ability(FIRE_BLAST).cooldown;
        assert!(b.cooldown(FIRE_BLAST) < fire_blast);
        assert!(b.cooldown(FIRE_BLAST) >= fire_blast * 0.5);
    }
}
