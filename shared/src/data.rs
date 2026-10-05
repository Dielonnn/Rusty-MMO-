//! Game data: races, classes, abilities, mobs, items, recipes and the numbers
//! that balance them.

use serde::{Deserialize, Serialize};

use crate::world::Zone;

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
/// Abilities per class: keys 1-6 and E.
pub const ACTION_BAR_SLOTS: usize = 7;
/// The action bar slot bound to the E key.
pub const E_SLOT: usize = 6;
/// The level each action bar slot's ability is learned at (the last is E).
pub const UNLOCK_LEVELS: [u8; ACTION_BAR_SLOTS] = [1, 2, 4, 6, 8, 10, 3];
pub const BAG_SLOTS: usize = 20;
/// How close you have to be to loot a corpse.
pub const LOOT_RANGE: f32 = 6.0;
/// The most players in one party.
pub const MAX_PARTY_SIZE: usize = 8;
/// Party members this close to a kill share its experience, quest credit
/// and loot.
pub const PARTY_RANGE: f32 = 60.0;
/// Seconds before an unanswered party invite runs out.
pub const PARTY_INVITE_TIME: f32 = 60.0;
/// Seconds a party's corpse is kept for whoever's turn it is to loot.
pub const LOOT_TURN_TIME: f32 = 10.0;
/// How close you have to be to trade with a merchant.
pub const MERCHANT_RANGE: f32 = 8.0;
/// Extra health per point of stamina.
pub const HP_PER_STAMINA: f32 = 8.0;
/// Potions share one cooldown.
pub const POTION_COOLDOWN: f32 = 30.0;

// ---- Races ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Race {
    #[default]
    Human,
    Orc,
    Elf,
    Goblin,
    Gnome,
    Undead,
}

impl Race {
    pub const ALL: [Race; 6] = [
        Race::Human,
        Race::Orc,
        Race::Elf,
        Race::Goblin,
        Race::Gnome,
        Race::Undead,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Race::Human => "Human",
            Race::Orc => "Orc",
            Race::Elf => "Elf",
            Race::Goblin => "Goblin",
            Race::Gnome => "Gnome",
            Race::Undead => "Undead",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Race::Human => {
                "Hardy and adaptable, humans begin in the autumn hills of Amberfall Vale."
            }
            Race::Orc => {
                "Proud warriors of the desert, orcs begin in the sun-baked Scorchsand Wastes."
            }
            Race::Elf => {
                "Graceful and ancient, elves begin beneath the great trees of Silverbough Glade."
            }
            Race::Goblin => "Clever and greedy, goblins begin in the glowing Grubdeep Caverns.",
            Race::Gnome => "Tiny tinkerers, gnomes begin among the snowy Frostcog Peaks.",
            Race::Undead => "Risen from the grave, the undead begin in the dying Witherwood.",
        }
    }
}

// ---- Classes ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Class {
    /// Called Warrior before v3.0; old saves still load.
    #[serde(alias = "Warrior")]
    Barbarian,
    Mage,
    Cleric,
    Rogue,
    Ranger,
    Sorcerer,
    Paladin,
    Druid,
    Artificer,
    Warlock,
    Fighter,
    Monk,
    Bard,
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

const fn melee(interval: f32, min: f32, max: f32) -> AutoAttack {
    AutoAttack {
        range: MELEE_RANGE,
        interval,
        min,
        max,
    }
}

/// Wands, bows and guns.
const fn ranged(range: f32, interval: f32, min: f32, max: f32) -> AutoAttack {
    AutoAttack {
        range,
        interval,
        min,
        max,
    }
}

impl Class {
    pub const ALL: [Class; 13] = [
        Class::Barbarian,
        Class::Fighter,
        Class::Paladin,
        Class::Monk,
        Class::Rogue,
        Class::Ranger,
        Class::Artificer,
        Class::Bard,
        Class::Cleric,
        Class::Druid,
        Class::Mage,
        Class::Sorcerer,
        Class::Warlock,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Class::Barbarian => "Barbarian",
            Class::Mage => "Mage",
            Class::Cleric => "Cleric",
            Class::Rogue => "Rogue",
            Class::Ranger => "Ranger",
            Class::Sorcerer => "Sorcerer",
            Class::Paladin => "Paladin",
            Class::Druid => "Druid",
            Class::Artificer => "Artificer",
            Class::Warlock => "Warlock",
            Class::Fighter => "Fighter",
            Class::Monk => "Monk",
            Class::Bard => "Bard",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Class::Barbarian => {
                "A furious two-handed brawler. Builds rage by fighting, charges into battle (E) and cuts down crowds with Whirlwind."
            }
            Class::Fighter => {
                "A disciplined soldier with sword and shield. Holds enemies' attention, interrupts casters and shrugs off blows."
            }
            Class::Paladin => {
                "A holy knight in plate. Fights up close, heals with the Light and protects allies."
            }
            Class::Monk => {
                "A martial artist who runs on energy. Builds chi with Tiger Palm and Rising Sun Kick, then spends it on Blackout Kick."
            }
            Class::Rogue => {
                "A quick melee fighter who runs on energy. Builds combo points with Sinister Strike and Backstab, then spends them on a deadly Eviscerate."
            }
            Class::Ranger => {
                "A hunter with a longbow. Shoots from afar, slows and stings enemies, and leaps away from danger (E)."
            }
            Class::Artificer => {
                "An inventor with a rifle and gadgets. Throws grenades and nets, and rockets around on boots (E)."
            }
            Class::Bard => {
                "A performer whose words wound and heal. Mocks enemies, inspires allies and lulls foes to sleep."
            }
            Class::Cleric => {
                "A healer who can hold their own. Heals and shields allies, and wears enemies down with Smite and Shadow Word: Pain."
            }
            Class::Druid => {
                "A keeper of nature. Calls down wrath and moonfire, roots enemies, and heals over time."
            }
            Class::Mage => {
                "A fragile spellcaster with high damage. Burns enemies with fire, slows and roots them with frost, and blinks away (E)."
            }
            Class::Sorcerer => {
                "Born with wild magic. Hurls chaos bolts and meteors, holds enemies in place and surges with power."
            }
            Class::Warlock => {
                "A caster who made a dark pact. Curses and drains enemies, siphoning their life to heal itself."
            }
        }
    }

    pub fn power_kind(self) -> PowerKind {
        match self {
            Class::Barbarian | Class::Fighter => PowerKind::Rage,
            Class::Rogue | Class::Monk | Class::Ranger => PowerKind::Energy,
            _ => PowerKind::Mana,
        }
    }

    /// Health from the class and level alone, before gear.
    pub fn max_hp(self, level: u8) -> f32 {
        let l = (level.max(1) - 1) as f32;
        let (base, per) = match self {
            Class::Barbarian => (140.0, 35.0),
            Class::Fighter => (135.0, 33.0),
            Class::Paladin => (130.0, 32.0),
            Class::Monk => (120.0, 29.0),
            Class::Rogue => (115.0, 28.0),
            Class::Ranger => (110.0, 26.0),
            Class::Artificer => (105.0, 25.0),
            Class::Bard | Class::Cleric | Class::Druid => (100.0, 25.0),
            Class::Warlock => (95.0, 23.0),
            Class::Mage | Class::Sorcerer => (90.0, 22.0),
        };
        base + per * l
    }

    pub fn max_power(self, level: u8) -> f32 {
        let l = (level.max(1) - 1) as f32;
        match self.power_kind() {
            PowerKind::Rage | PowerKind::Energy => 100.0,
            PowerKind::Mana => match self {
                Class::Paladin => 110.0 + 16.0 * l,
                Class::Mage => 120.0 + 18.0 * l,
                Class::Artificer => 120.0 + 16.0 * l,
                _ => 128.0 + 18.0 * l,
            },
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
            Class::Barbarian => melee(2.4, 10.0, 14.0),
            Class::Fighter => melee(2.2, 8.0, 12.0),
            Class::Paladin => melee(2.4, 9.0, 13.0),
            Class::Monk | Class::Rogue => melee(1.6, 5.0, 8.0),
            Class::Ranger => ranged(30.0, 2.0, 6.0, 9.0),
            Class::Artificer => ranged(25.0, 2.0, 5.0, 8.0),
            // Casters shoot a wand.
            _ => ranged(25.0, 1.8, 3.0, 5.0),
        }
    }

    /// Whether the class fights up close (and starts swinging when it uses
    /// an ability).
    pub fn is_melee(self) -> bool {
        self.auto_attack().range <= MELEE_RANGE
    }

    /// The class's action bar: keys 1-6, then E. Slot `i` is learned at
    /// `UNLOCK_LEVELS[i]`.
    pub fn abilities(self) -> [AbilityId; ACTION_BAR_SLOTS] {
        use ids::*;
        match self {
            Class::Barbarian => [
                HEROIC_STRIKE,
                REND,
                WHIRLWIND,
                SKULL_BASH,
                TAUNT,
                RECKLESSNESS,
                CHARGE,
            ],
            Class::Fighter => [
                POWER_STRIKE,
                THUNDER_CLAP,
                SHIELD_BASH,
                TAUNT,
                SHIELD_WALL,
                SECOND_WIND,
                SHIELD_BLOCK,
            ],
            Class::Paladin => [
                CRUSADER_STRIKE,
                HOLY_LIGHT,
                JUDGMENT,
                HAMMER_OF_JUSTICE,
                DIVINE_PROTECTION,
                CONSECRATION,
                LAY_ON_HANDS,
            ],
            Class::Monk => [
                TIGER_PALM,
                BLACKOUT_KICK,
                RISING_SUN_KICK,
                LEG_SWEEP,
                FORTIFYING_BREW,
                SPINNING_CRANE_KICK,
                ROLL,
            ],
            Class::Rogue => [
                SINISTER_STRIKE,
                EVISCERATE,
                BACKSTAB,
                GOUGE,
                EVASION,
                KICK,
                SPRINT,
            ],
            Class::Ranger => [
                STEADY_SHOT,
                SERPENT_STING,
                CONCUSSIVE_SHOT,
                MULTI_SHOT,
                HAWK_EYE,
                KILL_SHOT,
                DISENGAGE,
            ],
            Class::Artificer => [
                ARCANE_RIFLE,
                ACID_FLASK,
                SHOCK_NET,
                THUNDER_GRENADE,
                ARCANE_ARMOR,
                INFUSED_TONIC,
                ROCKET_BOOTS,
            ],
            Class::Bard => [
                VICIOUS_MOCKERY,
                HEALING_WORD,
                THUNDERWAVE,
                SONG_OF_REST,
                INSPIRE,
                HYPNOTIC_PATTERN,
                DISSONANT_WHISPERS,
            ],
            Class::Cleric => [
                SMITE,
                HEAL,
                SHADOW_WORD_PAIN,
                RENEW,
                POWER_WORD_SHIELD,
                HOLY_NOVA,
                FLASH_HEAL,
            ],
            Class::Druid => [
                WRATH,
                REJUVENATION,
                MOONFIRE,
                ENTANGLING_ROOTS,
                HEALING_TOUCH,
                STARFALL,
                DASH,
            ],
            Class::Mage => [
                FIREBALL,
                FROSTBOLT,
                FIRE_BLAST,
                FROST_NOVA,
                ICE_BARRIER,
                EVOCATION,
                BLINK,
            ],
            Class::Sorcerer => [
                CHAOS_BOLT,
                ARCANE_BARRAGE,
                HOLD_PERSON,
                METEOR,
                MANA_SHIELD,
                WILD_SURGE,
                MISTY_STEP,
            ],
            Class::Warlock => [
                ELDRITCH_BLAST,
                CORRUPTION,
                DRAIN_LIFE,
                CURSE_OF_WEAKNESS,
                SHADOW_WARD,
                SOUL_FIRE,
                SIPHON_SOUL,
            ],
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

    /// Rogues and monks build points to spend on finishers.
    pub fn uses_combo_points(self) -> bool {
        self.abilities()
            .iter()
            .any(|a| ability(*a).needs_combo_points())
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

// ---- Abilities ----

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
    /// Your hostile target and every enemy within this radius of it.
    AroundTarget(f32),
}

impl Targeting {
    /// Needs a hostile target to aim at.
    pub fn needs_enemy(self) -> bool {
        matches!(self, Targeting::Enemy | Targeting::AroundTarget(_))
    }
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
    /// Movement speed is multiplied by this (less than 1).
    Slow(f32),
    /// Movement speed is multiplied by this (more than 1).
    Speed(f32),
    /// Can't move, act or cast.
    Stun,
    /// Can't move.
    Root,
    /// Soaks up this much damage.
    Absorb(f32),
    /// Damage taken is multiplied by this.
    DamageTaken(f32),
    /// Damage done is multiplied by this.
    DamageDone(f32),
}

impl AuraKind {
    pub fn harmful(self) -> bool {
        match self {
            AuraKind::Dot { .. } | AuraKind::Slow(_) | AuraKind::Stun | AuraKind::Root => true,
            AuraKind::DamageTaken(f) => f > 1.0,
            AuraKind::DamageDone(f) => f < 1.0,
            AuraKind::Hot { .. } | AuraKind::Speed(_) | AuraKind::Absorb(_) => false,
        }
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
    /// Gives the caster back this fraction of their maximum power.
    RestorePower(f32),
    /// Adds a combo point (rogues) or chi (monks).
    ComboPoint,
    /// Spends every combo point for damage.
    Finisher {
        min: f32,
        max: f32,
        per_point: f32,
    },
    /// Rush to the target.
    Charge,
    /// Jump this far along your facing (negative: backwards).
    Leap(f32),
    /// Damages the target and heals the caster by as much.
    Drain {
        min: f32,
        max: f32,
    },
}

#[derive(Debug)]
pub struct Ability {
    pub name: &'static str,
    pub description: &'static str,
    pub school: School,
    pub targeting: Targeting,
    pub range: f32,
    /// Too close to use it (Charge).
    pub min_range: f32,
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

    /// Whether it moves the caster.
    pub fn moves_caster(&self) -> bool {
        self.effects
            .iter()
            .any(|e| matches!(e, Effect::Charge | Effect::Leap(_)))
    }
}

pub fn ability(id: AbilityId) -> &'static Ability {
    &ABILITIES[id.0 as usize]
}

pub mod ids {
    use super::AbilityId;

    // Barbarian (originally the Warrior) and Fighter.
    pub const HEROIC_STRIKE: AbilityId = AbilityId(0);
    pub const REND: AbilityId = AbilityId(1);
    pub const SHIELD_BASH: AbilityId = AbilityId(2);
    pub const THUNDER_CLAP: AbilityId = AbilityId(3);
    pub const TAUNT: AbilityId = AbilityId(4);
    pub const SECOND_WIND: AbilityId = AbilityId(5);
    // Mage
    pub const FIREBALL: AbilityId = AbilityId(6);
    pub const FROSTBOLT: AbilityId = AbilityId(7);
    pub const FIRE_BLAST: AbilityId = AbilityId(8);
    pub const FROST_NOVA: AbilityId = AbilityId(9);
    pub const ICE_BARRIER: AbilityId = AbilityId(10);
    pub const EVOCATION: AbilityId = AbilityId(11);
    // Cleric
    pub const SMITE: AbilityId = AbilityId(12);
    pub const SHADOW_WORD_PAIN: AbilityId = AbilityId(13);
    pub const HEAL: AbilityId = AbilityId(14);
    pub const RENEW: AbilityId = AbilityId(15);
    pub const POWER_WORD_SHIELD: AbilityId = AbilityId(16);
    pub const HOLY_NOVA: AbilityId = AbilityId(17);
    // Mobs
    pub const SAVAGE_BITE: AbilityId = AbilityId(18);
    pub const SHADOW_BOLT: AbilityId = AbilityId(19);
    pub const GROUND_SLAM: AbilityId = AbilityId(20);
    // Rogue
    pub const SINISTER_STRIKE: AbilityId = AbilityId(21);
    pub const EVISCERATE: AbilityId = AbilityId(22);
    pub const BACKSTAB: AbilityId = AbilityId(23);
    pub const GOUGE: AbilityId = AbilityId(24);
    pub const EVASION: AbilityId = AbilityId(25);
    pub const KICK: AbilityId = AbilityId(26);
    // E abilities and the rest of the Barbarian and Fighter.
    pub const CHARGE: AbilityId = AbilityId(27);
    pub const WHIRLWIND: AbilityId = AbilityId(28);
    pub const SKULL_BASH: AbilityId = AbilityId(29);
    pub const RECKLESSNESS: AbilityId = AbilityId(30);
    pub const POWER_STRIKE: AbilityId = AbilityId(31);
    pub const SHIELD_WALL: AbilityId = AbilityId(32);
    pub const SHIELD_BLOCK: AbilityId = AbilityId(33);
    pub const BLINK: AbilityId = AbilityId(34);
    pub const FLASH_HEAL: AbilityId = AbilityId(35);
    pub const SPRINT: AbilityId = AbilityId(36);
    // Ranger
    pub const STEADY_SHOT: AbilityId = AbilityId(37);
    pub const SERPENT_STING: AbilityId = AbilityId(38);
    pub const MULTI_SHOT: AbilityId = AbilityId(39);
    pub const CONCUSSIVE_SHOT: AbilityId = AbilityId(40);
    pub const HAWK_EYE: AbilityId = AbilityId(41);
    pub const KILL_SHOT: AbilityId = AbilityId(42);
    pub const DISENGAGE: AbilityId = AbilityId(43);
    // Sorcerer
    pub const CHAOS_BOLT: AbilityId = AbilityId(44);
    pub const ARCANE_BARRAGE: AbilityId = AbilityId(45);
    pub const METEOR: AbilityId = AbilityId(46);
    pub const HOLD_PERSON: AbilityId = AbilityId(47);
    pub const MANA_SHIELD: AbilityId = AbilityId(48);
    pub const WILD_SURGE: AbilityId = AbilityId(49);
    pub const MISTY_STEP: AbilityId = AbilityId(50);
    // Paladin
    pub const CRUSADER_STRIKE: AbilityId = AbilityId(51);
    pub const HOLY_LIGHT: AbilityId = AbilityId(52);
    pub const JUDGMENT: AbilityId = AbilityId(53);
    pub const HAMMER_OF_JUSTICE: AbilityId = AbilityId(54);
    pub const DIVINE_PROTECTION: AbilityId = AbilityId(55);
    pub const CONSECRATION: AbilityId = AbilityId(56);
    pub const LAY_ON_HANDS: AbilityId = AbilityId(57);
    // Druid
    pub const WRATH: AbilityId = AbilityId(58);
    pub const REJUVENATION: AbilityId = AbilityId(59);
    pub const MOONFIRE: AbilityId = AbilityId(60);
    pub const ENTANGLING_ROOTS: AbilityId = AbilityId(61);
    pub const HEALING_TOUCH: AbilityId = AbilityId(62);
    pub const STARFALL: AbilityId = AbilityId(63);
    pub const DASH: AbilityId = AbilityId(64);
    // Artificer
    pub const ARCANE_RIFLE: AbilityId = AbilityId(65);
    pub const ACID_FLASK: AbilityId = AbilityId(66);
    pub const THUNDER_GRENADE: AbilityId = AbilityId(67);
    pub const SHOCK_NET: AbilityId = AbilityId(68);
    pub const ARCANE_ARMOR: AbilityId = AbilityId(69);
    pub const INFUSED_TONIC: AbilityId = AbilityId(70);
    pub const ROCKET_BOOTS: AbilityId = AbilityId(71);
    // Warlock
    pub const ELDRITCH_BLAST: AbilityId = AbilityId(72);
    pub const CORRUPTION: AbilityId = AbilityId(73);
    pub const DRAIN_LIFE: AbilityId = AbilityId(74);
    pub const CURSE_OF_WEAKNESS: AbilityId = AbilityId(75);
    pub const SHADOW_WARD: AbilityId = AbilityId(76);
    pub const SOUL_FIRE: AbilityId = AbilityId(77);
    pub const SIPHON_SOUL: AbilityId = AbilityId(78);
    // Monk
    pub const TIGER_PALM: AbilityId = AbilityId(79);
    pub const BLACKOUT_KICK: AbilityId = AbilityId(80);
    pub const RISING_SUN_KICK: AbilityId = AbilityId(81);
    pub const LEG_SWEEP: AbilityId = AbilityId(82);
    pub const FORTIFYING_BREW: AbilityId = AbilityId(83);
    pub const SPINNING_CRANE_KICK: AbilityId = AbilityId(84);
    pub const ROLL: AbilityId = AbilityId(85);
    // Bard
    pub const VICIOUS_MOCKERY: AbilityId = AbilityId(86);
    pub const HEALING_WORD: AbilityId = AbilityId(87);
    pub const THUNDERWAVE: AbilityId = AbilityId(88);
    pub const SONG_OF_REST: AbilityId = AbilityId(89);
    pub const INSPIRE: AbilityId = AbilityId(90);
    pub const HYPNOTIC_PATTERN: AbilityId = AbilityId(91);
    pub const DISSONANT_WHISPERS: AbilityId = AbilityId(92);
    // More mob abilities.
    pub const VENOM_STING: AbilityId = AbilityId(93);
    pub const WEB: AbilityId = AbilityId(94);
    pub const LIGHTNING_BOLT: AbilityId = AbilityId(95);
    // The Sunken King's spells. Interrupt them!
    pub const DROWNING_GRASP: AbilityId = AbilityId(96);
    pub const CALL_OF_THE_DEEP: AbilityId = AbilityId(97);
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
        min_range: 0.0,
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

const fn min_range(mut a: Ability, min: f32) -> Ability {
    a.min_range = min;
    a
}

use AuraKind as A;
use Effect as E;
use School as S;
use Targeting as T;

const M: f32 = MELEE_RANGE;

/// Every ability, indexed by `AbilityId`. Damage and healing are at level 1
/// and grow with `level_scale`.
pub static ABILITIES: &[Ability] = &[
    // 0-5: Barbarian and Fighter
    threat(
        ab(
            "Savage Strike",
            "A brutal blow that causes a high amount of threat.",
            S::Physical,
            T::Enemy,
            M,
            0.0,
            0.0,
            15.0,
            &[E::Damage {
                min: 15.0,
                max: 19.0,
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
            M,
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
        "Bashes the target with your shield, interrupting spellcasting and stunning it for 2 sec.",
        S::Physical,
        T::Enemy,
        M,
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
    // 6-11: Mage
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
    // 12-17: Cleric
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
    // 18-20: Mobs
    ab(
        "Savage Bite",
        "A bite that bleeds over 9 sec.",
        S::Physical,
        T::Enemy,
        M,
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
    // 21-26: Rogue
    ab(
        "Sinister Strike",
        "A quick strike that adds a combo point.",
        S::Physical,
        T::Enemy,
        M,
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
        M,
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
        M,
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
        M,
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
        M,
        0.0,
        10.0,
        25.0,
        &[E::Damage { min: 4.0, max: 5.0 }, E::Interrupt],
    ),
    // 27-36: E abilities, and the rest of the Barbarian and Fighter
    min_range(
        ab(
            "Charge",
            "Rush to an enemy 8 to 25 yards away, stunning it for 1 sec and gaining 15 rage.",
            S::Physical,
            T::Enemy,
            25.0,
            0.0,
            15.0,
            0.0,
            &[
                E::Charge,
                E::Aura {
                    kind: A::Stun,
                    duration: 1.0,
                },
                E::RestorePower(0.15),
            ],
        ),
        8.0,
    ),
    threat(
        ab(
            "Whirlwind",
            "Spin with your weapon, hitting every enemy within 8 yards.",
            S::Physical,
            T::AroundCaster(8.0),
            0.0,
            0.0,
            8.0,
            20.0,
            &[E::Damage {
                min: 12.0,
                max: 15.0,
            }],
        ),
        1.5,
    ),
    ab(
        "Skull Bash",
        "Bashes the target's skull, interrupting spellcasting and stunning it for 2 sec.",
        S::Physical,
        T::Enemy,
        M,
        0.0,
        12.0,
        10.0,
        &[
            E::Damage { min: 7.0, max: 9.0 },
            E::Interrupt,
            E::Aura {
                kind: A::Stun,
                duration: 2.0,
            },
        ],
    ),
    ab(
        "Recklessness",
        "Fly into a rage: you deal 30% more damage for 12 sec.",
        S::Physical,
        T::Caster,
        0.0,
        0.0,
        60.0,
        0.0,
        &[E::Aura {
            kind: A::DamageDone(1.3),
            duration: 12.0,
        }],
    ),
    threat(
        ab(
            "Power Strike",
            "A strong, well-aimed sword strike.",
            S::Physical,
            T::Enemy,
            M,
            0.0,
            0.0,
            12.0,
            &[E::Damage {
                min: 13.0,
                max: 17.0,
            }],
        ),
        1.5,
    ),
    ab(
        "Shield Wall",
        "Take 50% less damage for 10 sec.",
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
        "Shield Block",
        "Raise your shield, absorbing damage for 6 sec.",
        S::Physical,
        T::Caster,
        0.0,
        0.0,
        12.0,
        10.0,
        &[E::Aura {
            kind: A::Absorb(35.0),
            duration: 6.0,
        }],
    ),
    ab(
        "Blink",
        "Teleport 15 yards forward.",
        S::Arcane,
        T::Caster,
        0.0,
        0.0,
        15.0,
        10.0,
        &[E::Leap(15.0)],
    ),
    ab(
        "Flash Heal",
        "A quick heal on a friendly target.",
        S::Holy,
        T::Friendly,
        40.0,
        1.0,
        0.0,
        16.0,
        &[E::Heal {
            min: 22.0,
            max: 27.0,
        }],
    ),
    ab(
        "Sprint",
        "Run 70% faster for 6 sec.",
        S::Physical,
        T::Caster,
        0.0,
        0.0,
        30.0,
        0.0,
        &[E::Aura {
            kind: A::Speed(1.7),
            duration: 6.0,
        }],
    ),
    // 37-43: Ranger
    projectile(ab(
        "Steady Shot",
        "A carefully aimed arrow.",
        S::Physical,
        T::Enemy,
        30.0,
        1.5,
        0.0,
        20.0,
        &[E::Damage {
            min: 15.0,
            max: 18.0,
        }],
    )),
    ab(
        "Serpent Sting",
        "Poisons the target over 15 sec.",
        S::Nature,
        T::Enemy,
        30.0,
        0.0,
        0.0,
        15.0,
        &[E::Aura {
            kind: A::Dot {
                per_tick: 6.0,
                interval: 3.0,
            },
            duration: 15.0,
        }],
    ),
    projectile(ab(
        "Multi-Shot",
        "Shoots your target and every enemy within 6 yards of it.",
        S::Physical,
        T::AroundTarget(6.0),
        30.0,
        0.0,
        8.0,
        30.0,
        &[E::Damage {
            min: 9.0,
            max: 11.0,
        }],
    )),
    projectile(ab(
        "Concussive Shot",
        "Dazes the target, slowing it by 50% for 6 sec.",
        S::Physical,
        T::Enemy,
        30.0,
        0.0,
        10.0,
        15.0,
        &[
            E::Damage { min: 5.0, max: 6.0 },
            E::Aura {
                kind: A::Slow(0.5),
                duration: 6.0,
            },
        ],
    )),
    ab(
        "Hawk Eye",
        "Your focus sharpens: you deal 20% more damage for 20 sec.",
        S::Nature,
        T::Caster,
        0.0,
        0.0,
        45.0,
        0.0,
        &[E::Aura {
            kind: A::DamageDone(1.2),
            duration: 20.0,
        }],
    ),
    projectile(ab(
        "Kill Shot",
        "A powerful shot.",
        S::Physical,
        T::Enemy,
        30.0,
        0.0,
        10.0,
        35.0,
        &[E::Damage {
            min: 26.0,
            max: 31.0,
        }],
    )),
    ab(
        "Disengage",
        "Leap 12 yards backwards.",
        S::Physical,
        T::Caster,
        0.0,
        0.0,
        15.0,
        0.0,
        &[E::Leap(-12.0)],
    ),
    // 44-50: Sorcerer
    projectile(ab(
        "Chaos Bolt",
        "A bolt of raw, wild magic.",
        S::Arcane,
        T::Enemy,
        30.0,
        2.0,
        0.0,
        13.0,
        &[E::Damage {
            min: 20.0,
            max: 26.0,
        }],
    )),
    projectile(ab(
        "Arcane Barrage",
        "Instantly fires a volley of arcane missiles.",
        S::Arcane,
        T::Enemy,
        30.0,
        0.0,
        6.0,
        12.0,
        &[E::Damage {
            min: 13.0,
            max: 16.0,
        }],
    )),
    ab(
        "Meteor",
        "Calls down a meteor that hits your target and every enemy within 7 yards of it.",
        S::Fire,
        T::AroundTarget(7.0),
        30.0,
        2.5,
        12.0,
        22.0,
        &[E::Damage {
            min: 20.0,
            max: 24.0,
        }],
    ),
    ab(
        "Hold Person",
        "Holds the target in place, stunning it for 4 sec.",
        S::Arcane,
        T::Enemy,
        30.0,
        0.0,
        20.0,
        12.0,
        &[
            E::Interrupt,
            E::Aura {
                kind: A::Stun,
                duration: 4.0,
            },
        ],
    ),
    ab(
        "Mana Shield",
        "Shields you, absorbing damage for 30 sec.",
        S::Arcane,
        T::Caster,
        0.0,
        0.0,
        30.0,
        16.0,
        &[E::Aura {
            kind: A::Absorb(50.0),
            duration: 30.0,
        }],
    ),
    ab(
        "Wild Surge",
        "Wild magic surges through you: you deal 35% more damage for 15 sec.",
        S::Arcane,
        T::Caster,
        0.0,
        0.0,
        90.0,
        0.0,
        &[E::Aura {
            kind: A::DamageDone(1.35),
            duration: 15.0,
        }],
    ),
    ab(
        "Misty Step",
        "Vanish in silver mist and reappear 12 yards ahead.",
        S::Arcane,
        T::Caster,
        0.0,
        0.0,
        12.0,
        8.0,
        &[E::Leap(12.0)],
    ),
    // 51-57: Paladin
    ab(
        "Crusader Strike",
        "A holy-charged melee strike.",
        S::Holy,
        T::Enemy,
        M,
        0.0,
        0.0,
        10.0,
        &[E::Damage {
            min: 12.0,
            max: 15.0,
        }],
    ),
    ab(
        "Holy Light",
        "Heals a friendly target.",
        S::Holy,
        T::Friendly,
        40.0,
        2.0,
        0.0,
        16.0,
        &[E::Heal {
            min: 30.0,
            max: 36.0,
        }],
    ),
    ab(
        "Judgment",
        "Unleashes holy judgment on an enemy within 10 yards.",
        S::Holy,
        T::Enemy,
        10.0,
        0.0,
        8.0,
        12.0,
        &[E::Damage {
            min: 14.0,
            max: 18.0,
        }],
    ),
    ab(
        "Hammer of Justice",
        "Stuns the target for 4 sec.",
        S::Holy,
        T::Enemy,
        10.0,
        0.0,
        30.0,
        10.0,
        &[
            E::Interrupt,
            E::Aura {
                kind: A::Stun,
                duration: 4.0,
            },
        ],
    ),
    ab(
        "Divine Protection",
        "Take 50% less damage for 8 sec.",
        S::Holy,
        T::Caster,
        0.0,
        0.0,
        60.0,
        10.0,
        &[E::Aura {
            kind: A::DamageTaken(0.5),
            duration: 8.0,
        }],
    ),
    ab(
        "Consecration",
        "Consecrates the ground, burning every enemy within 8 yards over 8 sec.",
        S::Holy,
        T::AroundCaster(8.0),
        0.0,
        0.0,
        10.0,
        18.0,
        &[E::Aura {
            kind: A::Dot {
                per_tick: 5.0,
                interval: 2.0,
            },
            duration: 8.0,
        }],
    ),
    ab(
        "Lay on Hands",
        "Heals a friendly target for a huge amount.",
        S::Holy,
        T::Friendly,
        40.0,
        0.0,
        120.0,
        0.0,
        &[E::Heal {
            min: 80.0,
            max: 90.0,
        }],
    ),
    // 58-64: Druid
    projectile(ab(
        "Wrath",
        "Hurls a ball of nature's fury at the target.",
        S::Nature,
        T::Enemy,
        30.0,
        2.0,
        0.0,
        10.0,
        &[E::Damage {
            min: 16.0,
            max: 20.0,
        }],
    )),
    ab(
        "Rejuvenation",
        "Heals a friendly target over 12 sec.",
        S::Nature,
        T::Friendly,
        40.0,
        0.0,
        0.0,
        12.0,
        &[E::Aura {
            kind: A::Hot {
                per_tick: 8.0,
                interval: 2.0,
            },
            duration: 12.0,
        }],
    ),
    ab(
        "Moonfire",
        "Burns the target with moonlight, then more over 12 sec.",
        S::Arcane,
        T::Enemy,
        30.0,
        0.0,
        0.0,
        12.0,
        &[
            E::Damage { min: 6.0, max: 8.0 },
            E::Aura {
                kind: A::Dot {
                    per_tick: 4.0,
                    interval: 3.0,
                },
                duration: 12.0,
            },
        ],
    ),
    ab(
        "Entangling Roots",
        "Roots the target in place for 8 sec.",
        S::Nature,
        T::Enemy,
        30.0,
        1.5,
        0.0,
        12.0,
        &[E::Aura {
            kind: A::Root,
            duration: 8.0,
        }],
    ),
    ab(
        "Healing Touch",
        "A slow but powerful heal.",
        S::Nature,
        T::Friendly,
        40.0,
        2.5,
        0.0,
        20.0,
        &[E::Heal {
            min: 44.0,
            max: 52.0,
        }],
    ),
    ab(
        "Starfall",
        "Stars rain down on every enemy within 10 yards.",
        S::Arcane,
        T::AroundCaster(10.0),
        0.0,
        0.0,
        12.0,
        18.0,
        &[E::Damage {
            min: 12.0,
            max: 15.0,
        }],
    ),
    ab(
        "Dash",
        "Take cat form and run 70% faster for 5 sec.",
        S::Nature,
        T::Caster,
        0.0,
        0.0,
        25.0,
        6.0,
        &[E::Aura {
            kind: A::Speed(1.7),
            duration: 5.0,
        }],
    ),
    // 65-71: Artificer
    projectile(ab(
        "Arcane Rifle",
        "Fires an arcane-charged round.",
        S::Arcane,
        T::Enemy,
        25.0,
        0.0,
        0.0,
        12.0,
        &[E::Damage {
            min: 12.0,
            max: 15.0,
        }],
    )),
    projectile(ab(
        "Acid Flask",
        "Throws a flask of acid that burns over 12 sec.",
        S::Nature,
        T::Enemy,
        25.0,
        0.0,
        0.0,
        10.0,
        &[
            E::Damage { min: 3.0, max: 4.0 },
            E::Aura {
                kind: A::Dot {
                    per_tick: 5.0,
                    interval: 3.0,
                },
                duration: 12.0,
            },
        ],
    )),
    projectile(ab(
        "Thunder Grenade",
        "Hits your target and every enemy within 6 yards of it, slowing them.",
        S::Nature,
        T::AroundTarget(6.0),
        25.0,
        0.0,
        10.0,
        18.0,
        &[
            E::Damage {
                min: 11.0,
                max: 14.0,
            },
            E::Aura {
                kind: A::Slow(0.6),
                duration: 6.0,
            },
        ],
    )),
    projectile(ab(
        "Shock Net",
        "Entangles the target in a crackling net for 5 sec.",
        S::Arcane,
        T::Enemy,
        25.0,
        0.0,
        15.0,
        10.0,
        &[
            E::Damage { min: 3.0, max: 4.0 },
            E::Aura {
                kind: A::Root,
                duration: 5.0,
            },
        ],
    )),
    ab(
        "Arcane Armor",
        "Arcane plates absorb damage for 30 sec.",
        S::Arcane,
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
        "Infused Tonic",
        "A healing tonic that works over 10 sec.",
        S::Nature,
        T::Friendly,
        40.0,
        0.0,
        12.0,
        14.0,
        &[
            E::Heal {
                min: 12.0,
                max: 15.0,
            },
            E::Aura {
                kind: A::Hot {
                    per_tick: 8.0,
                    interval: 2.0,
                },
                duration: 10.0,
            },
        ],
    ),
    ab(
        "Rocket Boots",
        "Blast 14 yards forward.",
        S::Fire,
        T::Caster,
        0.0,
        0.0,
        15.0,
        8.0,
        &[E::Leap(14.0)],
    ),
    // 72-78: Warlock
    projectile(ab(
        "Eldritch Blast",
        "A crackling beam of dark energy.",
        S::Shadow,
        T::Enemy,
        30.0,
        2.0,
        0.0,
        12.0,
        &[E::Damage {
            min: 18.0,
            max: 22.0,
        }],
    )),
    ab(
        "Corruption",
        "Corrupts the target, dealing shadow damage over 18 sec.",
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
        "Drain Life",
        "Drains life from the target, healing you as much.",
        S::Shadow,
        T::Enemy,
        30.0,
        1.5,
        6.0,
        14.0,
        &[E::Drain {
            min: 12.0,
            max: 15.0,
        }],
    ),
    ab(
        "Curse of Weakness",
        "The target deals 30% less damage for 15 sec.",
        S::Shadow,
        T::Enemy,
        30.0,
        0.0,
        0.0,
        10.0,
        &[E::Aura {
            kind: A::DamageDone(0.7),
            duration: 15.0,
        }],
    ),
    ab(
        "Shadow Ward",
        "Shields you, absorbing damage for 30 sec.",
        S::Shadow,
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
    projectile(ab(
        "Soul Fire",
        "A slow, devastating bolt of fel fire.",
        S::Fire,
        T::Enemy,
        30.0,
        3.0,
        15.0,
        20.0,
        &[E::Damage {
            min: 40.0,
            max: 48.0,
        }],
    )),
    ab(
        "Siphon Soul",
        "Instantly rips life from the target, healing you as much.",
        S::Shadow,
        T::Enemy,
        30.0,
        0.0,
        20.0,
        12.0,
        &[E::Drain {
            min: 16.0,
            max: 20.0,
        }],
    ),
    // 79-85: Monk
    ab(
        "Tiger Palm",
        "A quick palm strike that builds chi.",
        S::Physical,
        T::Enemy,
        M,
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
        "Blackout Kick",
        "A finishing kick that spends all your chi. More chi, more damage.",
        S::Physical,
        T::Enemy,
        M,
        0.0,
        0.0,
        30.0,
        &[E::Finisher {
            min: 6.0,
            max: 9.0,
            per_point: 10.0,
        }],
    ),
    ab(
        "Rising Sun Kick",
        "A powerful rising kick that builds chi.",
        S::Physical,
        T::Enemy,
        M,
        0.0,
        8.0,
        40.0,
        &[
            E::Damage {
                min: 18.0,
                max: 22.0,
            },
            E::ComboPoint,
        ],
    ),
    ab(
        "Leg Sweep",
        "Sweeps the legs of every enemy within 6 yards, stunning them for 2 sec.",
        S::Physical,
        T::AroundCaster(6.0),
        0.0,
        0.0,
        20.0,
        25.0,
        &[
            E::Interrupt,
            E::Aura {
                kind: A::Stun,
                duration: 2.0,
            },
        ],
    ),
    ab(
        "Fortifying Brew",
        "Take 40% less damage for 10 sec.",
        S::Nature,
        T::Caster,
        0.0,
        0.0,
        60.0,
        0.0,
        &[E::Aura {
            kind: A::DamageTaken(0.6),
            duration: 10.0,
        }],
    ),
    ab(
        "Spinning Crane Kick",
        "Spin and kick every enemy within 8 yards.",
        S::Physical,
        T::AroundCaster(8.0),
        0.0,
        0.0,
        8.0,
        40.0,
        &[E::Damage {
            min: 11.0,
            max: 14.0,
        }],
    ),
    ab(
        "Roll",
        "Roll 10 yards forward.",
        S::Physical,
        T::Caster,
        0.0,
        0.0,
        10.0,
        0.0,
        &[E::Leap(10.0)],
    ),
    // 86-92: Bard
    ab(
        "Vicious Mockery",
        "A cutting insult that hurts and makes the target deal 15% less damage for 6 sec.",
        S::Arcane,
        T::Enemy,
        30.0,
        0.0,
        0.0,
        10.0,
        &[
            E::Damage {
                min: 14.0,
                max: 17.0,
            },
            E::Aura {
                kind: A::DamageDone(0.85),
                duration: 6.0,
            },
        ],
    ),
    ab(
        "Healing Word",
        "An instant word of healing.",
        S::Holy,
        T::Friendly,
        40.0,
        0.0,
        0.0,
        14.0,
        &[E::Heal {
            min: 20.0,
            max: 25.0,
        }],
    ),
    ab(
        "Thunderwave",
        "A wave of thunderous sound damages and slows every enemy within 8 yards.",
        S::Nature,
        T::AroundCaster(8.0),
        0.0,
        0.0,
        10.0,
        16.0,
        &[
            E::Damage {
                min: 11.0,
                max: 14.0,
            },
            E::Aura {
                kind: A::Slow(0.6),
                duration: 6.0,
            },
        ],
    ),
    ab(
        "Song of Rest",
        "A soothing song that heals a friendly target over 15 sec.",
        S::Nature,
        T::Friendly,
        40.0,
        0.0,
        0.0,
        12.0,
        &[E::Aura {
            kind: A::Hot {
                per_tick: 8.0,
                interval: 3.0,
            },
            duration: 15.0,
        }],
    ),
    ab(
        "Inspire",
        "Inspires a friendly target: they deal 25% more damage for 15 sec.",
        S::Arcane,
        T::Friendly,
        40.0,
        0.0,
        30.0,
        12.0,
        &[E::Aura {
            kind: A::DamageDone(1.25),
            duration: 15.0,
        }],
    ),
    ab(
        "Hypnotic Pattern",
        "Mesmerizes the target, stunning it for 6 sec.",
        S::Arcane,
        T::Enemy,
        30.0,
        1.5,
        30.0,
        14.0,
        &[
            E::Interrupt,
            E::Aura {
                kind: A::Stun,
                duration: 6.0,
            },
        ],
    ),
    ab(
        "Dissonant Whispers",
        "Whispers a maddening melody, interrupting spellcasting.",
        S::Arcane,
        T::Enemy,
        30.0,
        0.0,
        12.0,
        10.0,
        &[
            E::Damage {
                min: 8.0,
                max: 10.0,
            },
            E::Interrupt,
        ],
    ),
    // 93-95: More mob abilities
    ab(
        "Venom Sting",
        "A poisonous sting that burns over 9 sec.",
        S::Nature,
        T::Enemy,
        M,
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
    ab(
        "Web",
        "Spits a sticky web that roots the target for 4 sec.",
        S::Nature,
        T::Enemy,
        20.0,
        0.0,
        0.0,
        0.0,
        &[E::Aura {
            kind: A::Root,
            duration: 4.0,
        }],
    ),
    projectile(ab(
        "Lightning Bolt",
        "Hurls a bolt of lightning at the target.",
        S::Nature,
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
    // 96-97: The Sunken King's interruptible spells
    projectile(ab(
        "Drowning Grasp",
        "Floods the target's lungs with black water.",
        S::Shadow,
        T::Enemy,
        40.0,
        2.5,
        0.0,
        0.0,
        &[E::Damage {
            min: 28.0,
            max: 34.0,
        }],
    )),
    ab(
        "Call of the Deep",
        "Calls on the deep to mend the caster's wounds.",
        S::Holy,
        T::Caster,
        0.0,
        3.0,
        0.0,
        0.0,
        &[E::Heal {
            min: 140.0,
            max: 145.0,
        }],
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
    /// Used up to restore this fraction of health and power.
    Potion { health: f32, power: f32 },
    /// Eaten to restore this fraction of health. Shares the potion cooldown.
    Food { health: f32 },
    /// Held in the weapon slot. Any class can use any weapon.
    Weapon {
        /// Added to every auto attack, before scaling with level.
        damage: f32,
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
    /// What merchants charge for it, in copper. They pay a quarter of this.
    pub price: u32,
}

impl Item {
    /// What a merchant pays for one.
    pub fn sell_price(&self) -> u32 {
        (self.price / 4).max(1)
    }
}

pub fn item(id: ItemId) -> &'static Item {
    &ITEMS[id.0 as usize]
}

pub mod items {
    use super::ItemId;

    pub const LIGHT_LEATHER: ItemId = ItemId(0);
    pub const LINEN_CLOTH: ItemId = ItemId(1);
    pub const ANCIENT_CORE: ItemId = ItemId(2);
    pub const LEATHER_CAP: ItemId = ItemId(3);
    pub const LEATHER_VEST: ItemId = ItemId(4);
    pub const LEATHER_GLOVES: ItemId = ItemId(5);
    pub const LEATHER_PANTS: ItemId = ItemId(6);
    pub const LEATHER_BOOTS: ItemId = ItemId(7);
    pub const LINEN_HOOD: ItemId = ItemId(8);
    pub const LINEN_ROBE: ItemId = ItemId(9);
    pub const LINEN_PANTS: ItemId = ItemId(10);
    pub const HEARTSTONE_CHESTGUARD: ItemId = ItemId(11);
    pub const HEALING_POTION: ItemId = ItemId(12);
    pub const MANA_POTION: ItemId = ItemId(13);
    // Rare (green) drops.
    pub const WOLFHIDE_HELM: ItemId = ItemId(14);
    pub const SILKWEAVE_CIRCLET: ItemId = ItemId(15);
    pub const IRONBARK_JERKIN: ItemId = ItemId(16);
    pub const MOONTHREAD_VESTMENT: ItemId = ItemId(17);
    pub const BRAWLERS_GRIPS: ItemId = ItemId(18);
    pub const SPELLWEAVER_GLOVES: ItemId = ItemId(19);
    pub const RIDGERUNNER_LEGGINGS: ItemId = ItemId(20);
    pub const STARWEAVE_TROUSERS: ItemId = ItemId(21);
    pub const TRAILBLAZER_BOOTS: ItemId = ItemId(22);
    pub const WHISPERSTEP_SLIPPERS: ItemId = ItemId(23);
    pub const IRON_SCRAP: ItemId = ItemId(24);
    pub const LINEN_GLOVES: ItemId = ItemId(25);
    pub const LINEN_SANDALS: ItemId = ItemId(26);
    // Weapons: crafted commons, then green drops, then the rare crafted one.
    pub const IRON_SWORD: ItemId = ItemId(27);
    pub const HUNTING_BOW: ItemId = ItemId(28);
    pub const APPRENTICE_STAFF: ItemId = ItemId(29);
    pub const HEARTSTONE_GREATSWORD: ItemId = ItemId(30);
    pub const WOLFBITE_AXE: ItemId = ItemId(31);
    pub const IRONWOOD_MACE: ItemId = ItemId(32);
    pub const SHADOWFANG_DAGGER: ItemId = ItemId(33);
    pub const ASHWOOD_LONGBOW: ItemId = ItemId(34);
    pub const EMBERWAND: ItemId = ItemId(35);
    pub const MOONWHISPER_STAFF: ItemId = ItemId(36);
    // The Sunken Vault.
    pub const CROWN_OF_THE_SUNKEN_KING: ItemId = ItemId(37);
    // Cooking.
    pub const RAW_FISH: ItemId = ItemId(38);
    pub const BOAR_MEAT: ItemId = ItemId(39);
    pub const COOKED_FISH: ItemId = ItemId(40);
    pub const ROASTED_BOAR: ItemId = ItemId(41);
}

/// Green items any mob may drop (and quests give): a little better than
/// crafted common gear, not as good as rare crafted gear.
pub const RARE_DROPS: [ItemId; 10] = {
    use items::*;
    [
        WOLFHIDE_HELM,
        SILKWEAVE_CIRCLET,
        IRONBARK_JERKIN,
        MOONTHREAD_VESTMENT,
        BRAWLERS_GRIPS,
        SPELLWEAVER_GLOVES,
        RIDGERUNNER_LEGGINGS,
        STARWEAVE_TROUSERS,
        TRAILBLAZER_BOOTS,
        WHISPERSTEP_SLIPPERS,
    ]
};
/// Chance an ordinary mob drops one of the `RARE_DROPS`.
pub const RARE_DROP_CHANCE: f32 = 0.04;
/// Chance an elite does.
pub const ELITE_RARE_DROP_CHANCE: f32 = 0.6;

/// Green weapons any mob may drop, rolled separately from `RARE_DROPS`.
pub const WEAPON_DROPS: [ItemId; 6] = {
    use items::*;
    [
        WOLFBITE_AXE,
        IRONWOOD_MACE,
        SHADOWFANG_DAGGER,
        ASHWOOD_LONGBOW,
        EMBERWAND,
        MOONWHISPER_STAFF,
    ]
};
/// Chance an ordinary mob drops one of the `WEAPON_DROPS`.
pub const WEAPON_DROP_CHANCE: f32 = 0.03;
/// Chance an elite does.
pub const ELITE_WEAPON_DROP_CHANCE: f32 = 0.3;
/// The Sunken Vault's last boss always drops green armor, and often a weapon.
pub const SUNKEN_KING_RARE_DROP_CHANCE: f32 = 1.0;
pub const SUNKEN_KING_WEAPON_DROP_CHANCE: f32 = 0.5;

const fn material(
    name: &'static str,
    description: &'static str,
    quality: Quality,
    color: (f32, f32, f32),
    price: u32,
) -> Item {
    Item {
        name,
        description,
        kind: ItemKind::Material,
        quality,
        max_stack: 20,
        color,
        price,
    }
}

#[allow(clippy::too_many_arguments)]
const fn armor(
    name: &'static str,
    slot: Slot,
    armor: f32,
    stamina: f32,
    power: f32,
    quality: Quality,
    color: (f32, f32, f32),
    price: u32,
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
        price,
    }
}

#[allow(clippy::too_many_arguments)]
const fn weapon(
    name: &'static str,
    description: &'static str,
    damage: f32,
    stamina: f32,
    power: f32,
    quality: Quality,
    color: (f32, f32, f32),
    price: u32,
) -> Item {
    Item {
        name,
        description,
        kind: ItemKind::Weapon {
            damage,
            stamina,
            power,
        },
        quality,
        max_stack: 1,
        color,
        price,
    }
}

const fn food(
    name: &'static str,
    description: &'static str,
    health: f32,
    color: (f32, f32, f32),
) -> Item {
    Item {
        name,
        description,
        kind: ItemKind::Food { health },
        quality: Quality::Common,
        max_stack: 20,
        color,
        price: 30,
    }
}

const fn potion(
    name: &'static str,
    description: &'static str,
    health: f32,
    power: f32,
    color: (f32, f32, f32),
) -> Item {
    Item {
        name,
        description,
        kind: ItemKind::Potion { health, power },
        quality: Quality::Common,
        max_stack: 10,
        color,
        price: 50,
    }
}

const LEATHER: (f32, f32, f32) = (0.5, 0.33, 0.18);
const LINEN: (f32, f32, f32) = (0.85, 0.8, 0.68);
const IRON: (f32, f32, f32) = (0.62, 0.64, 0.68);

pub static ITEMS: [Item; 42] = [
    material(
        "Light Leather",
        "Tanned hide from the beasts of the wilds. Used to make leather armor.",
        Quality::Common,
        (0.6, 0.42, 0.25),
        30,
    ),
    material(
        "Linen Cloth",
        "A bolt of plain cloth. Used to make linen armor.",
        Quality::Common,
        (0.9, 0.86, 0.75),
        30,
    ),
    material(
        "Ancient Core",
        "The still-warm heart of an ancient elite guardian.",
        Quality::Rare,
        (0.35, 0.85, 1.0),
        2000,
    ),
    armor(
        "Leather Cap",
        Slot::Head,
        6.0,
        2.0,
        0.0,
        Quality::Common,
        LEATHER,
        160,
    ),
    armor(
        "Leather Vest",
        Slot::Chest,
        12.0,
        4.0,
        0.0,
        Quality::Common,
        LEATHER,
        240,
    ),
    armor(
        "Leather Gloves",
        Slot::Hands,
        5.0,
        2.0,
        0.0,
        Quality::Common,
        LEATHER,
        120,
    ),
    armor(
        "Leather Pants",
        Slot::Legs,
        10.0,
        3.0,
        0.0,
        Quality::Common,
        LEATHER,
        200,
    ),
    armor(
        "Leather Boots",
        Slot::Feet,
        6.0,
        2.0,
        0.0,
        Quality::Common,
        LEATHER,
        160,
    ),
    armor(
        "Linen Hood",
        Slot::Head,
        2.0,
        1.0,
        2.0,
        Quality::Common,
        LINEN,
        120,
    ),
    armor(
        "Linen Robe",
        Slot::Chest,
        4.0,
        2.0,
        4.0,
        Quality::Common,
        LINEN,
        240,
    ),
    armor(
        "Linen Pants",
        Slot::Legs,
        3.0,
        1.0,
        3.0,
        Quality::Common,
        LINEN,
        160,
    ),
    armor(
        "Heartstone Chestguard",
        Slot::Chest,
        30.0,
        10.0,
        5.0,
        Quality::Rare,
        (0.42, 0.45, 0.5),
        4000,
    ),
    potion(
        "Healing Potion",
        "Restores 35% of your health. 30 sec shared cooldown.",
        HEALING_POTION_HEALTH,
        0.0,
        (0.85, 0.15, 0.15),
    ),
    potion(
        "Mana Potion",
        "Restores 35% of your mana, rage or energy. 30 sec shared cooldown.",
        0.0,
        0.35,
        (0.2, 0.35, 0.95),
    ),
    // Greens: sturdy (armor and stamina) and arcane (power) pieces for each
    // slot, between the crafted leather and linen and the Heartstone.
    armor(
        "Wolfhide Helm",
        Slot::Head,
        9.0,
        3.0,
        1.0,
        Quality::Uncommon,
        (0.55, 0.5, 0.45),
        600,
    ),
    armor(
        "Silkweave Circlet",
        Slot::Head,
        3.0,
        2.0,
        3.0,
        Quality::Uncommon,
        (0.75, 0.6, 0.9),
        600,
    ),
    armor(
        "Ironbark Jerkin",
        Slot::Chest,
        18.0,
        6.0,
        1.0,
        Quality::Uncommon,
        (0.4, 0.32, 0.22),
        900,
    ),
    armor(
        "Moonthread Vestment",
        Slot::Chest,
        6.0,
        3.0,
        6.0,
        Quality::Uncommon,
        (0.55, 0.65, 0.9),
        900,
    ),
    armor(
        "Brawler's Grips",
        Slot::Hands,
        8.0,
        3.0,
        1.0,
        Quality::Uncommon,
        (0.6, 0.35, 0.2),
        450,
    ),
    armor(
        "Spellweaver Gloves",
        Slot::Hands,
        3.0,
        1.0,
        3.0,
        Quality::Uncommon,
        (0.35, 0.3, 0.6),
        450,
    ),
    armor(
        "Ridgerunner Leggings",
        Slot::Legs,
        15.0,
        5.0,
        1.0,
        Quality::Uncommon,
        (0.45, 0.4, 0.3),
        750,
    ),
    armor(
        "Starweave Trousers",
        Slot::Legs,
        5.0,
        2.0,
        5.0,
        Quality::Uncommon,
        (0.3, 0.3, 0.55),
        750,
    ),
    armor(
        "Trailblazer Boots",
        Slot::Feet,
        9.0,
        3.0,
        1.0,
        Quality::Uncommon,
        (0.35, 0.25, 0.15),
        600,
    ),
    armor(
        "Whisperstep Slippers",
        Slot::Feet,
        3.0,
        2.0,
        3.0,
        Quality::Uncommon,
        (0.6, 0.55, 0.75),
        600,
    ),
    material(
        "Iron Scrap",
        "Bent nails and broken blades. Used to make weapons.",
        Quality::Common,
        (0.55, 0.57, 0.6),
        40,
    ),
    armor(
        "Linen Gloves",
        Slot::Hands,
        1.0,
        1.0,
        2.0,
        Quality::Common,
        LINEN,
        100,
    ),
    armor(
        "Linen Sandals",
        Slot::Feet,
        2.0,
        1.0,
        2.0,
        Quality::Common,
        LINEN,
        120,
    ),
    weapon(
        "Iron Sword",
        "A plain, honest blade.",
        2.0,
        0.0,
        0.0,
        Quality::Common,
        IRON,
        240,
    ),
    weapon(
        "Hunting Bow",
        "Strung with leather and good for rabbits, or worse.",
        2.0,
        0.0,
        0.0,
        Quality::Common,
        LEATHER,
        240,
    ),
    weapon(
        "Apprentice Staff",
        "Wrapped in linen and humming faintly.",
        1.0,
        0.0,
        2.0,
        Quality::Common,
        (0.55, 0.42, 0.28),
        240,
    ),
    weapon(
        "Heartstone Greatsword",
        "An ancient core burns in the hilt.",
        7.0,
        6.0,
        3.0,
        Quality::Rare,
        (0.35, 0.85, 1.0),
        4000,
    ),
    weapon(
        "Wolfbite Axe",
        "",
        4.0,
        2.0,
        0.0,
        Quality::Uncommon,
        (0.6, 0.55, 0.5),
        900,
    ),
    weapon(
        "Ironwood Mace",
        "",
        4.0,
        2.0,
        0.0,
        Quality::Uncommon,
        (0.4, 0.3, 0.2),
        900,
    ),
    weapon(
        "Shadowfang Dagger",
        "",
        4.0,
        2.0,
        0.0,
        Quality::Uncommon,
        (0.3, 0.28, 0.4),
        900,
    ),
    weapon(
        "Ashwood Longbow",
        "",
        4.0,
        1.0,
        1.0,
        Quality::Uncommon,
        (0.7, 0.6, 0.45),
        900,
    ),
    weapon(
        "Emberwand",
        "",
        2.0,
        0.0,
        4.0,
        Quality::Uncommon,
        (0.95, 0.45, 0.2),
        900,
    ),
    weapon(
        "Moonwhisper Staff",
        "",
        2.0,
        0.0,
        4.0,
        Quality::Uncommon,
        (0.6, 0.7, 0.95),
        900,
    ),
    Item {
        description: "Morvane's drowned crown, still cold from the deep.",
        ..armor(
            "Crown of the Sunken King",
            Slot::Head,
            14.0,
            6.0,
            5.0,
            Quality::Rare,
            (0.3, 0.85, 0.8),
            4000,
        )
    },
    material(
        "Raw Fish",
        "Slippery and fresh from the shallows. Cook it to eat it.",
        Quality::Common,
        (0.55, 0.68, 0.75),
        10,
    ),
    material(
        "Boar Meat",
        "A tough cut of boar. Cook it to eat it.",
        Quality::Common,
        (0.75, 0.32, 0.3),
        10,
    ),
    food(
        "Cooked Fish",
        "Flaky and warm.",
        FOOD_HEALTH,
        (0.85, 0.62, 0.38),
    ),
    food(
        "Roasted Boar",
        "Charred on the outside, juicy within.",
        FOOD_HEALTH,
        (0.55, 0.3, 0.18),
    ),
];

/// Cooked food heals three quarters of what a Healing Potion does.
pub const FOOD_HEALTH: f32 = HEALING_POTION_HEALTH * 0.75;
const HEALING_POTION_HEALTH: f32 = 0.35;

/// What every merchant sells.
pub const MERCHANT_GOODS: [ItemId; 4] = [
    items::HEALING_POTION,
    items::MANA_POTION,
    items::LIGHT_LEATHER,
    items::LINEN_CLOTH,
];

/// A trade you make things with. Each has its own tab in the skills
/// window (K).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Skill {
    #[default]
    Crafting,
    Cooking,
}

impl Skill {
    pub const ALL: [Skill; 2] = [Skill::Crafting, Skill::Cooking];

    pub fn name(self) -> &'static str {
        match self {
            Skill::Crafting => "Crafting",
            Skill::Cooking => "Cooking",
        }
    }

    /// This skill's recipes, with their index in `RECIPES`.
    pub fn recipes(self) -> impl Iterator<Item = (usize, &'static Recipe)> {
        RECIPES
            .iter()
            .enumerate()
            .filter(move |(_, r)| r.skill == self)
    }
}

/// Something you can make from materials.
#[derive(Debug)]
pub struct Recipe {
    pub skill: Skill,
    pub result: ItemId,
    pub materials: &'static [(ItemId, u16)],
}

pub static RECIPES: [Recipe; 17] = {
    use items::*;
    [
        Recipe {
            skill: Skill::Crafting,
            result: LEATHER_CAP,
            materials: &[(LIGHT_LEATHER, 4)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: LEATHER_VEST,
            materials: &[(LIGHT_LEATHER, 6)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: LEATHER_GLOVES,
            materials: &[(LIGHT_LEATHER, 3)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: LEATHER_PANTS,
            materials: &[(LIGHT_LEATHER, 5)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: LEATHER_BOOTS,
            materials: &[(LIGHT_LEATHER, 4)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: LINEN_HOOD,
            materials: &[(LINEN_CLOTH, 3)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: LINEN_ROBE,
            materials: &[(LINEN_CLOTH, 6)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: LINEN_PANTS,
            materials: &[(LINEN_CLOTH, 4)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: LINEN_GLOVES,
            materials: &[(LINEN_CLOTH, 2)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: LINEN_SANDALS,
            materials: &[(LINEN_CLOTH, 3)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: HEARTSTONE_CHESTGUARD,
            materials: &[(ANCIENT_CORE, 1), (LIGHT_LEATHER, 8)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: IRON_SWORD,
            materials: &[(IRON_SCRAP, 5), (LIGHT_LEATHER, 2)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: HUNTING_BOW,
            materials: &[(IRON_SCRAP, 2), (LIGHT_LEATHER, 4)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: APPRENTICE_STAFF,
            materials: &[(IRON_SCRAP, 2), (LINEN_CLOTH, 4)],
        },
        Recipe {
            skill: Skill::Crafting,
            result: HEARTSTONE_GREATSWORD,
            materials: &[(ANCIENT_CORE, 1), (IRON_SCRAP, 8)],
        },
        Recipe {
            skill: Skill::Cooking,
            result: COOKED_FISH,
            materials: &[(RAW_FISH, 1)],
        },
        Recipe {
            skill: Skill::Cooking,
            result: ROASTED_BOAR,
            materials: &[(BOAR_MEAT, 1)],
        },
    ]
};

// ---- Mobs ----

/// Every kind of mob. Each zone has its own five: an aggressive beast, a
/// neutral beast, a fighter, a caster and an elite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobKind {
    // Amberfall Vale (human)
    Wolf,
    Boar,
    Bandit,
    BanditMystic,
    Golem,
    // Scorchsand Wastes (orc)
    Scorpion,
    Hyena,
    SandRaider,
    SandShaman,
    SandstoneColossus,
    // Silverbough Glade (elf)
    ShadowfangWolf,
    ThornbackBoar,
    SatyrReaver,
    SatyrTrickster,
    Treant,
    // Grubdeep Caverns (goblin)
    CaveSpider,
    StonehideBoar,
    TroggBrute,
    TroggShaman,
    CrystalGolem,
    // Frostcog Peaks (gnome)
    SnowWolf,
    FrostBoar,
    FrostTroll,
    FrostTrollShaman,
    Yeti,
    // Witherwood (undead)
    GhoulHound,
    PlagueBoar,
    Skeleton,
    Necromancer,
    BoneColossus,
    // The Sunken Vault (dungeon)
    VaultHound,
    VaultCrawler,
    DrownedEnforcer,
    DrownedAdept,
    StoneWarden,
    SunkenKing,
    // Water mobs, in the shallows of zones with lakes.
    MudsnapCrab,
    GlimmershellCrab,
    BogLurker,
}

/// What a mob is built from when drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobModel {
    Wolf,
    Boar,
    Spider,
    Scorpion,
    /// A person, with a style for the details.
    Humanoid(HumanoidStyle),
    /// A huge elite.
    Giant(GiantStyle),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HumanoidStyle {
    Bandit,
    Mystic,
    Raider,
    Shaman,
    Satyr,
    Trickster,
    Trogg,
    TroggShaman,
    Troll,
    TrollShaman,
    Skeleton,
    Necromancer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiantStyle {
    Stone,
    Sandstone,
    Treant,
    Crystal,
    Yeti,
    Bone,
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
    /// A dungeon boss: an elite with its own mechanics, shown as "Boss".
    pub boss: bool,
    /// Calls nearby mobs from its camp for help when attacked.
    pub social: bool,
    /// Rough radius, used for melee reach and drawing.
    pub size: f32,
    /// An ability the mob uses every so many seconds.
    pub spell: Option<(AbilityId, f32)>,
    pub respawn: f32,
    pub loot: LootTable,
    pub model: MobModel,
    /// Main, secondary and accent colors.
    pub colors: [(f32, f32, f32); 3],
}

const BEAST_LOOT: LootTable = LootTable {
    copper_per_level: (1, 4),
    items: &[(items::LIGHT_LEATHER, 0.4, 1, 1)],
};
const HIDE_LOOT: LootTable = LootTable {
    copper_per_level: (1, 3),
    items: &[(items::LIGHT_LEATHER, 0.85, 1, 2)],
};
const BOAR_LOOT: LootTable = LootTable {
    copper_per_level: (1, 3),
    items: &[
        (items::LIGHT_LEATHER, 0.85, 1, 2),
        (items::BOAR_MEAT, 0.5, 1, 1),
    ],
};
const WATER_LOOT: LootTable = LootTable {
    copper_per_level: (1, 4),
    items: &[
        (items::LIGHT_LEATHER, 0.4, 1, 1),
        (items::RAW_FISH, 0.6, 1, 1),
    ],
};
const FIGHTER_LOOT: LootTable = LootTable {
    copper_per_level: (6, 15),
    items: &[
        (items::LINEN_CLOTH, 0.6, 1, 2),
        (items::IRON_SCRAP, 0.35, 1, 2),
    ],
};
const CASTER_LOOT: LootTable = LootTable {
    copper_per_level: (6, 15),
    items: &[(items::LINEN_CLOTH, 0.75, 1, 3)],
};
const BOSS_LOOT: LootTable = LootTable {
    copper_per_level: (40, 80),
    items: &[
        (items::CROWN_OF_THE_SUNKEN_KING, 0.25, 1, 1),
        (items::ANCIENT_CORE, 1.0, 2, 2),
        (items::LINEN_CLOTH, 1.0, 3, 5),
    ],
};
const ELITE_LOOT: LootTable = LootTable {
    copper_per_level: (20, 40),
    items: &[
        (items::ANCIENT_CORE, 1.0, 1, 1),
        (items::LINEN_CLOTH, 1.0, 2, 4),
    ],
};

/// An aggressive beast that hunts alone.
const fn hunter(
    name: &'static str,
    model: MobModel,
    spell: AbilityId,
    colors: [(f32, f32, f32); 3],
) -> MobTemplate {
    MobTemplate {
        name,
        hp: 55.0,
        damage: (4.0, 6.0),
        attack_interval: 2.0,
        speed: 7.5,
        aggressive: true,
        elite: false,
        boss: false,
        social: false,
        size: 0.9,
        spell: Some((spell, 10.0)),
        respawn: 25.0,
        loot: BEAST_LOOT,
        model,
        colors,
    }
}

/// A grazer that also drops boar meat.
const fn boar(name: &'static str, model: MobModel, colors: [(f32, f32, f32); 3]) -> MobTemplate {
    MobTemplate {
        loot: BOAR_LOOT,
        ..grazer(name, model, colors)
    }
}

/// A hunter from the shallows, that drops fish.
const fn water_hunter(
    name: &'static str,
    model: MobModel,
    spell: AbilityId,
    colors: [(f32, f32, f32); 3],
) -> MobTemplate {
    MobTemplate {
        loot: WATER_LOOT,
        ..hunter(name, model, spell, colors)
    }
}

/// A neutral beast that only fights back, and drops plenty of leather.
const fn grazer(name: &'static str, model: MobModel, colors: [(f32, f32, f32); 3]) -> MobTemplate {
    MobTemplate {
        name,
        hp: 65.0,
        damage: (4.0, 7.0),
        attack_interval: 2.2,
        speed: 6.5,
        aggressive: false,
        elite: false,
        boss: false,
        social: false,
        size: 0.9,
        spell: None,
        respawn: 25.0,
        loot: HIDE_LOOT,
        model,
        colors,
    }
}

const fn fighter(
    name: &'static str,
    style: HumanoidStyle,
    colors: [(f32, f32, f32); 3],
) -> MobTemplate {
    MobTemplate {
        name,
        hp: 70.0,
        damage: (6.0, 9.0),
        attack_interval: 2.2,
        speed: 6.5,
        aggressive: true,
        elite: false,
        boss: false,
        social: true,
        size: 0.8,
        spell: None,
        respawn: 30.0,
        loot: FIGHTER_LOOT,
        model: MobModel::Humanoid(style),
        colors,
    }
}

const fn caster(
    name: &'static str,
    style: HumanoidStyle,
    spell: AbilityId,
    colors: [(f32, f32, f32); 3],
) -> MobTemplate {
    MobTemplate {
        name,
        hp: 55.0,
        damage: (3.0, 5.0),
        attack_interval: 2.0,
        speed: 6.5,
        aggressive: true,
        elite: false,
        boss: false,
        social: true,
        size: 0.8,
        spell: Some((spell, 5.0)),
        respawn: 30.0,
        loot: CASTER_LOOT,
        model: MobModel::Humanoid(style),
        colors,
    }
}

const fn elite(name: &'static str, style: GiantStyle, colors: [(f32, f32, f32); 3]) -> MobTemplate {
    MobTemplate {
        name,
        hp: 330.0,
        damage: (12.0, 17.0),
        attack_interval: 3.0,
        speed: 5.5,
        aggressive: true,
        elite: true,
        boss: false,
        social: false,
        size: 2.2,
        spell: Some((ids::GROUND_SLAM, 12.0)),
        respawn: 120.0,
        loot: ELITE_LOOT,
        model: MobModel::Giant(style),
        colors,
    }
}

use HumanoidStyle as H;
use MobModel as Mm;

const WOLF: MobTemplate = hunter(
    "Gray Wolf",
    Mm::Wolf,
    ids::SAVAGE_BITE,
    [(0.5, 0.48, 0.47), (0.33, 0.31, 0.31), (1.0, 0.82, 0.25)],
);
const BOAR: MobTemplate = boar(
    "Wild Boar",
    Mm::Boar,
    [(0.43, 0.29, 0.19), (0.27, 0.18, 0.12), (0.96, 0.93, 0.83)],
);
const BANDIT: MobTemplate = fighter(
    "Bandit Thug",
    H::Bandit,
    [(0.46, 0.32, 0.2), (0.26, 0.23, 0.2), (0.65, 0.12, 0.1)],
);
const BANDIT_MYSTIC: MobTemplate = caster(
    "Bandit Mystic",
    H::Mystic,
    ids::SHADOW_BOLT,
    [(0.32, 0.12, 0.36), (0.26, 0.1, 0.3), (0.75, 0.35, 1.0)],
);
const GOLEM: MobTemplate = elite(
    "Ancient Golem",
    GiantStyle::Stone,
    [(0.5, 0.49, 0.46), (0.42, 0.41, 0.39), (0.4, 0.95, 1.0)],
);

const SCORPION: MobTemplate = hunter(
    "Dune Scorpion",
    Mm::Scorpion,
    ids::VENOM_STING,
    [(0.72, 0.5, 0.25), (0.48, 0.3, 0.14), (0.4, 0.85, 0.2)],
);
const HYENA: MobTemplate = grazer(
    "Dust Hyena",
    Mm::Wolf,
    [(0.74, 0.62, 0.4), (0.45, 0.35, 0.22), (0.95, 0.6, 0.2)],
);
const SAND_RAIDER: MobTemplate = fighter(
    "Sand Raider",
    H::Raider,
    [(0.78, 0.68, 0.48), (0.4, 0.28, 0.18), (0.75, 0.25, 0.12)],
);
const SAND_SHAMAN: MobTemplate = caster(
    "Sand Shaman",
    H::Shaman,
    ids::LIGHTNING_BOLT,
    [(0.62, 0.38, 0.2), (0.35, 0.24, 0.15), (0.4, 0.75, 1.0)],
);
const SANDSTONE_COLOSSUS: MobTemplate = elite(
    "Sandstone Colossus",
    GiantStyle::Sandstone,
    [(0.8, 0.66, 0.44), (0.66, 0.52, 0.34), (1.0, 0.7, 0.25)],
);

const SHADOWFANG: MobTemplate = hunter(
    "Shadowfang Wolf",
    Mm::Wolf,
    ids::SAVAGE_BITE,
    [(0.28, 0.26, 0.36), (0.16, 0.15, 0.22), (0.7, 0.4, 1.0)],
);
const THORNBACK: MobTemplate = boar(
    "Thornback Boar",
    Mm::Boar,
    [(0.35, 0.4, 0.25), (0.22, 0.3, 0.14), (0.9, 0.9, 0.75)],
);
const SATYR_REAVER: MobTemplate = fighter(
    "Satyr Reaver",
    H::Satyr,
    [(0.55, 0.38, 0.28), (0.36, 0.26, 0.2), (0.85, 0.8, 0.7)],
);
const SATYR_TRICKSTER: MobTemplate = caster(
    "Satyr Trickster",
    H::Trickster,
    ids::SHADOW_BOLT,
    [(0.45, 0.3, 0.42), (0.36, 0.26, 0.2), (0.5, 1.0, 0.6)],
);
const TREANT: MobTemplate = elite(
    "Ancient Treant",
    GiantStyle::Treant,
    [(0.4, 0.28, 0.18), (0.3, 0.2, 0.12), (0.4, 0.8, 0.35)],
);

const CAVE_SPIDER: MobTemplate = hunter(
    "Cave Spider",
    Mm::Spider,
    ids::WEB,
    [(0.25, 0.22, 0.28), (0.14, 0.12, 0.16), (0.9, 0.2, 0.2)],
);
const STONEHIDE: MobTemplate = boar(
    "Stonehide Boar",
    Mm::Boar,
    [(0.4, 0.42, 0.46), (0.26, 0.27, 0.3), (0.85, 0.85, 0.9)],
);
const TROGG: MobTemplate = fighter(
    "Trogg Brute",
    H::Trogg,
    [(0.48, 0.5, 0.5), (0.34, 0.3, 0.26), (0.9, 0.75, 0.3)],
);
const TROGG_SHAMAN: MobTemplate = caster(
    "Trogg Shaman",
    H::TroggShaman,
    ids::LIGHTNING_BOLT,
    [(0.46, 0.48, 0.5), (0.3, 0.24, 0.3), (0.5, 0.8, 1.0)],
);
const CRYSTAL_GOLEM: MobTemplate = elite(
    "Crystal Golem",
    GiantStyle::Crystal,
    [(0.35, 0.4, 0.5), (0.25, 0.28, 0.36), (0.6, 0.4, 1.0)],
);

const SNOW_WOLF: MobTemplate = hunter(
    "Snow Wolf",
    Mm::Wolf,
    ids::SAVAGE_BITE,
    [(0.9, 0.92, 0.95), (0.7, 0.74, 0.8), (0.4, 0.75, 1.0)],
);
const FROST_BOAR: MobTemplate = boar(
    "Frost Boar",
    Mm::Boar,
    [(0.55, 0.6, 0.68), (0.4, 0.44, 0.52), (0.95, 0.97, 1.0)],
);
const FROST_TROLL: MobTemplate = fighter(
    "Frost Troll",
    H::Troll,
    [(0.5, 0.65, 0.78), (0.35, 0.3, 0.32), (0.95, 0.95, 0.9)],
);
const FROST_TROLL_SHAMAN: MobTemplate = caster(
    "Frost Troll Shaman",
    H::TrollShaman,
    ids::FROSTBOLT,
    [(0.48, 0.62, 0.76), (0.25, 0.3, 0.45), (0.5, 0.85, 1.0)],
);
const YETI: MobTemplate = elite(
    "Yeti",
    GiantStyle::Yeti,
    [(0.92, 0.93, 0.95), (0.6, 0.62, 0.68), (0.4, 0.75, 1.0)],
);

const GHOUL_HOUND: MobTemplate = hunter(
    "Ghoul Hound",
    Mm::Wolf,
    ids::SAVAGE_BITE,
    [(0.42, 0.46, 0.36), (0.3, 0.26, 0.24), (0.6, 1.0, 0.3)],
);
const PLAGUE_BOAR: MobTemplate = boar(
    "Plague Boar",
    Mm::Boar,
    [(0.45, 0.42, 0.32), (0.3, 0.33, 0.2), (0.75, 0.85, 0.5)],
);
const SKELETON: MobTemplate = fighter(
    "Skeleton Warrior",
    H::Skeleton,
    [(0.88, 0.85, 0.76), (0.35, 0.3, 0.28), (0.5, 1.0, 0.5)],
);
const NECROMANCER: MobTemplate = caster(
    "Necromancer",
    H::Necromancer,
    ids::SHADOW_BOLT,
    [(0.18, 0.15, 0.2), (0.12, 0.1, 0.14), (0.5, 1.0, 0.4)],
);
const BONE_COLOSSUS: MobTemplate = elite(
    "Bone Colossus",
    GiantStyle::Bone,
    [(0.86, 0.83, 0.74), (0.6, 0.56, 0.5), (0.5, 1.0, 0.4)],
);

// The Sunken Vault's mobs are tougher than their kin outside: they're
// meant for a party.

const VAULT_HOUND: MobTemplate = MobTemplate {
    hp: 120.0,
    damage: (6.0, 9.0),
    respawn: 0.0,
    ..hunter(
        "Vault Hound",
        Mm::Wolf,
        ids::SAVAGE_BITE,
        [(0.3, 0.36, 0.38), (0.18, 0.22, 0.24), (0.35, 0.95, 0.85)],
    )
};
const VAULT_CRAWLER: MobTemplate = MobTemplate {
    hp: 110.0,
    damage: (6.0, 9.0),
    social: true,
    respawn: 0.0,
    ..hunter(
        "Vault Crawler",
        Mm::Spider,
        ids::WEB,
        [(0.2, 0.28, 0.27), (0.1, 0.15, 0.15), (0.3, 1.0, 0.75)],
    )
};
const DROWNED_ENFORCER: MobTemplate = MobTemplate {
    hp: 150.0,
    damage: (8.0, 12.0),
    respawn: 0.0,
    ..fighter(
        "Drowned Enforcer",
        H::Raider,
        [(0.25, 0.35, 0.4), (0.16, 0.2, 0.22), (0.2, 0.8, 0.75)],
    )
};
const DROWNED_ADEPT: MobTemplate = MobTemplate {
    hp: 120.0,
    damage: (4.0, 7.0),
    respawn: 0.0,
    ..caster(
        "Drowned Adept",
        H::Mystic,
        ids::SHADOW_BOLT,
        [(0.12, 0.25, 0.3), (0.08, 0.15, 0.18), (0.3, 0.95, 0.9)],
    )
};
const STONE_WARDEN: MobTemplate = MobTemplate {
    hp: 520.0,
    respawn: 0.0,
    ..elite(
        "Stone Warden",
        GiantStyle::Stone,
        [(0.36, 0.42, 0.38), (0.26, 0.3, 0.28), (0.3, 1.0, 0.8)],
    )
};
const SUNKEN_KING: MobTemplate = MobTemplate {
    hp: 800.0,
    damage: (16.0, 22.0),
    size: 2.6,
    respawn: 0.0,
    loot: BOSS_LOOT,
    boss: true,
    // The boss's own mechanics (server/src/world/boss.rs) replace the
    // elite's Ground Slam.
    spell: None,
    ..elite(
        "Morvane the Sunken King",
        GiantStyle::Bone,
        [(0.55, 0.62, 0.6), (0.3, 0.36, 0.36), (0.25, 1.0, 0.85)],
    )
};

// Water mobs hunt like the zone's aggressive beasts, from the shallows.
const MUDSNAP_CRAB: MobTemplate = water_hunter(
    "Mudsnap Crab",
    Mm::Scorpion,
    ids::SAVAGE_BITE,
    [(0.45, 0.33, 0.2), (0.3, 0.24, 0.14), (0.55, 0.6, 0.3)],
);
const GLIMMERSHELL_CRAB: MobTemplate = water_hunter(
    "Glimmershell Crab",
    Mm::Scorpion,
    ids::SAVAGE_BITE,
    [(0.3, 0.62, 0.62), (0.7, 0.75, 0.8), (0.85, 0.95, 1.0)],
);
const BOG_LURKER: MobTemplate = water_hunter(
    "Bog Lurker",
    Mm::Spider,
    ids::VENOM_STING,
    [(0.3, 0.38, 0.2), (0.12, 0.14, 0.1), (0.6, 0.85, 0.25)],
);

impl MobKind {
    pub fn template(self) -> &'static MobTemplate {
        use MobKind::*;
        match self {
            Wolf => &WOLF,
            Boar => &BOAR,
            Bandit => &BANDIT,
            BanditMystic => &BANDIT_MYSTIC,
            Golem => &GOLEM,
            Scorpion => &SCORPION,
            Hyena => &HYENA,
            SandRaider => &SAND_RAIDER,
            SandShaman => &SAND_SHAMAN,
            SandstoneColossus => &SANDSTONE_COLOSSUS,
            ShadowfangWolf => &SHADOWFANG,
            ThornbackBoar => &THORNBACK,
            SatyrReaver => &SATYR_REAVER,
            SatyrTrickster => &SATYR_TRICKSTER,
            Treant => &TREANT,
            CaveSpider => &CAVE_SPIDER,
            StonehideBoar => &STONEHIDE,
            TroggBrute => &TROGG,
            TroggShaman => &TROGG_SHAMAN,
            CrystalGolem => &CRYSTAL_GOLEM,
            SnowWolf => &SNOW_WOLF,
            FrostBoar => &FROST_BOAR,
            FrostTroll => &FROST_TROLL,
            FrostTrollShaman => &FROST_TROLL_SHAMAN,
            Yeti => &YETI,
            GhoulHound => &GHOUL_HOUND,
            PlagueBoar => &PLAGUE_BOAR,
            Skeleton => &SKELETON,
            Necromancer => &NECROMANCER,
            BoneColossus => &BONE_COLOSSUS,
            VaultHound => &VAULT_HOUND,
            VaultCrawler => &VAULT_CRAWLER,
            DrownedEnforcer => &DROWNED_ENFORCER,
            DrownedAdept => &DROWNED_ADEPT,
            StoneWarden => &STONE_WARDEN,
            SunkenKing => &SUNKEN_KING,
            MudsnapCrab => &MUDSNAP_CRAB,
            GlimmershellCrab => &GLIMMERSHELL_CRAB,
            BogLurker => &BOG_LURKER,
        }
    }

    pub fn max_hp(self, level: u8) -> f32 {
        self.template().hp * (1.0 + 0.3 * (level.max(1) - 1) as f32)
    }

    /// The Sunken Vault's mobs, bosses last.
    pub const DUNGEON: [MobKind; 6] = [
        MobKind::VaultHound,
        MobKind::VaultCrawler,
        MobKind::DrownedEnforcer,
        MobKind::DrownedAdept,
        MobKind::StoneWarden,
        MobKind::SunkenKing,
    ];

    /// Chances of a green armor piece (`RARE_DROPS`) and a green weapon
    /// (`WEAPON_DROPS`) on its corpse.
    pub fn drop_chances(self) -> (f32, f32) {
        if self == MobKind::SunkenKing {
            (SUNKEN_KING_RARE_DROP_CHANCE, SUNKEN_KING_WEAPON_DROP_CHANCE)
        } else if self.template().elite {
            (ELITE_RARE_DROP_CHANCE, ELITE_WEAPON_DROP_CHANCE)
        } else {
            (RARE_DROP_CHANCE, WEAPON_DROP_CHANCE)
        }
    }

    /// The mob that lives in a zone's lakes, if it has any worth a camp.
    pub fn water(zone: Zone) -> Option<MobKind> {
        match zone {
            Zone::Amberfall => Some(MobKind::MudsnapCrab),
            Zone::Silverbough => Some(MobKind::GlimmershellCrab),
            Zone::Witherwood => Some(MobKind::BogLurker),
            // No lakes, or too small for a camp.
            Zone::Scorchsand | Zone::Grubdeep | Zone::Frostcog => None,
        }
    }

    /// A zone's five mobs: aggressive beast, neutral beast, fighter, caster
    /// and elite.
    pub fn for_zone(zone: Zone) -> [MobKind; 5] {
        use MobKind::*;
        match zone {
            Zone::Amberfall => [Wolf, Boar, Bandit, BanditMystic, Golem],
            Zone::Scorchsand => [Scorpion, Hyena, SandRaider, SandShaman, SandstoneColossus],
            Zone::Silverbough => [
                ShadowfangWolf,
                ThornbackBoar,
                SatyrReaver,
                SatyrTrickster,
                Treant,
            ],
            Zone::Grubdeep => [
                CaveSpider,
                StonehideBoar,
                TroggBrute,
                TroggShaman,
                CrystalGolem,
            ],
            Zone::Frostcog => [SnowWolf, FrostBoar, FrostTroll, FrostTrollShaman, Yeti],
            Zone::Witherwood => [GhoulHound, PlagueBoar, Skeleton, Necromancer, BoneColossus],
        }
    }
}

// ---- Appearance ----

/// How a character looks, chosen at character creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appearance {
    /// Characters made before races existed are human.
    #[serde(default)]
    pub race: Race,
    /// 0: broad build, 1: slender build.
    pub body: u8,
    pub skin: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    /// Height slider, 0 (shortest) to `SLIDER_MAX` (tallest). Characters
    /// made before the slider existed are in the middle.
    #[serde(default = "slider_middle")]
    pub height: u8,
    /// Weight (plumpness) slider, 0 (thinnest) to `SLIDER_MAX` (heaviest).
    #[serde(default = "slider_middle")]
    pub weight: u8,
}

fn slider_middle() -> u8 {
    Appearance::SLIDER_MAX / 2
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            race: Race::default(),
            body: 0,
            skin: 0,
            hair_style: 0,
            hair_color: 0,
            height: slider_middle(),
            weight: slider_middle(),
        }
    }
}

impl Appearance {
    pub const BODIES: u8 = 2;
    /// The top of the height and weight sliders.
    pub const SLIDER_MAX: u8 = 100;
    /// How far the height slider stretches the race's usual height, each way.
    pub const HEIGHT_RANGE: f32 = 0.1;
    /// How far the weight slider widens or narrows the body, each way.
    pub const WEIGHT_RANGE: f32 = 0.2;
    pub const SKINS: u8 = 5;
    pub const HAIR_STYLES: u8 = 5;
    pub const HAIR_COLORS: u8 = 6;

    pub fn hair_style_name(self) -> &'static str {
        ["Bald", "Short", "Long", "Ponytail", "Mohawk"][self.hair_style as usize % 5]
    }

    /// Keeps every field in range (for data from the network or a save file).
    pub fn clamped(self) -> Self {
        Self {
            race: self.race,
            body: self.body % Self::BODIES,
            skin: self.skin % Self::SKINS,
            hair_style: self.hair_style % Self::HAIR_STYLES,
            hair_color: self.hair_color % Self::HAIR_COLORS,
            height: self.height.min(Self::SLIDER_MAX),
            weight: self.weight.min(Self::SLIDER_MAX),
        }
    }

    /// The height slider as a multiplier on the race's usual height: 1.0 in
    /// the middle, `1 - HEIGHT_RANGE` to `1 + HEIGHT_RANGE` at the ends.
    pub fn height_scale(self) -> f32 {
        1.0 + Self::slider(self.height) * Self::HEIGHT_RANGE
    }

    /// The weight slider as a multiplier on body width: 1.0 in the middle,
    /// `1 - WEIGHT_RANGE` to `1 + WEIGHT_RANGE` at the ends.
    pub fn weight_scale(self) -> f32 {
        1.0 + Self::slider(self.weight) * Self::WEIGHT_RANGE
    }

    /// A slider value from -1 (bottom) to 1 (top).
    fn slider(value: u8) -> f32 {
        let max = Self::SLIDER_MAX as f32;
        (value.min(Self::SLIDER_MAX) as f32 - max / 2.0) / (max / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_appearance_loads_with_middle_sliders() {
        let old = r#"{"race":"Orc","body":1,"skin":2,"hair_style":3,"hair_color":4}"#;
        let a: Appearance = serde_json::from_str(old).unwrap();
        assert_eq!(a.race, Race::Orc);
        assert_eq!((a.height, a.weight), (50, 50));
        assert_eq!(a.height_scale(), 1.0);
        assert_eq!(a.weight_scale(), 1.0);
        assert_eq!(Appearance::default().height, 50);
    }

    #[test]
    fn sliders_clamp_and_scale() {
        let a = Appearance {
            height: 250,
            weight: 0,
            ..Default::default()
        };
        assert_eq!(a.clamped().height, Appearance::SLIDER_MAX);
        assert!((a.height_scale() - (1.0 + Appearance::HEIGHT_RANGE)).abs() < 1e-6);
        assert!((a.weight_scale() - (1.0 - Appearance::WEIGHT_RANGE)).abs() < 1e-6);
    }

    #[test]
    fn ability_ids_match_table() {
        assert_eq!(ABILITIES.len(), 98);
        for class in Class::ALL {
            for id in class.abilities() {
                assert!((id.0 as usize) < ABILITIES.len());
            }
        }
        assert_eq!(ability(ids::GROUND_SLAM).name, "Ground Slam");
        assert_eq!(ability(ids::SMITE).name, "Smite");
        assert_eq!(ability(ids::FIREBALL).name, "Fireball");
        assert_eq!(ability(ids::KICK).name, "Kick");
        assert_eq!(ability(ids::CHARGE).name, "Charge");
        assert_eq!(ability(ids::DISSONANT_WHISPERS).name, "Dissonant Whispers");
        assert_eq!(ability(ids::LIGHTNING_BOLT).name, "Lightning Bolt");
        assert_eq!(item(items::MANA_POTION).name, "Mana Potion");
    }

    #[test]
    fn every_class_has_seven_distinct_abilities_and_starts_with_one() {
        assert_eq!(Class::ALL.len(), 13);
        for class in Class::ALL {
            let a = class.abilities();
            for (i, x) in a.iter().enumerate() {
                assert!(
                    !a[i + 1..].contains(x),
                    "{class:?} repeats {}",
                    ability(*x).name
                );
            }
            assert_eq!(class.known(1).count(), 1, "{class:?}");
            assert_eq!(class.known(MAX_LEVEL).count(), ACTION_BAR_SLOTS);
            assert_eq!(class.unlock_level(class.abilities()[0]), Some(1));
            // The first ability always does damage, so everyone can fight.
            let first = ability(a[0]);
            assert!(first.targeting.needs_enemy(), "{class:?}");
            assert!(
                first
                    .effects
                    .iter()
                    .any(|e| matches!(e, Effect::Damage { .. })),
                "{class:?}"
            );
        }
        assert_eq!(Class::Barbarian.abilities()[E_SLOT], ids::CHARGE);
        assert_eq!(Class::Mage.unlock_level(ids::SMITE), None);
    }

    #[test]
    fn old_saves_still_load() {
        let class: Class = serde_json::from_str("\"Warrior\"").unwrap();
        assert_eq!(class, Class::Barbarian);
        let a: Appearance =
            serde_json::from_str(r#"{"body":1,"skin":2,"hair_style":3,"hair_color":4}"#).unwrap();
        assert_eq!(a.race, Race::Human);
        assert_eq!(a.hair_color, 4);
    }

    #[test]
    fn every_zone_has_its_own_mobs() {
        let mut seen = std::collections::HashSet::new();
        for zone in Zone::ALL {
            let [hunter, grazer, fighter, caster, elite] = MobKind::for_zone(zone);
            assert!(hunter.template().aggressive && !grazer.template().aggressive);
            assert!(fighter.template().social && caster.template().spell.is_some());
            assert!(elite.template().elite);
            for k in MobKind::for_zone(zone)
                .into_iter()
                .chain(MobKind::water(zone))
            {
                assert!(seen.insert(k), "{k:?} is in two zones");
            }
            if let Some(w) = MobKind::water(zone) {
                assert!(w.template().aggressive && !w.template().elite);
            }
        }
    }

    #[test]
    fn xp_curve() {
        assert_eq!(xp_to_next(1), 100);
        assert_eq!(xp_to_next(MAX_LEVEL), 0);
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
    fn recipes_make_gear_from_drops() {
        for r in &RECIPES {
            let made = item(r.result).kind;
            match r.skill {
                Skill::Crafting => assert!(matches!(
                    made,
                    ItemKind::Armor { .. } | ItemKind::Weapon { .. }
                )),
                Skill::Cooking => assert!(matches!(made, ItemKind::Food { .. })),
            }
            for (m, n) in r.materials {
                assert_eq!(item(*m).kind, ItemKind::Material);
                assert!(*n <= item(*m).max_stack);
            }
        }
    }

    #[test]
    fn merchants_buy_for_less_than_they_sell() {
        for it in &ITEMS {
            assert!(it.sell_price() < it.price || it.price <= 1);
        }
    }

    #[test]
    fn greens_sit_between_crafted_commons_and_blues() {
        let score = |id: ItemId| match item(id).kind {
            ItemKind::Armor {
                slot,
                armor,
                stamina,
                power,
            } => (slot, armor + stamina * 2.0 + power * 3.0),
            _ => panic!("not armor"),
        };
        let blue = score(items::HEARTSTONE_CHESTGUARD).1;
        for green in RARE_DROPS {
            assert_eq!(item(green).quality, Quality::Uncommon);
            let (slot, s) = score(green);
            assert!(s < blue, "{} beats the Heartstone", item(green).name);
            // Better than every crafted common piece for the same slot.
            for r in &RECIPES {
                let it = item(r.result);
                if it.quality == Quality::Common
                    && matches!(it.kind, ItemKind::Armor { .. })
                    && let (rs, rscore) = score(r.result)
                    && rs == slot
                {
                    assert!(
                        s > rscore,
                        "{} isn't better than {}",
                        item(green).name,
                        it.name
                    );
                }
            }
        }
    }

    #[test]
    fn green_weapons_sit_between_crafted_and_heartstone() {
        let score = |id: ItemId| match item(id).kind {
            ItemKind::Weapon {
                damage,
                stamina,
                power,
            } => damage * 3.0 + stamina * 2.0 + power * 3.0,
            _ => panic!("{} is not a weapon", item(id).name),
        };
        let blue = score(items::HEARTSTONE_GREATSWORD);
        for green in WEAPON_DROPS {
            assert_eq!(item(green).quality, Quality::Uncommon);
            assert!(score(green) < blue);
            for r in &RECIPES {
                let it = item(r.result);
                if it.quality == Quality::Common && matches!(it.kind, ItemKind::Weapon { .. }) {
                    assert!(score(green) > score(r.result), "{}", it.name);
                }
            }
        }
    }

    #[test]
    fn item_ids_match_their_names() {
        use items::*;
        for (id, name) in [
            (IRON_SCRAP, "Iron Scrap"),
            (LINEN_SANDALS, "Linen Sandals"),
            (IRON_SWORD, "Iron Sword"),
            (HEARTSTONE_GREATSWORD, "Heartstone Greatsword"),
            (MOONWHISPER_STAFF, "Moonwhisper Staff"),
            (CROWN_OF_THE_SUNKEN_KING, "Crown of the Sunken King"),
        ] {
            assert_eq!(item(id).name, name);
        }
    }
}
