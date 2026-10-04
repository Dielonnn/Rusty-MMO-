//! Messages between the client and the server.
//!
//! A session goes: `Hello` (account name) -> character list -> create, delete
//! or `EnterWorld` -> playing -> `Logout` (back to the character list).
//!
//! The server is authoritative for everything except the player's own
//! movement: clients move their character locally and report the result,
//! and the server checks it against the run speed.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::data::{AbilityId, Appearance, Class, ItemId, MobKind, Slot};

/// Bump whenever a message changes shape.
pub const PROTOCOL_VERSION: u32 = 2;
pub const DEFAULT_PORT: u16 = 7878;

pub type EntityId = u32;

/// One stack of items in a bag slot.
pub type Stack = (ItemId, u16);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    /// The first message on a connection.
    Hello {
        version: u32,
        account: String,
    },
    CreateCharacter {
        name: String,
        class: Class,
        appearance: Appearance,
    },
    DeleteCharacter(String),
    EnterWorld(String),
    /// Leave the world and go back to the character list.
    Logout,
    Move {
        pos: Vec3,
        yaw: f32,
        moving: bool,
    },
    SetTarget(Option<EntityId>),
    UseAbility {
        ability: AbilityId,
        target: Option<EntityId>,
    },
    StartAttack,
    StopAttack,
    Chat(String),
    /// Come back to life at the graveyard.
    ReleaseSpirit,
    /// Take everything from a corpse.
    Loot(EntityId),
    /// Wear the item in this bag slot.
    Equip(usize),
    /// Take off a piece of armor and put it in your bags.
    Unequip(Slot),
    /// Make the recipe with this index in `RECIPES`.
    Craft(usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Your characters on this server. Sent after `Hello`, after logging
    /// out, and after creating or deleting a character.
    Characters(Vec<CharacterSummary>),
    /// Creating, deleting or entering with a character didn't work.
    CharacterError(String),
    Welcome {
        id: EntityId,
    },
    /// The server refused the connection.
    Rejected(String),
    Snapshot(Snapshot),
    Event(GameEvent),
    /// The server moved you (respawn) or refused a move.
    SetPosition {
        pos: Vec3,
        yaw: f32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub name: String,
    pub class: Class,
    pub level: u8,
    pub appearance: Appearance,
    pub gear: [Option<ItemId>; 5],
}

/// The part of the world a player can see, sent every server tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub entities: Vec<EntityView>,
    pub me: SelfView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityView {
    pub id: EntityId,
    pub name: String,
    pub kind: EntityKind,
    pub level: u8,
    pub pos: Vec3,
    pub yaw: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub power: f32,
    pub max_power: f32,
    pub target: Option<EntityId>,
    pub cast: Option<CastView>,
    pub auras: Vec<AuraView>,
    pub dead: bool,
    pub in_combat: bool,
    pub moving: bool,
    /// Players only.
    pub appearance: Appearance,
    /// Worn armor, by `Slot` index (players only).
    pub gear: [Option<ItemId>; 5],
    /// A corpse with loot you're allowed to take.
    pub lootable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EntityKind {
    Player(Class),
    Mob {
        kind: MobKind,
        /// Attacks on sight (red name) rather than only fighting back (yellow).
        aggressive: bool,
        elite: bool,
        /// Running home after giving up a chase; immune to everything.
        evading: bool,
    },
}

impl EntityKind {
    pub fn is_player(self) -> bool {
        matches!(self, EntityKind::Player(_))
    }

    /// Players fight mobs; nobody fights their own side.
    pub fn hostile_to(self, other: EntityKind) -> bool {
        self.is_player() != other.is_player()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CastView {
    pub ability: AbilityId,
    pub elapsed: f32,
    pub total: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AuraView {
    pub ability: AbilityId,
    pub remaining: f32,
    pub duration: f32,
    pub harmful: bool,
    pub source: EntityId,
    pub stacks: u8,
}

/// Things only the player themselves needs to know.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelfView {
    pub xp: u32,
    pub xp_next: u32,
    /// Remaining and total cooldown of each ability that is cooling down.
    pub cooldowns: Vec<(AbilityId, f32, f32)>,
    pub gcd: f32,
    pub auto_attacking: bool,
    pub combo_points: u8,
    /// In copper.
    pub money: u32,
    pub bags: Vec<Option<Stack>>,
    pub stats: Stats,
}

/// Totals from worn armor.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub armor: f32,
    pub stamina: f32,
    pub power: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GameEvent {
    Damage {
        source: EntityId,
        target: EntityId,
        amount: u32,
        absorbed: u32,
        crit: bool,
        /// `None` for auto attacks.
        ability: Option<AbilityId>,
    },
    Heal {
        source: EntityId,
        target: EntityId,
        amount: u32,
        crit: bool,
        ability: AbilityId,
    },
    /// An ability went off (used for projectiles and other visuals).
    AbilityUsed {
        caster: EntityId,
        target: Option<EntityId>,
        ability: AbilityId,
    },
    Interrupted {
        target: EntityId,
        ability: AbilityId,
    },
    /// The target was immune (an evading mob).
    Evade {
        target: EntityId,
    },
    Died {
        id: EntityId,
        killer: Option<EntityId>,
    },
    LevelUp {
        id: EntityId,
        level: u8,
    },
    /// You learned a new ability.
    Learned(AbilityId),
    /// You earned experience.
    Xp {
        amount: u32,
        from: String,
    },
    /// You picked up money and items.
    Looted {
        money: u32,
        items: Vec<Stack>,
    },
    /// You made an item.
    Crafted(ItemId),
    /// Something you tried didn't work ("Out of range.").
    Error(String),
    Chat {
        from: String,
        text: String,
    },
    System(String),
}
