//! Messages between the client and the server.
//!
//! A session goes: `Hello` (account name and password) -> character list ->
//! create, delete or `EnterWorld` -> playing -> `Logout` (back to the character list).
//!
//! The server is authoritative for everything except the player's own
//! movement: clients move their character locally and report the result,
//! and the server checks it against the run speed.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::data::{AbilityId, Appearance, Class, Hotbar, ItemId, MobKind, Race, Slot};
use crate::quests::{QuestId, QuestLog};
use crate::talents::Ranks;
use crate::world::Zone;

/// Bump whenever a message changes shape.
pub const PROTOCOL_VERSION: u32 = 17;
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
        /// Checked against the account's password. An account with no
        /// password yet (a new one) takes this one. Solo and sandbox
        /// servers ignore it.
        password: String,
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
    /// Rearranged hotbars, saved with the character.
    SetHotbar(Hotbar),
    Chat(String),
    /// Come back to life at the graveyard.
    ReleaseSpirit,
    /// Take everything from a corpse.
    Loot(EntityId),
    /// Wear the item in this bag slot.
    Equip(usize),
    /// Take off a piece of armor and put it in your bags.
    Unequip(Slot),
    /// Put your weapon away in your bags.
    UnequipWeapon,
    /// Make the recipe with this index in `RECIPES`.
    Craft(usize),
    /// Buy one of an item from a merchant.
    Buy {
        merchant: EntityId,
        item: ItemId,
    },
    /// Sell everything in a bag slot to a merchant.
    Sell {
        merchant: EntityId,
        slot: usize,
    },
    /// Use the item in a bag slot (drink a potion).
    UseItem(usize),
    /// Put a point into the talent with this index (see `talents`).
    LearnTalent(usize),
    /// Take back every talent point.
    ResetTalents,
    /// A sandbox server's cheats. Refused on normal servers.
    Sandbox(SandboxCmd),
    /// Take a quest from a quest giver.
    AcceptQuest {
        giver: EntityId,
        quest: QuestId,
    },
    /// Hand in a finished quest to its quest giver.
    TurnInQuest {
        giver: EntityId,
        quest: QuestId,
    },
    /// Give up a quest (its progress is lost).
    AbandonQuest(QuestId),
    /// Ask a player (by name) to join your party.
    PartyInvite(String),
    /// Join the party you were invited to.
    PartyAccept,
    /// Turn down a party invite.
    PartyDecline,
    PartyLeave,
    /// Remove a member (by name). Leader only.
    PartyKick(String),
    /// Make another member (by name) the leader. Leader only.
    PartyPromote(String),
    /// Use the waystone you're standing at.
    Travel(Destination),
    /// Challenge a player (by name) to a duel.
    DuelRequest(String),
    /// Take up the duel you were challenged to.
    DuelAccept,
    /// Turn down a duel challenge.
    DuelDecline,
}

/// Where a waystone can take you.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Destination {
    /// Another starting area's town (or back out of the vault).
    Town(Zone),
    /// Your party's copy of a dungeon.
    Dungeon(crate::dungeon::DungeonId),
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
    Snapshot(Box<Snapshot>),
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
    /// Held weapon.
    pub weapon: Option<ItemId>,
}

/// What you can do in sandbox mode.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SandboxCmd {
    SetLevel(u8),
    AddMoney(u32),
    GiveItem(ItemId),
    /// Go to a starting area's town.
    Teleport(Zone),
    /// Take no damage and pay nothing for abilities.
    ToggleGod,
    /// A mob of your choice appears in front of you.
    SpawnMob {
        kind: MobKind,
        level: u8,
    },
    /// Removes every mob you spawned.
    ClearSpawns,
    /// Cooldowns, health and power back to full.
    Refresh,
}

/// The part of the world a player can see, sent every server tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub entities: Vec<EntityView>,
    pub me: SelfView,
    /// Marked ground about to be hit nearby: get out of the circle!
    pub hazards: Vec<HazardView>,
    /// The teleporter out of the vault, once its boss is dead.
    pub portal: Option<Vec3>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HazardView {
    pub pos: Vec3,
    pub radius: f32,
    /// Seconds until it lands, and how long the warning lasts in all.
    pub remaining: f32,
    pub total: f32,
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
    /// Held weapon (players only).
    pub weapon: Option<ItemId>,
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
    /// A friendly townsperson who trades.
    Merchant(Race),
    /// A friendly townsperson who hands out quests.
    QuestGiver(Race),
}

impl EntityKind {
    /// Townspeople: merchants and quest givers.
    pub fn is_npc(self) -> bool {
        matches!(self, EntityKind::Merchant(_) | EntityKind::QuestGiver(_))
    }

    pub fn is_player(self) -> bool {
        matches!(self, EntityKind::Player(_))
    }

    pub fn is_mob(self) -> bool {
        matches!(self, EntityKind::Mob { .. })
    }

    /// Players fight mobs; nobody else fights anyone.
    pub fn hostile_to(self, other: EntityKind) -> bool {
        (self.is_player() && other.is_mob()) || (self.is_mob() && other.is_player())
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
    /// Seconds until potions can be used again.
    pub potion_cooldown: f32,
    /// In copper.
    pub money: u32,
    pub bags: Vec<Option<Stack>>,
    pub stats: Stats,
    /// Ranks in each of your class's talents.
    pub talents: Ranks,
    /// The server is in sandbox mode.
    pub sandbox: bool,
    /// Sandbox god mode is on.
    pub god: bool,
    pub quests: QuestLog,
    pub party: Option<PartyView>,
    /// Who has invited you to a party, if anyone.
    pub invite: Option<String>,
    /// What's on your two hotbars.
    pub hotbar: Hotbar,
    /// Who has challenged you to a duel, if anyone.
    pub duel_invite: Option<String>,
    /// The duel you're in, if any.
    pub duel: Option<DuelView>,
}

/// A duel between two players. Nobody dies: it ends when one of them is
/// nearly beaten, leaves the area or logs out.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DuelView {
    pub opponent: EntityId,
    /// Seconds until the fighting starts; zero or below once it has.
    pub countdown: f32,
    /// Where the duel started; the area is `DUEL_AREA` around it.
    pub flag: Vec3,
    /// Seconds you've been outside the area (you forfeit at `DUEL_LEAVE_TIME`).
    pub away: f32,
}

/// The party you're in, yourself included.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartyView {
    pub leader: EntityId,
    pub members: Vec<PartyMember>,
}

/// One party member, wherever they are in the world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartyMember {
    pub id: EntityId,
    pub name: String,
    pub class: Class,
    pub level: u8,
    pub hp: f32,
    pub max_hp: f32,
    pub power: f32,
    pub max_power: f32,
    pub dead: bool,
    /// Close enough to you to share kills (within `PARTY_RANGE`).
    pub near: bool,
}

/// Totals from worn armor and the held weapon.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub armor: f32,
    pub stamina: f32,
    pub power: f32,
    /// Added to every auto attack.
    pub damage: f32,
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
    /// You bought an item for this much.
    Bought {
        item: ItemId,
        price: u32,
    },
    /// You sold items for this much.
    Sold {
        item: ItemId,
        count: u16,
        money: u32,
    },
    /// You took a quest.
    QuestAccepted(QuestId),
    /// You killed something a quest wants.
    QuestProgress {
        quest: QuestId,
        progress: u16,
    },
    /// You handed in a quest and got its rewards.
    QuestComplete {
        quest: QuestId,
        xp: u32,
        money: u32,
        reward: Option<ItemId>,
    },
    /// Something you tried didn't work ("Out of range.").
    Error(String),
    Chat {
        from: String,
        text: String,
    },
    /// A message to your party only.
    PartyChat {
        from: String,
        text: String,
    },
    System(String),
}
