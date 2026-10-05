//! The authoritative game world: players, mobs, combat, loot and AI.
//!
//! The network layer feeds `handle` with client messages, calls `tick` at a
//! fixed rate, sends each player `snapshot_for`, and delivers whatever
//! `drain_outbox` returns.

use std::collections::{BTreeMap, HashMap, HashSet};

use glam::{Vec2, Vec3, vec2};
use shared::data::*;
use shared::protocol::*;
use shared::talents::{self, Bonuses, Ranks};
use shared::world::*;

use crate::character::{Character, add_item, count_item, gear_stats, remove_item};
use shared::props::{ARRIVAL_SPOT, MERCHANT_SPOT, QUEST_SPOT};
use shared::quests::{self, Goal, QuestId, QuestLog};

mod boss;
mod ground;
mod instance;
mod party;

use boss::{BossTimers, Hazard};
use ground::GroundItem;
pub use instance::{Instance, Owner};
pub use party::{Party, PartyId};
use shared::emote::Emote;

/// Who a message is for.
#[derive(Clone, Copy, Debug)]
pub enum Audience {
    Everyone,
    /// Players within view distance of this point.
    Near(Vec3),
    Only(EntityId),
}

/// A small deterministic random number generator (xorshift).
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.f32()
    }

    /// A whole number in `min..=max`.
    fn int(&mut self, min: u32, max: u32) -> u32 {
        min + (self.f32() * (max - min + 1) as f32) as u32
    }

    fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
}

pub struct Aura {
    pub ability: AbilityId,
    pub kind: AuraKind,
    pub duration: f32,
    pub remaining: f32,
    tick_timer: f32,
    pub source: EntityId,
    pub stacks: u8,
}

pub struct Cast {
    pub ability: AbilityId,
    pub target: Option<EntityId>,
    pub elapsed: f32,
    pub total: f32,
}

pub struct PlayerData {
    pub account: String,
    pub class: Class,
    pub appearance: Appearance,
    pub xp: u32,
    pub money: u32,
    pub bags: Vec<Option<Stack>>,
    pub gear: [Option<ItemId>; 5],
    pub weapon: Option<ItemId>,
    pub stats: Stats,
    pub combo_points: u8,
    pub auto_attack: bool,
    /// How far the player may still move; refills at run speed.
    move_budget: f32,
    /// Seconds since mana was last spent.
    since_spend: f32,
    potion_cooldown: f32,
    pub talents: Ranks,
    /// What the talents add up to.
    pub bonuses: Bonuses,
    /// Rearranged hotbars; `None` until the player first moves something.
    pub hotbar: Option<Hotbar>,
    /// Sandbox god mode: no damage taken, abilities are free.
    pub god: bool,
    pub quests: QuestLog,
    pub party: Option<PartyId>,
    /// A party invite: who from, and seconds left to answer it.
    pub invite: Option<(EntityId, f32)>,
    /// The town they entered the Sunken Vault from, and go back to.
    pub home_town: Zone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobState {
    Idle,
    Combat,
    /// Running home after a chase went too far.
    Evading,
}

/// What's left on a corpse, and who may take it.
pub struct Loot {
    pub money: u32,
    pub items: Vec<Stack>,
    pub looters: Vec<EntityId>,
    /// A party member whose turn it is: only they may loot until the
    /// seconds run out.
    pub turn: Option<(EntityId, f32)>,
    /// Gear rolled for each player on their own (in the dungeon): only they
    /// see and take it.
    pub personal: Vec<(EntityId, Vec<Stack>)>,
}

impl Loot {
    /// Whether this player may take the shared money and items.
    fn shares_with(&self, id: EntityId) -> bool {
        (self.money > 0 || !self.items.is_empty())
            && self.looters.contains(&id)
            && self.turn.is_none_or(|(t, _)| t == id)
    }

    /// Whether there's anything here this player may take.
    pub fn allows(&self, id: EntityId) -> bool {
        self.shares_with(id) || self.personal.iter().any(|(p, _)| *p == id)
    }

    fn is_empty(&self) -> bool {
        self.money == 0 && self.items.is_empty() && self.personal.is_empty()
    }
}

pub struct MobData {
    pub kind: MobKind,
    camp: usize,
    pub home: Vec3,
    /// Who the mob wants to hit, and how much.
    pub threat: Vec<(EntityId, f32)>,
    pub state: MobState,
    wander_timer: f32,
    wander_to: Option<Vec3>,
    respawn_timer: f32,
    spell_timer: f32,
    /// Bosses' own mechanics.
    boss: BossTimers,
    pub loot: Option<Loot>,
    /// Who summoned this mob (a player in sandbox mode, or a boss calling
    /// for help); it doesn't come back.
    pub summoner: Option<EntityId>,
}

/// What a townsperson does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcRole {
    Merchant,
    QuestGiver,
}

/// A townsperson who trades or hands out quests.
pub struct NpcData {
    pub role: NpcRole,
    pub zone: Zone,
    pub race: Race,
    pub home: Vec3,
    pub home_yaw: f32,
}

pub enum Brain {
    Player(PlayerData),
    Mob(MobData),
    Npc(NpcData),
}

pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub level: u8,
    pub pos: Vec3,
    pub yaw: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub power: f32,
    pub max_power: f32,
    pub target: Option<EntityId>,
    pub cast: Option<Cast>,
    pub auras: Vec<Aura>,
    /// Remaining and total cooldown per ability.
    pub cooldowns: HashMap<AbilityId, (f32, f32)>,
    pub gcd: f32,
    swing_timer: f32,
    pub dead: bool,
    pub in_combat: bool,
    pub moving: bool,
    /// The emote being played, and seconds since it started.
    pub emote: Option<(Emote, f32)>,
    pub brain: Brain,
}

impl Entity {
    pub fn kind(&self) -> EntityKind {
        match &self.brain {
            Brain::Player(p) => EntityKind::Player(p.class),
            Brain::Mob(m) => {
                let t = m.kind.template();
                EntityKind::Mob {
                    kind: m.kind,
                    aggressive: t.aggressive,
                    elite: t.elite,
                    evading: m.state == MobState::Evading,
                }
            }
            Brain::Npc(n) => match n.role {
                NpcRole::Merchant => EntityKind::Merchant(n.race),
                NpcRole::QuestGiver => EntityKind::QuestGiver(n.race),
            },
        }
    }

    pub fn player(&self) -> Option<&PlayerData> {
        match &self.brain {
            Brain::Player(p) => Some(p),
            _ => None,
        }
    }

    fn player_mut(&mut self) -> Option<&mut PlayerData> {
        match &mut self.brain {
            Brain::Player(p) => Some(p),
            _ => None,
        }
    }

    pub fn mob(&self) -> Option<&MobData> {
        match &self.brain {
            Brain::Mob(m) => Some(m),
            _ => None,
        }
    }

    fn mob_mut(&mut self) -> Option<&mut MobData> {
        match &mut self.brain {
            Brain::Mob(m) => Some(m),
            _ => None,
        }
    }

    fn has_aura(&self, f: impl Fn(AuraKind) -> bool) -> bool {
        self.auras.iter().any(|a| f(a.kind))
    }

    pub fn stunned(&self) -> bool {
        self.has_aura(|k| k == AuraKind::Stun)
    }

    pub fn rooted(&self) -> bool {
        self.has_aura(|k| matches!(k, AuraKind::Stun | AuraKind::Root))
    }

    /// Movement speed multiplier from slows (the strongest one wins).
    /// Movement speed multiplier: the strongest slow times the strongest
    /// speed boost.
    fn move_mult(&self) -> f32 {
        let talent = 1.0 + self.bonuses().map_or(0.0, |b| b.speed);
        let slow = self
            .auras
            .iter()
            .filter_map(|a| match a.kind {
                AuraKind::Slow(f) => Some(f),
                _ => None,
            })
            .fold(1.0, f32::min);
        let fast = self
            .auras
            .iter()
            .filter_map(|a| match a.kind {
                AuraKind::Speed(f) => Some(f),
                _ => None,
            })
            .fold(1.0, f32::max);
        slow * fast * talent
    }

    /// Damage done multiplier from auras like Recklessness and curses.
    fn damage_done(&self) -> f32 {
        self.auras
            .iter()
            .filter_map(|a| match a.kind {
                AuraKind::DamageDone(f) => Some(f),
                _ => None,
            })
            .product()
    }

    /// Damage taken multiplier from auras like Evasion.
    fn damage_taken(&self) -> f32 {
        self.auras
            .iter()
            .filter_map(|a| match a.kind {
                AuraKind::DamageTaken(f) => Some(f),
                _ => None,
            })
            .product()
    }

    fn evading(&self) -> bool {
        self.mob().is_some_and(|m| m.state == MobState::Evading)
    }

    /// Melee reach, counting the size of big mobs.
    fn reach(&self) -> f32 {
        self.mob().map_or(0.0, |m| m.kind.template().size * 0.5)
    }

    /// Talent bonuses (none for mobs).
    fn bonuses(&self) -> Option<&Bonuses> {
        self.player().map(|p| &p.bonuses)
    }

    /// Damage and healing done multiplier from gear.
    fn power_mult(&self) -> f32 {
        1.0 + self.player().map_or(0.0, |p| p.stats.power) / 100.0
    }

    fn view(&self, viewer: EntityId) -> EntityView {
        let (appearance, gear, weapon) = match &self.brain {
            Brain::Player(p) => (p.appearance, p.gear, p.weapon),
            Brain::Mob(_) => (Appearance::default(), [None; 5], None),
            Brain::Npc(n) => (
                Appearance {
                    race: n.race,
                    body: (self.id % 2) as u8,
                    skin: (self.id % 5) as u8,
                    hair_style: 1 + (self.id % 4) as u8,
                    hair_color: (self.id % 6) as u8,
                    ..Default::default()
                },
                [None; 5],
                None,
            ),
        };
        let lootable = self
            .mob()
            .and_then(|m| m.loot.as_ref())
            .is_some_and(|l| l.allows(viewer));
        EntityView {
            id: self.id,
            name: self.name.clone(),
            kind: self.kind(),
            level: self.level,
            pos: self.pos,
            yaw: self.yaw,
            hp: self.hp,
            max_hp: self.max_hp,
            power: self.power,
            max_power: self.max_power,
            target: self.target,
            cast: self.cast.as_ref().map(|c| CastView {
                ability: c.ability,
                elapsed: c.elapsed,
                total: c.total,
            }),
            auras: self
                .auras
                .iter()
                .map(|a| AuraView {
                    ability: a.ability,
                    remaining: a.remaining,
                    duration: a.duration,
                    harmful: a.kind.harmful(),
                    source: a.source,
                    stacks: a.stacks,
                })
                .collect(),
            dead: self.dead,
            in_combat: self.in_combat,
            moving: self.moving,
            appearance,
            gear,
            weapon,
            lootable,
            emote: self
                .emote
                .map(|(emote, elapsed)| EmoteView { emote, elapsed }),
        }
    }
}

/// The mob half of an entity, borrowed separately from its other fields.
fn mob_of(brain: &mut Brain) -> &mut MobData {
    match brain {
        Brain::Mob(m) => m,
        _ => panic!("not a mob"),
    }
}

struct Camp {
    center: Vec2,
    radius: f32,
    kind: MobKind,
    levels: (u8, u8),
    count: usize,
    /// Camps with the same group help each other (a vault pack can mix
    /// kinds). Ordinary camps are their own group.
    group: usize,
    /// The copy of the Sunken Vault this camp is in. Its mobs don't come
    /// back when killed.
    instance: Option<u32>,
}

/// Every zone's camps, in world coordinates, with that zone's mobs.
fn camps() -> Vec<Camp> {
    let mut camps = Vec::new();
    for zone in Zone::ALL {
        let mobs = MobKind::for_zone(zone);
        for site in &zone.layout().sites {
            for spawn in &site.spawns {
                camps.push(Camp {
                    center: zone.to_world(site.center + spawn.offset),
                    radius: spawn.radius,
                    kind: mobs[spawn.role],
                    levels: spawn.levels,
                    count: spawn.count,
                    group: camps.len(),
                    instance: None,
                });
            }
        }
        if let Some(kind) = MobKind::water(zone) {
            for shore in &zone.layout().shores {
                camps.push(Camp {
                    center: zone.to_world(shore.center),
                    radius: shared::layout::SHORE_RADIUS,
                    kind,
                    levels: shared::layout::SHORE_LEVELS,
                    count: shared::layout::SHORE_COUNT,
                    group: camps.len(),
                    instance: None,
                });
            }
        }
    }
    camps
}

/// Something that happens to a target when an ability or aura lands.
enum Hit {
    Damage(f32, bool),
    Heal(f32, bool),
}

pub struct World {
    pub entities: BTreeMap<EntityId, Entity>,
    next_id: EntityId,
    camps: Vec<Camp>,
    outbox: Vec<(Audience, ServerMsg)>,
    rng: Rng,
    tick: u64,
    /// Players may use the sandbox cheats.
    pub sandbox: bool,
    pub parties: HashMap<PartyId, Party>,
    next_party: PartyId,
    /// Open copies of the Sunken Vault, by index.
    pub instances: BTreeMap<u32, Instance>,
    /// Camps of closed vault copies, to reuse.
    free_camps: Vec<usize>,
    /// Ground marked by bosses, about to be hit.
    hazards: Vec<Hazard>,
    /// Spells in flight.
    missiles: Vec<Missile>,
    /// Items dropped on the ground.
    ground: Vec<GroundItem>,
    next_ground: u32,
}

/// How hard an ability hits, fixed when it's used.
#[derive(Clone, Copy, Debug)]
struct Power {
    scale: f32,
    harm: f32,
    heal: f32,
    crit: f32,
}

/// A spell or shot on its way to its target.
#[derive(Clone, Copy, Debug)]
struct Missile {
    caster: EntityId,
    ability: AbilityId,
    target: EntityId,
    /// Seconds until it arrives.
    eta: f32,
    power: Power,
}

impl World {
    pub fn new(seed: u64) -> Self {
        let mut world = World {
            entities: BTreeMap::new(),
            next_id: 1,
            camps: camps(),
            outbox: Vec::new(),
            rng: Rng(seed | 1),
            tick: 0,
            sandbox: false,
            parties: HashMap::new(),
            next_party: 1,
            instances: BTreeMap::new(),
            free_camps: Vec::new(),
            hazards: Vec::new(),
            missiles: Vec::new(),
            ground: Vec::new(),
            next_ground: 1,
        };
        for camp in 0..world.camps.len() {
            for _ in 0..world.camps[camp].count {
                world.spawn_mob(camp);
            }
        }
        for zone in Zone::ALL {
            world.spawn_merchant(zone);
            world.spawn_npc(zone, NpcRole::QuestGiver);
        }
        world
    }

    /// A merchant in a zone's town, facing the square.
    fn spawn_merchant(&mut self, zone: Zone) -> EntityId {
        self.spawn_npc(zone, NpcRole::Merchant)
    }

    /// A townsperson standing at their spot in a zone's town.
    fn spawn_npc(&mut self, zone: Zone, role: NpcRole) -> EntityId {
        let (spot, name) = match role {
            NpcRole::Merchant => (MERCHANT_SPOT, zone.merchant_name()),
            NpcRole::QuestGiver => (QUEST_SPOT, quests::quest_giver_name(zone)),
        };
        let pos = zone.ground_local(spot);
        let center = zone.ground_local(Vec2::ZERO);
        self.spawn_npc_at(zone, role, name, pos, yaw_towards(pos, center))
    }

    /// Puts an NPC of `zone`'s race at `pos`.
    fn spawn_npc_at(
        &mut self,
        zone: Zone,
        role: NpcRole,
        name: &str,
        pos: Vec3,
        yaw: f32,
    ) -> EntityId {
        let id = self.alloc_id();
        self.entities.insert(
            id,
            Entity {
                id,
                name: name.to_string(),
                level: MAX_LEVEL,
                pos,
                yaw,
                hp: 1000.0,
                max_hp: 1000.0,
                power: 0.0,
                max_power: 0.0,
                target: None,
                cast: None,
                auras: Vec::new(),
                cooldowns: HashMap::new(),
                gcd: 0.0,
                swing_timer: 0.0,
                dead: false,
                in_combat: false,
                moving: false,
                emote: None,
                brain: Brain::Npc(NpcData {
                    role,
                    zone,
                    race: zone.race(),
                    home: pos,
                    home_yaw: yaw,
                }),
            },
        );
        id
    }

    fn alloc_id(&mut self) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn spawn_mob(&mut self, camp: usize) -> EntityId {
        let id = self.alloc_id();
        let mut mob = Entity {
            id,
            name: String::new(),
            level: 1,
            pos: Vec3::ZERO,
            yaw: 0.0,
            hp: 0.0,
            max_hp: 0.0,
            power: 0.0,
            max_power: 0.0,
            target: None,
            cast: None,
            auras: Vec::new(),
            cooldowns: HashMap::new(),
            gcd: 0.0,
            swing_timer: 0.0,
            dead: false,
            in_combat: false,
            moving: false,
            emote: None,
            brain: Brain::Mob(MobData {
                kind: self.camps[camp].kind,
                camp,
                home: Vec3::ZERO,
                threat: Vec::new(),
                state: MobState::Idle,
                wander_timer: 0.0,
                wander_to: None,
                respawn_timer: 0.0,
                spell_timer: 0.0,
                boss: BossTimers::default(),
                loot: None,
                summoner: None,
            }),
        };
        self.reset_mob(&mut mob);
        self.entities.insert(id, mob);
        id
    }

    /// Puts a mob back at a random spot in its camp with full health.
    fn reset_mob(&mut self, mob: &mut Entity) {
        let m = mob_of(&mut mob.brain);
        let camp = &self.camps[m.camp];
        let angle = self.rng.range(0.0, std::f32::consts::TAU);
        let dist = camp.radius * self.rng.f32().sqrt();
        let p = camp.center + vec2(angle.cos(), angle.sin()) * dist;
        let level =
            (camp.levels.0 as u32 + self.rng.int(0, (camp.levels.1 - camp.levels.0) as u32)) as u8;
        let kind = m.kind;
        let t = kind.template();
        m.home = ground(p.x, p.y);
        m.threat.clear();
        m.state = MobState::Idle;
        m.wander_timer = self.rng.range(2.0, 8.0);
        m.wander_to = None;
        m.spell_timer = self.rng.range(1.0, 3.0);
        m.boss = BossTimers::default();
        m.loot = None;
        mob.name = t.name.to_string();
        mob.level = level;
        let home = m.home;
        mob.pos = home;
        mob.yaw = self.rng.range(0.0, std::f32::consts::TAU);
        mob.max_hp = kind.max_hp(level);
        mob.hp = mob.max_hp;
        mob.dead = false;
        mob.target = None;
        mob.cast = None;
        mob.auras.clear();
        mob.in_combat = false;
        mob.moving = false;
    }

    /// Brings a saved character into the world and returns their entity id.
    pub fn add_player(&mut self, c: &Character) -> EntityId {
        let id = self.alloc_id();
        let mut pos = clamp_to_world(Vec3::from(c.pos));
        // Vault copies don't outlast the server; wake up in town instead.
        if matches!(Place::at(pos), Place::Dungeon(_)) {
            pos = c.appearance.race.zone().graveyard();
        }
        let stats = gear_stats(&c.gear, c.weapon);
        let ranks = talents::sanitize(&c.talents, c.level);
        let bonuses = Bonuses::new(c.class, &ranks);
        let max_hp =
            (c.class.max_hp(c.level) + stats.stamina * HP_PER_STAMINA) * (1.0 + bonuses.health);
        self.entities.insert(
            id,
            Entity {
                id,
                name: c.name.clone(),
                level: c.level,
                pos: vec3_on_ground(pos),
                yaw: c.yaw,
                hp: max_hp,
                max_hp,
                power: c.class.starting_power(c.level),
                max_power: c.class.max_power(c.level),
                target: None,
                cast: None,
                auras: Vec::new(),
                cooldowns: HashMap::new(),
                gcd: 0.0,
                swing_timer: 0.0,
                dead: false,
                in_combat: false,
                moving: false,
                emote: None,
                brain: Brain::Player(PlayerData {
                    account: c.account.clone(),
                    class: c.class,
                    appearance: c.appearance,
                    xp: c.xp,
                    money: c.money,
                    bags: c.bags.clone(),
                    gear: c.gear,
                    weapon: c.weapon,
                    stats,
                    combo_points: 0,
                    auto_attack: false,
                    move_budget: 0.0,
                    since_spend: MANA_REGEN_DELAY,
                    potion_cooldown: 0.0,
                    talents: ranks,
                    bonuses,
                    hotbar: c.hotbar.map(|bar| c.class.sanitize_hotbar(&bar)),
                    god: false,
                    quests: c.quests.clone(),
                    party: None,
                    invite: None,
                    home_town: Zone::at(pos),
                }),
            },
        );
        self.send(
            Audience::Everyone,
            GameEvent::System(format!("{} has entered the world.", c.name)),
        );
        id
    }

    /// The saveable state of a player who is in the world.
    pub fn character(&self, id: EntityId) -> Option<Character> {
        let e = self.entities.get(&id)?;
        let p = e.player()?;
        // Someone saved in the vault comes back by the waystone of the town
        // they entered it from.
        let pos = match Place::at(e.pos) {
            Place::Zone(_) => e.pos,
            Place::Dungeon(_) => p.home_town.ground_local(ARRIVAL_SPOT),
        };
        Some(Character {
            account: p.account.clone(),
            name: e.name.clone(),
            class: p.class,
            appearance: p.appearance,
            level: e.level,
            xp: p.xp,
            money: p.money,
            pos: pos.to_array(),
            yaw: e.yaw,
            bags: p.bags.clone(),
            gear: p.gear,
            weapon: p.weapon,
            talents: p.talents,
            hotbar: p.hotbar,
            quests: p.quests.clone(),
        })
    }

    /// Takes a player out of the world, returning their character to save.
    pub fn remove_player(&mut self, id: EntityId) -> Option<Character> {
        let mut c = self.character(id)?;
        self.clear_spawns(id);
        self.party_leave(id, "leaves");
        self.decline_invites_from(id);
        // Logging out dead brings you back at the graveyard.
        if self.entities[&id].dead {
            c.pos = Zone::at(Vec3::from(c.pos)).graveyard().to_array();
        }
        self.entities.remove(&id);
        self.forget(id);
        self.send(
            Audience::Everyone,
            GameEvent::System(format!("{} has left the world.", c.name)),
        );
        Some(c)
    }

    /// Removes all references to an entity that is gone or dead.
    fn forget(&mut self, id: EntityId) {
        for e in self.entities.values_mut() {
            if let Some(m) = e.mob_mut() {
                m.threat.retain(|(t, _)| *t != id);
                if let Some(loot) = &mut m.loot {
                    loot.looters.retain(|l| *l != id);
                    if loot.turn.is_some_and(|(t, _)| t == id) {
                        loot.turn = None;
                    }
                }
            }
        }
    }

    pub fn drain_outbox(&mut self) -> Vec<(Audience, ServerMsg)> {
        std::mem::take(&mut self.outbox)
    }

    fn send(&mut self, to: Audience, event: GameEvent) {
        self.outbox.push((to, ServerMsg::Event(event)));
    }

    fn error(&mut self, to: EntityId, msg: &str) {
        self.send(Audience::Only(to), GameEvent::Error(msg.to_string()));
    }

    fn pos_of(&self, id: EntityId) -> Vec3 {
        self.entities.get(&id).map_or(Vec3::ZERO, |e| e.pos)
    }

    /// Everything a player can see right now.
    pub fn snapshot_for(&self, id: EntityId) -> Option<Snapshot> {
        let me = self.entities.get(&id)?;
        let entities = self
            .entities
            .values()
            .filter(|e| {
                e.id == id || me.target == Some(e.id) || e.pos.distance(me.pos) <= VIEW_DISTANCE
            })
            .map(|e| e.view(id))
            .collect();
        let p = me.player()?;
        Some(Snapshot {
            tick: self.tick,
            hazards: self.hazards_near(me.pos),
            portal: self.portal_at(me.pos),
            ground: self.ground_near(id, me.pos),
            entities,
            me: SelfView {
                xp: p.xp,
                xp_next: xp_to_next(me.level),
                cooldowns: me
                    .cooldowns
                    .iter()
                    .map(|(a, (r, t))| (*a, *r, *t))
                    .collect(),
                gcd: me.gcd,
                auto_attacking: p.auto_attack,
                combo_points: p.combo_points,
                potion_cooldown: p.potion_cooldown,
                money: p.money,
                bags: p.bags.clone(),
                stats: p.stats,
                talents: p.talents,
                sandbox: self.sandbox,
                god: p.god,
                quests: p.quests.clone(),
                party: self.party_view(id),
                invite: p
                    .invite
                    .and_then(|(from, _)| self.entities.get(&from))
                    .map(|e| e.name.clone()),
                hotbar: p.hotbar.unwrap_or_else(|| p.class.default_hotbar()),
            },
        })
    }

    pub fn handle(&mut self, id: EntityId, msg: ClientMsg) {
        if !self.entities.contains_key(&id) {
            return;
        }
        match msg {
            ClientMsg::Move { pos, yaw, moving } => self.handle_move(id, pos, yaw, moving),
            ClientMsg::SetTarget(target) => {
                let target = target.filter(|t| self.entities.contains_key(t));
                self.entities.get_mut(&id).unwrap().target = target;
            }
            ClientMsg::UseAbility { ability, target } => {
                if let Some(target) = target.filter(|t| self.entities.contains_key(t)) {
                    self.entities.get_mut(&id).unwrap().target = Some(target);
                }
                if let Err(e) = self.try_use(id, ability) {
                    self.error(id, e);
                }
            }
            ClientMsg::StartAttack => match self.hostile_target(id) {
                Ok(_) => {
                    let e = self.entities.get_mut(&id).unwrap();
                    e.player_mut().unwrap().auto_attack = true;
                }
                Err(err) => self.error(id, err),
            },
            ClientMsg::StopAttack => {
                let e = self.entities.get_mut(&id).unwrap();
                e.player_mut().unwrap().auto_attack = false;
            }
            ClientMsg::SetHotbar(bar) => {
                let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
                p.hotbar = Some(p.class.sanitize_hotbar(&bar));
            }
            ClientMsg::Chat(text) => self.chat(id, text),
            ClientMsg::ReleaseSpirit => self.release(id),
            ClientMsg::Loot(corpse) => {
                if let Err(e) = self.loot(id, corpse) {
                    self.error(id, e);
                }
            }
            ClientMsg::Equip(slot) => {
                if let Err(e) = self.equip(id, slot) {
                    self.error(id, e);
                }
            }
            ClientMsg::Unequip(slot) => {
                if let Err(e) = self.unequip(id, slot) {
                    self.error(id, e);
                }
            }
            ClientMsg::UnequipWeapon => {
                if let Err(e) = self.unequip_weapon(id) {
                    self.error(id, e);
                }
            }
            ClientMsg::Craft(recipe) => {
                if let Err(e) = self.craft(id, recipe) {
                    self.error(id, e);
                }
            }
            ClientMsg::Buy { merchant, item } => {
                if let Err(e) = self.buy(id, merchant, item) {
                    self.error(id, e);
                }
            }
            ClientMsg::Sell { merchant, slot } => {
                if let Err(e) = self.sell(id, merchant, slot) {
                    self.error(id, e);
                }
            }
            ClientMsg::UseItem(slot) => {
                if let Err(e) = self.use_item(id, slot) {
                    self.error(id, e);
                }
            }
            ClientMsg::DropItem(slot) => {
                if let Err(e) = self.drop_item(id, slot) {
                    self.error(id, e);
                }
            }
            ClientMsg::PickUp(item) => {
                if let Err(e) = self.pick_up(id, item) {
                    self.error(id, e);
                }
            }
            ClientMsg::LearnTalent(index) => {
                if let Err(e) = self.learn_talent(id, index) {
                    self.error(id, e);
                }
            }
            ClientMsg::ResetTalents => self.set_talents(id, [0; talents::TALENTS]),
            ClientMsg::Sandbox(cmd) => {
                if let Err(e) = self.sandbox(id, cmd) {
                    self.error(id, e);
                }
            }
            ClientMsg::AcceptQuest { giver, quest } => {
                if let Err(e) = self.accept_quest(id, giver, quest) {
                    self.error(id, e);
                }
            }
            ClientMsg::TurnInQuest { giver, quest } => {
                if let Err(e) = self.turn_in_quest(id, giver, quest) {
                    self.error(id, e);
                }
            }
            ClientMsg::AbandonQuest(quest) => {
                if let Some(p) = self.entities.get_mut(&id).unwrap().player_mut() {
                    p.quests.active.retain(|(q, _)| *q != quest);
                }
            }
            ClientMsg::PartyInvite(name) => {
                if let Err(e) = self.party_invite(id, &name) {
                    self.error(id, e);
                }
            }
            ClientMsg::PartyAccept => {
                if let Err(e) = self.party_accept(id) {
                    self.error(id, e);
                }
            }
            ClientMsg::PartyDecline => self.party_decline(id),
            ClientMsg::PartyLeave => self.party_leave(id, "leaves"),
            ClientMsg::PartyKick(name) => {
                if let Err(e) = self.party_kick(id, &name) {
                    self.error(id, e);
                }
            }
            ClientMsg::PartyPromote(name) => {
                if let Err(e) = self.party_promote(id, &name) {
                    self.error(id, e);
                }
            }
            ClientMsg::Travel(to) => {
                if let Err(e) = self.travel(id, to) {
                    self.error(id, &e);
                }
            }
            // Session messages are handled by the network layer.
            ClientMsg::Hello { .. }
            | ClientMsg::CreateCharacter { .. }
            | ClientMsg::DeleteCharacter(_)
            | ClientMsg::EnterWorld(_)
            | ClientMsg::Logout => {}
        }
    }

    fn handle_move(&mut self, id: EntityId, pos: Vec3, yaw: f32, moving: bool) {
        let e = self.entities.get_mut(&id).unwrap();
        if e.dead || !pos.is_finite() || !yaw.is_finite() {
            return;
        }
        let pos = clamp_to_world(pos);
        let dist = flat_distance(pos, e.pos);
        let rooted = e.rooted() && dist > 0.05;
        let stunned = e.stunned();
        let floating = pos.y > terrain_height(pos.x, pos.z) + 6.0;
        let budget = &mut e.player_mut().unwrap().move_budget;
        let too_far = dist > *budget + 0.5;
        if too_far || rooted || floating {
            let (pos, yaw) = (e.pos, e.yaw);
            self.outbox
                .push((Audience::Only(id), ServerMsg::SetPosition { pos, yaw }));
            return;
        }
        *budget = (*budget - dist).max(0.0);
        if stunned {
            return;
        }
        let interrupt = dist > 0.05 && e.cast.is_some();
        e.pos = pos;
        e.yaw = yaw;
        e.moving = moving;
        if interrupt {
            let ability = e.cast.take().unwrap().ability;
            self.send(
                Audience::Near(pos),
                GameEvent::Interrupted {
                    target: id,
                    ability,
                },
            );
        }
    }

    fn chat(&mut self, id: EntityId, text: String) {
        let text: String = text
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        if text.is_empty() {
            return;
        }
        if text == "/who" {
            let mut names: Vec<String> = self
                .entities
                .values()
                .filter_map(|e| {
                    e.player()
                        .map(|p| format!("{} (level {} {})", e.name, e.level, p.class.name()))
                })
                .collect();
            names.sort();
            let msg = format!("{} online: {}", names.len(), names.join(", "));
            self.send(Audience::Only(id), GameEvent::System(msg));
            return;
        }
        if let Some(rest) = text.strip_prefix('/') {
            let (cmd, arg) = rest.split_once(' ').unwrap_or((rest, ""));
            let arg = arg.trim();
            let result = match cmd.to_ascii_lowercase().as_str() {
                "invite" | "inv" if !arg.is_empty() => self.party_invite(id, arg),
                "accept" => self.party_accept(id),
                "decline" => {
                    self.party_decline(id);
                    Ok(())
                }
                "leave" => {
                    self.party_leave(id, "leaves");
                    Ok(())
                }
                "kick" if !arg.is_empty() => self.party_kick(id, arg),
                "promote" if !arg.is_empty() => self.party_promote(id, arg),
                "p" | "party" if !arg.is_empty() => self.party_chat(id, arg),
                c if Emote::from_command(c).is_some() => {
                    self.emote(id, Emote::from_command(c).unwrap())
                }
                _ => {
                    self.send(
                        Audience::Only(id),
                        GameEvent::System(
                            "Commands: /who, /invite NAME, /accept, /decline, /leave, \
                             /kick NAME, /promote NAME, /p MESSAGE (party chat). \
                             Emotes: /kiss, /sit, /backflip, /wave, /flipoff, /no, /point \
                             (at your target, if you have one)."
                                .into(),
                        ),
                    );
                    Ok(())
                }
            };
            if let Err(e) = result {
                self.error(id, e);
            }
            return;
        }
        let from = self.entities[&id].name.clone();
        self.send(Audience::Everyone, GameEvent::Chat { from, text });
    }

    /// Plays an emote and tells everyone nearby.
    fn emote(&mut self, id: EntityId, emote: Emote) -> Result<(), &'static str> {
        let e = &self.entities[&id];
        if e.dead {
            return Err("You are dead.");
        }
        let target = e
            .target
            .filter(|t| *t != id)
            .and_then(|t| self.entities.get(&t))
            .map(|t| t.name.as_str());
        let text = emote.text(&e.name, target);
        let pos = e.pos;
        self.entities.get_mut(&id).unwrap().emote = Some((emote, 0.0));
        self.send(
            Audience::Near(pos),
            GameEvent::Emote {
                who: id,
                emote,
                text,
            },
        );
        Ok(())
    }

    fn release(&mut self, id: EntityId) {
        let e = self.entities.get_mut(&id).unwrap();
        if !e.dead {
            return;
        }
        e.dead = false;
        e.hp = e.max_hp * 0.5;
        if let Some(p) = e.player()
            && p.class.power_kind() == PowerKind::Mana
        {
            e.power = e.max_power * 0.5;
        }
        e.pos = Place::at(e.pos).graveyard();
        e.yaw = 0.0;
        let (pos, yaw) = (e.pos, e.yaw);
        self.outbox
            .push((Audience::Only(id), ServerMsg::SetPosition { pos, yaw }));
    }

    // ---- Items ----

    fn loot(&mut self, id: EntityId, corpse: EntityId) -> Result<(), &'static str> {
        let me = self.entities.get(&id).ok_or("")?;
        if me.dead {
            return Err("You are dead.");
        }
        let c = self
            .entities
            .get(&corpse)
            .ok_or("There's nothing to loot.")?;
        let allowed = c
            .mob()
            .and_then(|m| m.loot.as_ref())
            .is_some_and(|l| l.allows(id));
        if !c.dead || !allowed {
            return Err("There's nothing to loot.");
        }
        if c.pos.distance(me.pos) > LOOT_RANGE + 1.0 {
            return Err("You are too far away.");
        }
        let mut loot = mob_of(&mut self.entities.get_mut(&corpse).unwrap().brain)
            .loot
            .take()
            .unwrap();
        let shared = loot.shares_with(id);
        let money = if shared {
            loot.turn = None;
            std::mem::take(&mut loot.money)
        } else {
            0
        };
        let shared_items = if shared {
            std::mem::take(&mut loot.items)
        } else {
            Vec::new()
        };
        let mine = loot
            .personal
            .iter()
            .position(|(p, _)| *p == id)
            .map_or(Vec::new(), |i| loot.personal.remove(i).1);
        let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
        p.money += money;
        let mut got = Vec::new();
        let mut full = false;
        // What doesn't fit stays on the corpse, shared or still just yours.
        let mut take = |stacks: Vec<Stack>, bags: &mut Vec<Option<Stack>>| {
            let mut left = Vec::new();
            for (item, n) in stacks {
                let rest = add_item(bags, item, n);
                if rest < n {
                    got.push((item, n - rest));
                }
                if rest > 0 {
                    left.push((item, rest));
                    full = true;
                }
            }
            left
        };
        loot.items.extend(take(shared_items, &mut p.bags));
        let mine_left = take(mine, &mut p.bags);
        if !mine_left.is_empty() {
            loot.personal.push((id, mine_left));
        }
        if !loot.is_empty() {
            mob_of(&mut self.entities.get_mut(&corpse).unwrap().brain).loot = Some(loot);
        }
        if full {
            self.error(id, "Your bags are full.");
        }
        self.send(Audience::Only(id), GameEvent::Looted { money, items: got });
        Ok(())
    }

    /// Recomputes stats and health after gear changes.
    fn refresh_stats(&mut self, id: EntityId) {
        let e = self.entities.get_mut(&id).unwrap();
        let level = e.level;
        let p = e.player_mut().unwrap();
        p.stats = gear_stats(&p.gear, p.weapon);
        let max_hp =
            (p.class.max_hp(level) + p.stats.stamina * HP_PER_STAMINA) * (1.0 + p.bonuses.health);
        let frac = e.hp / e.max_hp.max(1.0);
        e.max_hp = max_hp;
        if !e.dead {
            e.hp = (max_hp * frac).max(1.0);
        }
    }

    fn equip(&mut self, id: EntityId, bag_slot: usize) -> Result<(), &'static str> {
        let e = self.entities.get_mut(&id).unwrap();
        if e.in_combat {
            return Err("You can't change armor in combat.");
        }
        let p = e.player_mut().unwrap();
        let (item_id, _) = p
            .bags
            .get(bag_slot)
            .copied()
            .flatten()
            .ok_or("That slot is empty.")?;
        // Swap: what you were wearing goes where the new piece was.
        let old = match item(item_id).kind {
            ItemKind::Armor { slot, .. } => p.gear[slot.index()].replace(item_id),
            ItemKind::Weapon { .. } => p.weapon.replace(item_id),
            _ => return Err("You can't wear that."),
        };
        p.bags[bag_slot] = old.map(|o| (o, 1));
        self.refresh_stats(id);
        Ok(())
    }

    fn unequip(&mut self, id: EntityId, slot: Slot) -> Result<(), &'static str> {
        let e = self.entities.get_mut(&id).unwrap();
        if e.in_combat {
            return Err("You can't change armor in combat.");
        }
        let p = e.player_mut().unwrap();
        let worn = p.gear[slot.index()].ok_or("Nothing is worn there.")?;
        let free = p
            .bags
            .iter()
            .position(Option::is_none)
            .ok_or("Your bags are full.")?;
        p.bags[free] = Some((worn, 1));
        p.gear[slot.index()] = None;
        self.refresh_stats(id);
        Ok(())
    }

    fn unequip_weapon(&mut self, id: EntityId) -> Result<(), &'static str> {
        let e = self.entities.get_mut(&id).unwrap();
        if e.in_combat {
            return Err("You can't change weapons in combat.");
        }
        let p = e.player_mut().unwrap();
        let held = p.weapon.ok_or("You aren't holding a weapon.")?;
        let free = p
            .bags
            .iter()
            .position(Option::is_none)
            .ok_or("Your bags are full.")?;
        p.bags[free] = Some((held, 1));
        p.weapon = None;
        self.refresh_stats(id);
        Ok(())
    }

    fn craft(&mut self, id: EntityId, recipe: usize) -> Result<(), &'static str> {
        let r = RECIPES.get(recipe).ok_or("Unknown recipe.")?;
        let e = self.entities.get_mut(&id).unwrap();
        if e.dead {
            return Err("You are dead.");
        }
        let p = e.player_mut().unwrap();
        if r.materials
            .iter()
            .any(|(m, n)| count_item(&p.bags, *m) < *n as u32)
        {
            return Err("You don't have the materials.");
        }
        let mut bags = p.bags.clone();
        for (m, n) in r.materials {
            remove_item(&mut bags, *m, *n);
        }
        if add_item(&mut bags, r.result, 1) > 0 {
            return Err("Your bags are full.");
        }
        p.bags = bags;
        self.send(Audience::Only(id), GameEvent::Crafted(r.result));
        Ok(())
    }

    /// Checks that a merchant is close enough to trade with.
    fn learn_talent(&mut self, id: EntityId, index: usize) -> Result<(), &'static str> {
        let e = &self.entities[&id];
        let p = e.player().ok_or("Only players have talents.")?;
        talents::can_learn(&p.talents, e.level, index)?;
        let mut ranks = p.talents;
        ranks[index] += 1;
        let name = talents::tree(p.class).talents[index].name;
        self.set_talents(id, ranks);
        self.send(
            Audience::Only(id),
            GameEvent::System(format!("You learn {name} (rank {}).", ranks[index])),
        );
        Ok(())
    }

    fn set_talents(&mut self, id: EntityId, ranks: Ranks) {
        let e = self.entities.get_mut(&id).unwrap();
        let level = e.level;
        let Some(p) = e.player_mut() else { return };
        p.talents = talents::sanitize(&ranks, level);
        p.bonuses = Bonuses::new(p.class, &p.talents);
        self.refresh_stats(id);
    }

    /// Sandbox cheats.
    fn sandbox(&mut self, id: EntityId, cmd: SandboxCmd) -> Result<(), &'static str> {
        if !self.sandbox {
            return Err("Sandbox commands only work in sandbox mode.");
        }
        match cmd {
            SandboxCmd::SetLevel(level) => {
                let level = level.clamp(1, MAX_LEVEL);
                let e = self.entities.get_mut(&id).unwrap();
                let class = e.player().unwrap().class;
                let old = e.level;
                e.level = level;
                e.player_mut().unwrap().xp = 0;
                e.max_power = class.max_power(level);
                e.power = class.starting_power(level).max(e.power.min(e.max_power));
                let ranks = e.player().unwrap().talents;
                self.set_talents(id, ranks);
                let e = self.entities.get_mut(&id).unwrap();
                e.hp = e.max_hp;
                let pos = e.pos;
                if level != old {
                    self.send(Audience::Near(pos), GameEvent::LevelUp { id, level });
                }
            }
            SandboxCmd::AddMoney(copper) => {
                let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
                p.money = p.money.saturating_add(copper);
            }
            SandboxCmd::GiveItem(item_id) => {
                if item_id.0 as usize >= ITEMS.len() {
                    return Err("No such item.");
                }
                let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
                if add_item(&mut p.bags, item_id, 1) > 0 {
                    return Err("Your bags are full.");
                }
            }
            SandboxCmd::Teleport(zone) => {
                self.clear_spawns(id);
                let yaw = self.entities[&id].yaw;
                self.teleport(id, zone.graveyard(), yaw);
            }
            SandboxCmd::ToggleGod => {
                let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
                p.god = !p.god;
                let msg = if p.god {
                    "God mode on."
                } else {
                    "God mode off."
                };
                self.send(Audience::Only(id), GameEvent::System(msg.into()));
            }
            SandboxCmd::SpawnMob { kind, level } => {
                let e = &self.entities[&id];
                let spot = e.pos + forward(e.yaw) * 8.0;
                let level = level.clamp(1, MAX_LEVEL);
                self.camps.push(Camp {
                    center: vec2(spot.x, spot.z),
                    radius: 0.5,
                    kind,
                    levels: (level, level),
                    count: 0,
                    group: self.camps.len(),
                    instance: None,
                });
                let camp = self.camps.len() - 1;
                let mob = self.spawn_mob(camp);
                let me = self.pos_of(id);
                let m = self.entities.get_mut(&mob).unwrap();
                m.yaw = yaw_towards(m.pos, me);
                mob_of(&mut m.brain).summoner = Some(id);
            }
            SandboxCmd::ClearSpawns => self.clear_spawns(id),
            SandboxCmd::Refresh => {
                let e = self.entities.get_mut(&id).unwrap();
                e.cooldowns.clear();
                e.gcd = 0.0;
                e.hp = e.max_hp;
                e.power = e.max_power;
                if let Some(p) = e.player_mut() {
                    p.potion_cooldown = 0.0;
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn mob_count(&self) -> usize {
        self.entities.values().filter(|e| e.mob().is_some()).count()
    }

    /// Removes the mobs a player summoned in sandbox mode.
    fn clear_spawns(&mut self, id: EntityId) {
        let gone: Vec<EntityId> = self
            .entities
            .values()
            .filter(|e| e.mob().is_some_and(|m| m.summoner == Some(id)))
            .map(|e| e.id)
            .collect();
        for mob in gone {
            self.entities.remove(&mob);
            self.forget(mob);
        }
    }

    /// Checks a player is next to a quest giver who offers this quest.
    fn giver_near(
        &self,
        id: EntityId,
        giver: EntityId,
        quest: QuestId,
    ) -> Result<(), &'static str> {
        if quest.0 as usize >= quests::QUESTS.len() {
            return Err("No such quest.");
        }
        let me = &self.entities[&id];
        let g = self.entities.get(&giver).ok_or("There's no one here.")?;
        let Brain::Npc(n) = &g.brain else {
            return Err("They have no quests for you.");
        };
        let theirs = quests::offered(Place::at(g.pos));
        if n.role != NpcRole::QuestGiver || !theirs.iter().any(|q| q.id == quest) {
            return Err("They have no quests for you.");
        }
        if g.pos.distance(me.pos) > MERCHANT_RANGE + 1.0 {
            return Err("You are too far away.");
        }
        if me.dead {
            return Err("You are dead.");
        }
        Ok(())
    }

    fn accept_quest(
        &mut self,
        id: EntityId,
        giver: EntityId,
        quest: QuestId,
    ) -> Result<(), &'static str> {
        self.giver_near(id, giver, quest)?;
        let e = self.entities.get_mut(&id).unwrap();
        let level = e.level;
        let p = e.player_mut().ok_or("Only players take quests.")?;
        let q = quests::quest(quest);
        p.quests.can_accept(q, level)?;
        p.quests.active.push((quest, 0));
        self.send(Audience::Only(id), GameEvent::QuestAccepted(quest));
        Ok(())
    }

    fn turn_in_quest(
        &mut self,
        id: EntityId,
        giver: EntityId,
        quest: QuestId,
    ) -> Result<(), &'static str> {
        self.giver_near(id, giver, quest)?;
        let q = quests::quest(quest);
        let p = self
            .entities
            .get_mut(&id)
            .unwrap()
            .player_mut()
            .ok_or("Only players take quests.")?;
        let progress = p
            .quests
            .progress(quest)
            .ok_or("You're not on that quest.")?;
        match q.goal {
            Goal::Kill { count, .. } => {
                if progress < count {
                    return Err("You haven't finished that quest yet.");
                }
            }
            Goal::TurnIn { item } => {
                if count_item(&p.bags, item) == 0 {
                    return Err("You don't have what they asked for.");
                }
            }
        }
        // Make room for the reward before taking anything.
        if let Some(reward) = q.reward {
            let mut bags = p.bags.clone();
            if let Goal::TurnIn { item } = q.goal {
                remove_item(&mut bags, item, 1);
            }
            if add_item(&mut bags, reward, 1) > 0 {
                return Err("Your bags are full.");
            }
            p.bags = bags;
        } else if let Goal::TurnIn { item } = q.goal {
            remove_item(&mut p.bags, item, 1);
        }
        p.quests.active.retain(|(qid, _)| *qid != quest);
        p.quests.done.push(quest);
        p.money = p.money.saturating_add(q.money);
        self.send(
            Audience::Only(id),
            GameEvent::QuestComplete {
                quest,
                xp: q.xp,
                money: q.money,
                reward: q.reward,
            },
        );
        if self.entities[&id].level < MAX_LEVEL {
            self.give_xp(id, q.xp, q.name);
        }
        Ok(())
    }

    /// Counts a kill for everyone hunting this kind of mob.
    fn quest_kill(&mut self, player: EntityId, kind: MobKind) {
        let Some(p) = self.entities.get_mut(&player).and_then(|e| e.player_mut()) else {
            return;
        };
        let mut updates = Vec::new();
        for (qid, progress) in &mut p.quests.active {
            if let Goal::Kill { kind: k, count } = quests::quest(*qid).goal
                && k == kind
                && *progress < count
            {
                *progress += 1;
                updates.push((*qid, *progress));
            }
        }
        for (quest, progress) in updates {
            self.send(
                Audience::Only(player),
                GameEvent::QuestProgress { quest, progress },
            );
        }
    }

    fn merchant_near(&self, id: EntityId, merchant: EntityId) -> Result<(), &'static str> {
        let me = &self.entities[&id];
        let m = self
            .entities
            .get(&merchant)
            .ok_or("There's no one to trade with.")?;
        if !matches!(&m.brain, Brain::Npc(n) if n.role == NpcRole::Merchant) {
            return Err("They won't trade with you.");
        }
        if m.pos.distance(me.pos) > MERCHANT_RANGE + 1.0 {
            return Err("You are too far away.");
        }
        if me.dead {
            return Err("You are dead.");
        }
        Ok(())
    }

    fn buy(
        &mut self,
        id: EntityId,
        merchant: EntityId,
        item_id: ItemId,
    ) -> Result<(), &'static str> {
        self.merchant_near(id, merchant)?;
        if !MERCHANT_GOODS.contains(&item_id) {
            return Err("That isn't for sale.");
        }
        let price = item(item_id).price;
        let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
        if p.money < price {
            return Err("You don't have enough money.");
        }
        let mut bags = p.bags.clone();
        if add_item(&mut bags, item_id, 1) > 0 {
            return Err("Your bags are full.");
        }
        p.bags = bags;
        p.money -= price;
        self.send(
            Audience::Only(id),
            GameEvent::Bought {
                item: item_id,
                price,
            },
        );
        Ok(())
    }

    fn sell(&mut self, id: EntityId, merchant: EntityId, slot: usize) -> Result<(), &'static str> {
        self.merchant_near(id, merchant)?;
        let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
        let (item_id, count) = p
            .bags
            .get(slot)
            .copied()
            .flatten()
            .ok_or("That slot is empty.")?;
        let money = item(item_id).sell_price() * count as u32;
        p.bags[slot] = None;
        p.money += money;
        self.send(
            Audience::Only(id),
            GameEvent::Sold {
                item: item_id,
                count,
                money,
            },
        );
        Ok(())
    }

    fn use_item(&mut self, id: EntityId, slot: usize) -> Result<(), &'static str> {
        let e = self.entities.get_mut(&id).unwrap();
        if e.dead {
            return Err("You are dead.");
        }
        let p = e.player_mut().unwrap();
        let (item_id, count) = p
            .bags
            .get(slot)
            .copied()
            .flatten()
            .ok_or("That slot is empty.")?;
        let (health, power) = match item(item_id).kind {
            ItemKind::Potion { health, power } => (health, power),
            ItemKind::Food { health } => (health, 0.0),
            _ => return Err("You can't use that."),
        };
        if p.potion_cooldown > 0.0 {
            return Err("Potions and food are not ready yet.");
        }
        p.potion_cooldown = POTION_COOLDOWN;
        p.bags[slot] = (count > 1).then_some((item_id, count - 1));
        let heal = e.max_hp * health;
        e.power = (e.power + e.max_power * power).min(e.max_power);
        if heal > 0.0 {
            self.apply_hit(id, id, Hit::Heal(heal, false), None);
        }
        Ok(())
    }

    // ---- Combat ----

    /// The caster's target, if it's something they can attack.
    fn hostile_target(&self, id: EntityId) -> Result<EntityId, &'static str> {
        let e = &self.entities[&id];
        let target = e.target.ok_or("You have no target.")?;
        let t = self.entities.get(&target).ok_or("You have no target.")?;
        if !e.kind().hostile_to(t.kind()) {
            return Err("Invalid target.");
        }
        if t.dead {
            return Err("Your target is dead.");
        }
        Ok(target)
    }

    /// Checks whether `caster` can use `ability` right now, and starts it.
    pub fn try_use(&mut self, caster: EntityId, id: AbilityId) -> Result<(), &'static str> {
        let a = try_ability(id).ok_or("Unknown ability.")?;
        let e = &self.entities[&caster];
        if let Some(p) = e.player() {
            match p.class.unlock_level(id) {
                None => return Err("You don't know that ability."),
                Some(l) if l > e.level => return Err("You haven't learned that yet."),
                Some(_) => {}
            }
            if a.needs_combo_points() && p.combo_points == 0 {
                return Err("That ability requires combo points.");
            }
        }
        if e.dead {
            return Err("You are dead.");
        }
        if e.stunned() {
            return Err("Can't do that while stunned.");
        }
        if e.cast.is_some() {
            return Err("You are already casting.");
        }
        if e.gcd > 0.0 || e.cooldowns.contains_key(&id) {
            return Err("Ability is not ready yet.");
        }
        if a.moves_caster() && e.rooted() {
            return Err("Can't do that while rooted.");
        }
        let target = self.resolve_target(caster, a, false)?;
        if a.min_range > 0.0
            && let Some(t) = target
            && self.entities[&t].pos.distance(e.pos) < a.min_range
        {
            return Err("Too close.");
        }
        if a.from_behind {
            let t = &self.entities[&target.unwrap()];
            if is_facing(t.pos, t.yaw, e.pos) {
                return Err("You must be behind your target.");
            }
        }
        // Mobs (and sandbox gods) don't pay for their spells.
        let pays = e.player().is_some_and(|p| !p.god);
        if pays && e.power < a.cost {
            return Err(match e.player().map(|p| p.class.power_kind()) {
                Some(PowerKind::Rage) => "Not enough rage.",
                Some(PowerKind::Energy) => "Not enough energy.",
                _ => "Not enough mana.",
            });
        }
        let cast_time = e.bonuses().map_or(a.cast_time, |b| b.cast_time(id));
        let e = self.entities.get_mut(&caster).unwrap();
        e.gcd = GCD;
        if cast_time > 0.0 {
            e.cast = Some(Cast {
                ability: id,
                target,
                elapsed: 0.0,
                total: cast_time,
            });
        } else {
            self.complete(caster, id, target);
        }
        // Melee classes start swinging when they use an ability on an enemy.
        if a.targeting.needs_enemy()
            && let Some(p) = self.entities.get_mut(&caster).unwrap().player_mut()
            && p.class.is_melee()
        {
            p.auto_attack = true;
        }
        Ok(())
    }

    /// Picks the target of an ability and checks range and facing.
    /// `finishing` is set when a cast completes; it is a little more lenient.
    fn resolve_target(
        &self,
        caster: EntityId,
        a: &Ability,
        finishing: bool,
    ) -> Result<Option<EntityId>, &'static str> {
        let e = &self.entities[&caster];
        let leeway = if finishing { 3.0 } else { 1.0 };
        let in_range = |t: &Entity| e.pos.distance(t.pos) <= a.range + leeway + t.reach();
        match a.targeting {
            Targeting::Enemy | Targeting::AroundTarget(_) => {
                let target = self.hostile_target(caster)?;
                let t = &self.entities[&target];
                if !in_range(t) {
                    return Err("Out of range.");
                }
                if e.player().is_some() && !finishing && !is_facing(e.pos, e.yaw, t.pos) {
                    return Err("You are facing the wrong way!");
                }
                Ok(Some(target))
            }
            Targeting::Friendly => {
                let friend = e
                    .target
                    .and_then(|t| self.entities.get(&t))
                    .filter(|t| !t.dead && !e.kind().hostile_to(t.kind()));
                match friend {
                    Some(t) if t.id != caster => {
                        if !in_range(t) {
                            return Err("Out of range.");
                        }
                        Ok(Some(t.id))
                    }
                    _ => Ok(Some(caster)),
                }
            }
            Targeting::Caster | Targeting::AroundCaster(_) => Ok(None),
        }
    }

    /// Moves an entity (Charge, leaps) and tells its player.
    fn relocate(&mut self, id: EntityId, pos: Vec3, yaw: f32) {
        let e = self.entities.get_mut(&id).unwrap();
        // Never jump into another zone.
        let pos = if Place::at(pos) == Place::at(e.pos) {
            pos
        } else {
            e.pos
        };
        e.pos = pos;
        e.yaw = yaw;
        e.moving = false;
        if e.player().is_some() {
            self.outbox
                .push((Audience::Only(id), ServerMsg::SetPosition { pos, yaw }));
        }
    }

    /// Rolls an amount between `min` and `max`, maybe a critical hit (with
    /// `crit` added chance from talents).
    fn roll_with(&mut self, min: f32, max: f32, scale: f32, crit: f32) -> (f32, bool) {
        let crit = self.rng.chance(CRIT_CHANCE + crit);
        let amount = self.rng.range(min, max) * scale * if crit { CRIT_MULTIPLIER } else { 1.0 };
        (amount, crit)
    }

    /// An ability goes off: pay for it and apply its effects.
    fn complete(&mut self, caster: EntityId, id: AbilityId, target: Option<EntityId>) {
        let a = ability(id);
        let (pos, scale, harm_scale, heal_scale, crit) = {
            let e = self.entities.get_mut(&caster).unwrap();
            if e.player().is_some_and(|p| !p.god) {
                e.power = (e.power - a.cost).max(0.0);
            }
            let bonus = e.bonuses().cloned().unwrap_or_default();
            let cooldown = bonus.cooldown(id);
            if cooldown > 0.0 {
                e.cooldowns.insert(id, (cooldown, cooldown));
            }
            let scale = level_scale(e.level) * e.power_mult() * (1.0 + bonus.ability(id).power);
            if let Some(p) = e.player_mut()
                && a.cost > 0.0
            {
                p.since_spend = 0.0;
            }
            (
                e.pos,
                scale,
                scale * (1.0 + bonus.damage),
                scale * (1.0 + bonus.healing),
                bonus.crit,
            )
        };
        self.send(
            Audience::Near(pos),
            GameEvent::AbilityUsed {
                caster,
                target,
                ability: id,
            },
        );
        let power = Power {
            scale,
            harm: harm_scale,
            heal: heal_scale,
            crit,
        };
        // A missile hits when it gets there, not when it's thrown.
        if a.projectile
            && let Some(t) = target.filter(|&t| t != caster)
        {
            let class = match self.entities[&caster].kind() {
                EntityKind::Player(c) => Some(c),
                _ => None,
            };
            let eta = pos.distance(self.pos_of(t)) / a.missile_speed(class);
            self.missiles.push(Missile {
                caster,
                ability: id,
                target: t,
                eta,
                power,
            });
            return;
        }
        self.land(caster, id, target, power);
    }

    /// Missiles fly on; those that arrive land their ability's effects.
    fn tick_missiles(&mut self, dt: f32) {
        let mut arrived = Vec::new();
        self.missiles.retain_mut(|m| {
            m.eta -= dt;
            if m.eta > 0.0 {
                return true;
            }
            arrived.push(*m);
            false
        });
        for m in arrived {
            let alive = |id| self.entities.get(&id).is_some_and(|e: &Entity| !e.dead);
            if self.entities.contains_key(&m.caster) && alive(m.target) {
                self.land(m.caster, m.ability, Some(m.target), m.power);
            }
        }
    }

    /// An ability's effects reach their targets.
    fn land(&mut self, caster: EntityId, id: AbilityId, target: Option<EntityId>, power: Power) {
        let a = ability(id);
        let pos = self.pos_of(caster);
        let Power {
            scale,
            harm: harm_scale,
            heal: heal_scale,
            crit,
        } = power;
        let targets: Vec<EntityId> = match a.targeting {
            Targeting::Enemy | Targeting::Friendly => target.into_iter().collect(),
            Targeting::Caster => vec![caster],
            Targeting::AroundCaster(radius) => {
                let kind = self.entities[&caster].kind();
                self.entities
                    .values()
                    .filter(|t| {
                        !t.dead
                            && kind.hostile_to(t.kind())
                            && t.pos.distance(pos) <= radius + t.reach()
                    })
                    .map(|t| t.id)
                    .collect()
            }
            Targeting::AroundTarget(radius) => {
                let kind = self.entities[&caster].kind();
                let center = target.map_or(pos, |t| self.pos_of(t));
                let mut hit: Vec<EntityId> = target.into_iter().collect();
                hit.extend(
                    self.entities
                        .values()
                        .filter(|t| {
                            Some(t.id) != target
                                && !t.dead
                                && kind.hostile_to(t.kind())
                                && t.pos.distance(center) <= radius + t.reach()
                        })
                        .map(|t| t.id),
                );
                hit
            }
        };
        for t in targets {
            for effect in a.effects {
                if self.entities.get(&t).is_none_or(|e| e.dead) {
                    break;
                }
                match *effect {
                    Effect::Damage { min, max } => {
                        let (amount, crit) = self.roll_with(min, max, harm_scale, crit);
                        self.apply_hit(caster, t, Hit::Damage(amount, crit), Some(id));
                    }
                    Effect::Heal { min, max } => {
                        let (amount, crit) = self.roll_with(min, max, heal_scale, crit);
                        self.apply_hit(caster, t, Hit::Heal(amount, crit), Some(id));
                    }
                    Effect::Aura { kind, duration } => {
                        let s = match kind {
                            AuraKind::Dot { .. } => harm_scale,
                            AuraKind::Hot { .. } | AuraKind::Absorb(_) => heal_scale,
                            _ => scale,
                        };
                        self.apply_aura(caster, t, id, kind, duration, s)
                    }
                    Effect::Interrupt => {
                        let target = self.entities.get_mut(&t).unwrap();
                        if let Some(cast) = target.cast.take() {
                            let p = target.pos;
                            self.send(
                                Audience::Near(p),
                                GameEvent::Interrupted {
                                    target: t,
                                    ability: cast.ability,
                                },
                            );
                        }
                    }
                    Effect::Taunt => {
                        if let Some(m) = self.entities.get_mut(&t).unwrap().mob_mut() {
                            let top = m.threat.iter().map(|(_, v)| *v).fold(0.0, f32::max);
                            add_threat(m, caster, 0.0);
                            for (who, v) in &mut m.threat {
                                if *who == caster {
                                    *v = top * 1.1 + 10.0;
                                }
                            }
                            m.state = MobState::Combat;
                        }
                    }
                    Effect::RestorePower(frac) => {
                        let e = self.entities.get_mut(&caster).unwrap();
                        e.power = (e.power + e.max_power * frac).min(e.max_power);
                    }
                    Effect::Charge => {
                        let me = self.pos_of(caster);
                        let to = self.pos_of(t);
                        let gap = MELEE_RANGE * 0.6 + self.entities[&t].reach();
                        let dir = (to - me).normalize_or_zero();
                        let dest = to - dir * gap;
                        self.relocate(caster, ground(dest.x, dest.z), yaw_towards(me, to));
                    }
                    Effect::Leap(distance) => {
                        let e = &self.entities[&caster];
                        let dest = e.pos + forward(e.yaw) * distance;
                        let yaw = e.yaw;
                        self.relocate(caster, clamp_to_world(ground(dest.x, dest.z)), yaw);
                    }
                    Effect::Drain { min, max } => {
                        let (amount, crit) = self.roll_with(min, max, harm_scale, crit);
                        self.apply_hit(caster, t, Hit::Damage(amount, crit), Some(id));
                        self.apply_hit(caster, caster, Hit::Heal(amount, crit), Some(id));
                    }
                    Effect::ComboPoint => {
                        if let Some(p) = self.entities.get_mut(&caster).unwrap().player_mut() {
                            p.combo_points = (p.combo_points + 1).min(MAX_COMBO_POINTS);
                        }
                    }
                    Effect::Finisher {
                        min,
                        max,
                        per_point,
                    } => {
                        let points = self
                            .entities
                            .get_mut(&caster)
                            .unwrap()
                            .player_mut()
                            .map_or(1, |p| std::mem::take(&mut p.combo_points));
                        let (amount, crit) = self.roll_with(
                            min + per_point * points as f32,
                            max + per_point * points as f32,
                            harm_scale,
                            crit,
                        );
                        self.apply_hit(caster, t, Hit::Damage(amount, crit), Some(id));
                    }
                }
            }
        }
    }

    fn apply_aura(
        &mut self,
        source: EntityId,
        target: EntityId,
        ability_id: AbilityId,
        kind: AuraKind,
        duration: f32,
        scale: f32,
    ) {
        let kind = match kind {
            AuraKind::Dot { per_tick, interval } => AuraKind::Dot {
                per_tick: per_tick * scale,
                interval,
            },
            AuraKind::Hot { per_tick, interval } => AuraKind::Hot {
                per_tick: per_tick * scale,
                interval,
            },
            AuraKind::Absorb(a) => AuraKind::Absorb(a * scale),
            k => k,
        };
        if kind.harmful() && self.entities[&target].evading() {
            let pos = self.pos_of(target);
            self.send(Audience::Near(pos), GameEvent::Evade { target });
            return;
        }
        let interval = match kind {
            AuraKind::Dot { interval, .. } | AuraKind::Hot { interval, .. } => interval,
            _ => 0.0,
        };
        let max_stacks = ability(ability_id).max_stacks;
        let t = self.entities.get_mut(&target).unwrap();
        match t
            .auras
            .iter_mut()
            .find(|a| a.ability == ability_id && a.source == source)
        {
            // Reapplying refreshes the duration and adds a stack if it can.
            Some(existing) => {
                existing.stacks = (existing.stacks + 1).min(max_stacks);
                existing.remaining = duration;
                existing.duration = duration;
                existing.kind = kind;
            }
            None => t.auras.push(Aura {
                ability: ability_id,
                kind,
                duration,
                remaining: duration,
                tick_timer: interval,
                source,
                stacks: 1,
            }),
        }
        if kind == AuraKind::Stun
            && let Some(cast) = t.cast.take()
        {
            let p = t.pos;
            self.send(
                Audience::Near(p),
                GameEvent::Interrupted {
                    target,
                    ability: cast.ability,
                },
            );
        }
        if kind.harmful() {
            self.provoke(source, target, 5.0);
        }
    }

    /// `source` did something hostile to `target`: both enter combat, and a
    /// mob that was attacked turns on its attacker.
    fn provoke(&mut self, source: EntityId, target: EntityId, threat: f32) {
        let Some(t) = self.entities.get_mut(&target) else {
            return;
        };
        if let Some(m) = t.mob_mut() {
            let first = m.state == MobState::Idle;
            add_threat(m, source, threat.max(0.1));
            m.state = MobState::Combat;
            if first {
                self.social_aggro(target, source);
            }
        } else if let Some(m) = self.entities.get_mut(&source).and_then(Entity::mob_mut) {
            add_threat(m, target, 0.0);
        }
    }

    /// Social mobs (people) call nearby friends from the same camp into the fight.
    fn social_aggro(&mut self, mob: EntityId, enemy: EntityId) {
        let (group, pos, social) = {
            let e = &self.entities[&mob];
            let m = e.mob().unwrap();
            (self.camps[m.camp].group, e.pos, m.kind.template().social)
        };
        if !social {
            return;
        }
        for e in self.entities.values_mut() {
            if e.dead || e.id == mob || e.pos.distance(pos) > 8.0 {
                continue;
            }
            if let Some(m) = e.mob_mut()
                && self.camps[m.camp].group == group
                && m.state == MobState::Idle
            {
                add_threat(m, enemy, 1.0);
                m.state = MobState::Combat;
            }
        }
    }

    fn apply_hit(
        &mut self,
        source: EntityId,
        target: EntityId,
        hit: Hit,
        ability_id: Option<AbilityId>,
    ) {
        let Some(t) = self.entities.get(&target) else {
            return;
        };
        if t.dead {
            return;
        }
        let pos = t.pos;
        match hit {
            Hit::Damage(amount, crit) => {
                if t.evading() || matches!(t.brain, Brain::Npc(_)) {
                    self.send(Audience::Near(pos), GameEvent::Evade { target });
                    return;
                }
                // Buffs and curses on the attacker.
                let amount = amount * self.entities.get(&source).map_or(1.0, |s| s.damage_done());
                // Armor softens physical blows.
                let physical = ability_id.is_none_or(|a| ability(a).school == School::Physical);
                let attacker_level = self.entities.get(&source).map_or(1, |s| s.level);
                let armor = t.player().map_or(0.0, |p| p.stats.armor);
                let toughness = t.bonuses().map_or(0.0, |b| b.toughness);
                let mut amount = amount * t.damage_taken() * (1.0 - toughness).max(0.2);
                if t.player().is_some_and(|p| p.god) {
                    amount = 0.0;
                }
                if physical {
                    amount *= armor_multiplier(armor, attacker_level);
                }
                let t = self.entities.get_mut(&target).unwrap();
                let mut left = if amount <= 0.0 {
                    0.0
                } else {
                    amount.round().max(1.0)
                };
                let mut absorbed = 0.0;
                for aura in &mut t.auras {
                    if let AuraKind::Absorb(shield) = &mut aura.kind {
                        let soak = left.min(*shield);
                        *shield -= soak;
                        left -= soak;
                        absorbed += soak;
                        if *shield <= 0.0 {
                            aura.remaining = 0.0;
                        }
                    }
                }
                t.auras.retain(|a| a.remaining > 0.0);
                t.hp -= left;
                let target_level = t.level;
                if t.player()
                    .is_some_and(|p| p.class.power_kind() == PowerKind::Rage)
                {
                    t.power = (t.power + left / level_scale(target_level) * 0.5).min(t.max_power);
                }
                let dead = t.hp <= 0.0;
                // Leeching talents heal the attacker.
                if let Some(s) = self.entities.get_mut(&source)
                    && !s.dead
                    && let Some(leech) = s.bonuses().map(|b| b.leech).filter(|l| *l > 0.0)
                {
                    s.hp = (s.hp + left * leech).min(s.max_hp);
                }
                if let Some(s) = self.entities.get_mut(&source)
                    && ability_id.is_none()
                    && s.player()
                        .is_some_and(|p| p.class.power_kind() == PowerKind::Rage)
                {
                    s.power = (s.power + amount / level_scale(s.level) * 1.2).min(s.max_power);
                }
                self.send(
                    Audience::Near(pos),
                    GameEvent::Damage {
                        source,
                        target,
                        amount: left as u32,
                        absorbed: absorbed as u32,
                        crit,
                        ability: ability_id,
                    },
                );
                let mult = ability_id.map_or(1.0, |a| ability(a).threat);
                self.provoke(source, target, amount * mult);
                if dead {
                    self.kill(target, Some(source));
                }
            }
            Hit::Heal(amount, crit) => {
                let t = self.entities.get_mut(&target).unwrap();
                let healed = amount.round().min(t.max_hp - t.hp).max(0.0);
                t.hp += healed;
                self.send(
                    Audience::Near(pos),
                    GameEvent::Heal {
                        source,
                        target,
                        amount: healed as u32,
                        crit,
                        ability: ability_id.unwrap_or(ids::HEAL),
                    },
                );
                // Healing someone draws the attention of whatever is fighting them.
                for e in self.entities.values_mut() {
                    if let Some(m) = e.mob_mut()
                        && m.state == MobState::Combat
                        && m.threat.iter().any(|(id, _)| *id == target)
                    {
                        add_threat(m, source, healed * 0.5);
                    }
                }
            }
        }
    }

    fn roll_loot(&mut self, kind: MobKind, level: u8, looters: Vec<EntityId>) -> Option<Loot> {
        let table = &kind.template().loot;
        let (lo, hi) = table.copper_per_level;
        let money = self.rng.int(lo, hi) * level as u32;
        let mut items = Vec::new();
        for &(item, chance, min, max) in table.items {
            if !is_gear(item) && self.rng.chance(chance) {
                items.push((item, self.rng.int(min as u32, max as u32) as u16));
            }
        }
        // The dungeon rolls gear for each player on their own; elsewhere
        // it's shared like everything else.
        let mut personal = Vec::new();
        if MobKind::DUNGEON.contains(&kind) {
            for &p in &looters {
                let gear = self.roll_gear(kind);
                if !gear.is_empty() {
                    personal.push((p, gear));
                }
            }
        } else {
            items.extend(self.roll_gear(kind));
        }
        let turn = self.loot_turn(&looters).map(|t| (t, LOOT_TURN_TIME));
        (!looters.is_empty()).then_some(Loot {
            money,
            items,
            looters,
            turn,
            personal,
        })
    }

    /// The gear a mob drops: anything wearable from its loot table, and now
    /// and then a green piece or weapon.
    fn roll_gear(&mut self, kind: MobKind) -> Vec<Stack> {
        let mut gear = Vec::new();
        for &(item, chance, min, max) in kind.template().loot.items {
            if is_gear(item) && self.rng.chance(chance) {
                gear.push((item, self.rng.int(min as u32, max as u32) as u16));
            }
        }
        let (rare, weapon) = kind.drop_chances();
        if self.rng.chance(rare) {
            let pick = RARE_DROPS[self.rng.int(0, RARE_DROPS.len() as u32 - 1) as usize];
            gear.push((pick, 1));
        }
        if self.rng.chance(weapon) {
            let pick = WEAPON_DROPS[self.rng.int(0, WEAPON_DROPS.len() as u32 - 1) as usize];
            gear.push((pick, 1));
        }
        gear
    }

    fn kill(&mut self, victim: EntityId, killer: Option<EntityId>) {
        let e = self.entities.get_mut(&victim).unwrap();
        e.dead = true;
        e.hp = 0.0;
        e.cast = None;
        e.auras.clear();
        e.target = None;
        e.moving = false;
        e.in_combat = false;
        let pos = e.pos;
        let level = e.level;
        let mut fighters = Vec::new();
        let mut mob_kind = None;
        match &mut e.brain {
            Brain::Player(p) => {
                p.auto_attack = false;
                p.combo_points = 0;
            }
            Brain::Mob(m) => {
                m.respawn_timer = m.kind.template().respawn;
                m.state = MobState::Idle;
                fighters = m.threat.drain(..).map(|(id, _)| id).collect();
                mob_kind = Some(m.kind);
            }
            Brain::Npc(_) => {}
        }
        self.send(Audience::Near(pos), GameEvent::Died { id: victim, killer });
        self.forget(victim);
        let Some(kind) = mob_kind else { return };
        let t = kind.template();
        if t.boss {
            self.open_portal(pos);
        }
        // Everyone who fought the mob, and their party members nearby, share
        // the kill and may loot it.
        let fighters: Vec<EntityId> = fighters
            .into_iter()
            .filter(|id| self.entities.get(id).is_some_and(|e| e.player().is_some()))
            .collect();
        let players = self.kill_credit(&fighters, pos);
        let loot = self.roll_loot(kind, level, players.clone());
        mob_of(&mut self.entities.get_mut(&victim).unwrap().brain).loot = loot;
        for &player in &players {
            self.quest_kill(player, kind);
        }
        for player in players {
            let xp = kill_xp(self.entities[&player].level, level, t.elite);
            if xp > 0 {
                self.give_xp(player, xp, t.name);
            }
        }
    }

    pub fn give_xp(&mut self, id: EntityId, amount: u32, from: &str) {
        self.send(
            Audience::Only(id),
            GameEvent::Xp {
                amount,
                from: from.to_string(),
            },
        );
        let e = self.entities.get_mut(&id).unwrap();
        let Brain::Player(p) = &mut e.brain else {
            return;
        };
        p.xp += amount;
        let class = p.class;
        let old_level = e.level;
        while e.level < MAX_LEVEL && p.xp >= xp_to_next(e.level) {
            p.xp -= xp_to_next(e.level);
            e.level += 1;
        }
        if e.level >= MAX_LEVEL {
            p.xp = 0;
        }
        let level = e.level;
        if level == old_level {
            return;
        }
        e.max_power = class.max_power(level);
        if class.power_kind() != PowerKind::Rage {
            e.power = e.max_power;
        }
        let pos = e.pos;
        self.refresh_stats(id);
        let e = self.entities.get_mut(&id).unwrap();
        e.hp = e.max_hp;
        self.send(Audience::Near(pos), GameEvent::LevelUp { id, level });
        for (ability, unlock) in class.spellbook() {
            if unlock > old_level && unlock <= level {
                self.send(Audience::Only(id), GameEvent::Learned(ability));
                // A rearranged hotbar gets new spells in its first free slot.
                let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
                if let Some(bar) = &mut p.hotbar
                    && !bar.contains(&Some(ability))
                    && let Some(slot) = bar.iter_mut().find(|s| s.is_none())
                {
                    *slot = Some(ability);
                }
            }
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.tick += 1;
        self.tick_parties(dt);
        self.tick_instances(dt);
        self.tick_hazards(dt);
        self.tick_missiles(dt);
        self.tick_ground(dt);
        let ids: Vec<EntityId> = self.entities.keys().copied().collect();
        for &id in &ids {
            self.tick_timers(id, dt);
            self.tick_auras(id, dt);
            self.tick_cast(id, dt);
            match self.entities.get(&id).map(|e| &e.brain) {
                Some(Brain::Player(_)) => self.tick_player(id, dt),
                Some(Brain::Mob(_)) => self.tick_mob(id, dt),
                Some(Brain::Npc(_)) => self.tick_npc(id),
                None => {}
            }
        }
        // Players are in combat while any mob wants to hit them.
        let mut fighting = HashSet::new();
        for e in self.entities.values() {
            if let Some(m) = e.mob().filter(|m| !e.dead && m.state == MobState::Combat) {
                fighting.extend(m.threat.iter().map(|(id, _)| *id));
            }
        }
        for e in self.entities.values_mut() {
            if e.player().is_some() {
                e.in_combat = !e.dead && fighting.contains(&e.id);
            }
        }
    }

    fn tick_timers(&mut self, id: EntityId, dt: f32) {
        let Some(e) = self.entities.get_mut(&id) else {
            return;
        };
        e.gcd = (e.gcd - dt).max(0.0);
        e.swing_timer = (e.swing_timer - dt).max(0.0);
        // Emotes play out; dying or casting ends them, and sitting also
        // ends when you move or fight.
        if let Some((emote, elapsed)) = &mut e.emote {
            *elapsed += dt;
            let over = emote.duration().is_some_and(|d| *elapsed >= d);
            let busy = e.dead || e.cast.is_some();
            let stand_up = *emote == Emote::Sit
                && (e.moving || e.in_combat || e.player().is_some_and(|p| p.auto_attack));
            if over || busy || stand_up {
                e.emote = None;
            }
        }
        e.cooldowns.retain(|_, (r, _)| {
            *r -= dt;
            *r > 0.0
        });
    }

    fn tick_auras(&mut self, id: EntityId, dt: f32) {
        let Some(e) = self.entities.get_mut(&id) else {
            return;
        };
        if e.dead {
            return;
        }
        let mut hits = Vec::new();
        for a in &mut e.auras {
            a.remaining -= dt;
            if let AuraKind::Dot { per_tick, interval } | AuraKind::Hot { per_tick, interval } =
                a.kind
            {
                a.tick_timer -= dt;
                if a.tick_timer <= 0.0 {
                    a.tick_timer += interval;
                    let amount = per_tick * a.stacks as f32;
                    let hit = if matches!(a.kind, AuraKind::Dot { .. }) {
                        Hit::Damage(amount, false)
                    } else {
                        Hit::Heal(amount, false)
                    };
                    hits.push((a.source, hit, a.ability));
                }
            }
        }
        e.auras.retain(|a| a.remaining > 0.0);
        for (source, hit, ability) in hits {
            self.apply_hit(source, id, hit, Some(ability));
        }
    }

    fn tick_cast(&mut self, id: EntityId, dt: f32) {
        let Some(e) = self.entities.get_mut(&id) else {
            return;
        };
        let Some(cast) = &mut e.cast else { return };
        cast.elapsed += dt;
        if cast.elapsed < cast.total {
            return;
        }
        let Cast {
            ability: aid,
            target,
            ..
        } = e.cast.take().unwrap();
        let a = ability(aid);
        // The target may have died or run off while we were casting.
        let valid = match a.targeting {
            Targeting::Enemy | Targeting::Friendly => {
                target.and_then(|t| self.entities.get(&t)).is_some_and(|t| {
                    !t.dead && t.pos.distance(self.entities[&id].pos) <= a.range + 3.0 + t.reach()
                })
            }
            _ => true,
        };
        if valid {
            self.complete(id, aid, target);
        } else {
            self.error(id, "Your target is no longer valid.");
        }
    }

    fn tick_player(&mut self, id: EntityId, dt: f32) {
        let e = self.entities.get_mut(&id).unwrap();
        let speed = RUN_SPEED * e.move_mult();
        let in_combat = e.in_combat;
        let regen = 1.0 + e.bonuses().map_or(0.0, |b| b.regen);
        let p = e.player_mut().unwrap();
        p.move_budget = (p.move_budget + speed * 1.25 * dt).min(speed);
        p.since_spend += dt;
        p.potion_cooldown = (p.potion_cooldown - dt).max(0.0);
        let since_spend = p.since_spend;
        let class = p.class;
        if e.dead {
            return;
        }
        // Regeneration.
        if !in_combat {
            e.hp = (e.hp + e.max_hp * 0.04 * dt).min(e.max_hp);
        }
        match class.power_kind() {
            PowerKind::Mana => {
                let rate = if since_spend >= MANA_REGEN_DELAY {
                    0.04
                } else {
                    0.006
                };
                e.power = (e.power + e.max_power * rate * regen * dt).min(e.max_power);
            }
            PowerKind::Rage => {
                if !in_combat {
                    // Talents slow the drain.
                    e.power = (e.power - 4.0 / regen * dt).max(0.0);
                }
            }
            PowerKind::Energy => {
                e.power = (e.power + ENERGY_PER_SECOND * regen * dt).min(e.max_power);
            }
        }
        // Auto attack.
        if !e.player().unwrap().auto_attack {
            return;
        }
        let target = match self.hostile_target(id) {
            Ok(t) => t,
            Err(_) => {
                self.entities
                    .get_mut(&id)
                    .unwrap()
                    .player_mut()
                    .unwrap()
                    .auto_attack = false;
                return;
            }
        };
        let e = &self.entities[&id];
        let t = &self.entities[&target];
        let aa = class.auto_attack();
        let ready = e.swing_timer <= 0.0 && e.cast.is_none() && !e.stunned();
        let in_range = e.pos.distance(t.pos) <= aa.range + 1.0 + t.reach();
        if ready && in_range && is_facing(e.pos, e.yaw, t.pos) {
            let bonus = e.bonuses().cloned().unwrap_or_default();
            let scale = level_scale(e.level) * e.power_mult() * (1.0 + bonus.damage);
            let weapon = e.player().map_or(0.0, |p| p.stats.damage);
            let (amount, crit) =
                self.roll_with(aa.min + weapon, aa.max + weapon, scale, bonus.crit);
            self.entities.get_mut(&id).unwrap().swing_timer = aa.interval;
            self.apply_hit(id, target, Hit::Damage(amount, crit), None);
        }
    }

    /// Merchants turn to face the nearest player close by.
    fn tick_npc(&mut self, id: EntityId) {
        let me = self.entities[&id].pos;
        let nearest = self
            .entities
            .values()
            .filter(|e| e.player().is_some() && e.pos.distance(me) < MERCHANT_RANGE + 2.0)
            .min_by(|a, b| a.pos.distance(me).total_cmp(&b.pos.distance(me)))
            .map(|e| e.pos);
        let e = self.entities.get_mut(&id).unwrap();
        let Brain::Npc(n) = &e.brain else { return };
        e.yaw = match nearest {
            Some(p) => yaw_towards(me, p),
            None => n.home_yaw,
        };
    }

    fn tick_mob(&mut self, id: EntityId, dt: f32) {
        let Some(e) = self.entities.get_mut(&id) else {
            return;
        };
        let m = mob_of(&mut e.brain);
        let t = m.kind.template();
        if e.dead {
            // Vault mobs stay dead until their copy closes.
            if self.camps[m.camp].instance.is_some() {
                return;
            }
            m.respawn_timer -= dt;
            if m.respawn_timer <= 0.0 && m.summoner.is_some() {
                self.entities.remove(&id);
                self.forget(id);
                return;
            }
            if m.respawn_timer <= 0.0 {
                let mut mob = self.entities.remove(&id).unwrap();
                self.reset_mob(&mut mob);
                self.entities.insert(id, mob);
            }
            return;
        }
        m.spell_timer -= dt;
        match m.state {
            MobState::Idle => {
                e.in_combat = false;
                e.target = None;
                e.hp = (e.hp + e.max_hp * 0.1 * dt).min(e.max_hp);
                self.mob_idle(id, dt);
            }
            MobState::Evading => {
                let home = m.home;
                e.hp = e.max_hp;
                e.target = None;
                e.cast = None;
                let step = t.speed * 1.6 * dt;
                if move_towards(e, home, step) {
                    e.auras.clear();
                    mob_of(&mut e.brain).state = MobState::Idle;
                    e.in_combat = false;
                    e.moving = false;
                }
            }
            MobState::Combat => self.mob_combat(id, dt),
        }
    }

    fn mob_idle(&mut self, id: EntityId, dt: f32) {
        let e = &self.entities[&id];
        let m = e.mob().unwrap();
        let t = m.kind.template();
        if t.aggressive {
            // Higher-level mobs notice you from further away.
            let victim = self
                .entities
                .values()
                .filter(|p| p.player().is_some() && !p.dead && !in_town(p.pos))
                .filter(|p| {
                    let radius = (10.0 + (e.level as f32 - p.level as f32) * 1.5).clamp(5.0, 20.0);
                    p.pos.distance(e.pos) <= radius
                })
                .min_by(|a, b| a.pos.distance(e.pos).total_cmp(&b.pos.distance(e.pos)))
                .map(|p| p.id);
            if let Some(victim) = victim {
                self.provoke(victim, id, 1.0);
                return;
            }
        }
        let e = self.entities.get_mut(&id).unwrap();
        let rooted = e.rooted();
        let slow = e.move_mult();
        let m = mob_of(&mut e.brain);
        let speed = m.kind.template().speed * 0.35;
        let radius = self.camps[m.camp].radius * 0.5;
        if let Some(to) = m.wander_to {
            if move_towards(e, to, speed * slow * dt) {
                mob_of(&mut e.brain).wander_to = None;
                e.moving = false;
            }
        } else {
            m.wander_timer -= dt;
            e.moving = false;
            if m.wander_timer <= 0.0 && !rooted {
                m.wander_timer = self.rng.range(4.0, 12.0);
                let angle = self.rng.range(0.0, std::f32::consts::TAU);
                let dist = self.rng.range(0.0, radius);
                let to = m.home + Vec3::new(angle.cos(), 0.0, angle.sin()) * dist;
                m.wander_to = Some(ground(to.x, to.z));
            }
        }
    }

    fn mob_combat(&mut self, id: EntityId, dt: f32) {
        let e = self.entities.get_mut(&id).unwrap();
        e.in_combat = true;
        mob_of(&mut e.brain).wander_to = None;
        // Drop anyone who is gone, dead or back in town.
        let alive: Vec<EntityId> = {
            let ents = &self.entities;
            ents[&id]
                .mob()
                .unwrap()
                .threat
                .iter()
                .map(|(t, _)| *t)
                .filter(|t| ents.get(t).is_some_and(|p| !p.dead && !in_town(p.pos)))
                .collect()
        };
        let e = self.entities.get_mut(&id).unwrap();
        let m = mob_of(&mut e.brain);
        m.threat.retain(|(t, _)| alive.contains(t));
        let home = m.home;
        if m.threat.is_empty() || e.pos.distance(home) > LEASH_RANGE {
            m.state = MobState::Evading;
            m.threat.clear();
            m.boss = BossTimers::default();
            let boss = m.kind.template().boss;
            e.cast = None;
            e.hp = e.max_hp;
            // A boss starts over: its rage cools and its pack goes.
            if boss {
                e.auras.clear();
                self.dismiss_pack(id);
            }
            return;
        }
        let target = m
            .threat
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(t, _)| *t)
            .unwrap();
        let kind = m.kind;
        let spell_ready = m.spell_timer <= 0.0;
        e.target = Some(target);
        if e.stunned() || e.cast.is_some() {
            e.moving = false;
            return;
        }
        let t = kind.template();
        let target_pos = self.pos_of(target);
        let e = self.entities.get_mut(&id).unwrap();
        e.yaw = yaw_towards(e.pos, target_pos);
        let dist = e.pos.distance(target_pos);

        if t.boss && self.boss_combat(id, dt) {
            self.entities.get_mut(&id).unwrap().moving = false;
            return;
        }
        let e = self.entities.get_mut(&id).unwrap();
        // Spiders web on a hit (below) rather than on a timer.
        if let Some((spell, interval)) = t.spell.filter(|(s, _)| *s != ids::WEB) {
            let a = ability(spell);
            let in_range = match a.targeting {
                Targeting::AroundCaster(r) => dist <= r * 0.8 + t.size * 0.5,
                _ => dist <= a.range,
            };
            if spell_ready && in_range {
                mob_of(&mut e.brain).spell_timer = interval;
                e.moving = false;
                if self.try_use(id, spell).is_ok() {
                    return;
                }
            }
        }

        let e = self.entities.get_mut(&id).unwrap();
        let reach = MELEE_RANGE * 0.7 + t.size * 0.5;
        if dist > reach && !e.rooted() {
            let step = t.speed * e.move_mult() * dt;
            let to = target_pos - (target_pos - e.pos).normalize_or_zero() * (reach * 0.8);
            move_towards(e, to, step);
            e.yaw = yaw_towards(e.pos, target_pos);
        } else {
            e.moving = false;
        }
        let dist = e.pos.distance(target_pos);
        if dist <= MELEE_RANGE + t.size * 0.5 + 0.5 && e.swing_timer <= 0.0 {
            e.swing_timer = t.attack_interval;
            let scale = level_scale(e.level);
            let crit = self.rng.chance(CRIT_CHANCE * 0.5);
            let amount = self.rng.range(t.damage.0, t.damage.1)
                * scale
                * if crit { CRIT_MULTIPLIER } else { 1.0 };
            self.apply_hit(id, target, Hit::Damage(amount, crit), None);
            let webs = t.spell.is_some_and(|(s, _)| s == ids::WEB);
            if webs
                && self.entities.get(&target).is_some_and(|t| !t.dead)
                && self.rng.chance(WEB_CHANCE)
            {
                let _ = self.try_use(id, ids::WEB);
            }
        }
    }
}

fn is_gear(id: ItemId) -> bool {
    matches!(
        item(id).kind,
        ItemKind::Armor { .. } | ItemKind::Weapon { .. }
    )
}

fn vec3_on_ground(p: Vec3) -> Vec3 {
    let floor = terrain_height(p.x, p.z);
    Vec3::new(p.x, p.y.max(floor).min(floor + 1.0), p.z)
}

fn add_threat(m: &mut MobData, who: EntityId, amount: f32) {
    match m.threat.iter_mut().find(|(id, _)| *id == who) {
        Some((_, v)) => *v += amount,
        None => m.threat.push((who, amount)),
    }
}

/// Walks an entity along the ground towards `to`. Returns true on arrival.
fn move_towards(e: &mut Entity, to: Vec3, step: f32) -> bool {
    let delta = Vec3::new(to.x - e.pos.x, 0.0, to.z - e.pos.z);
    let dist = delta.length();
    if dist <= step.max(0.05) {
        e.pos = ground(to.x, to.z);
        return true;
    }
    let dir = delta / dist;
    let next = e.pos + dir * step;
    e.pos = ground(next.x, next.z);
    e.yaw = dir.x.atan2(dir.z);
    e.moving = true;
    false
}

/// Used by the tests and the network layer's tests.
pub fn test_character(name: &str, class: Class, level: u8) -> Character {
    let mut c = Character::new("test", name, class, Appearance::default());
    c.level = level;
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::data::{ids, items};
    use shared::dungeon::DungeonId;

    const DT: f32 = 0.05;

    fn run(world: &mut World, seconds: f32) {
        for _ in 0..(seconds / DT) as usize {
            world.tick(DT);
        }
    }

    fn join(w: &mut World, name: &str, class: Class, level: u8) -> EntityId {
        w.add_player(&test_character(name, class, level))
    }

    /// The nearest living mob of a kind; moves the player next to it, facing it.
    fn engage(world: &mut World, player: EntityId, kind: MobKind) -> EntityId {
        let mob = world
            .entities
            .values()
            .filter(|e| e.mob().is_some_and(|m| m.kind == kind) && !e.dead)
            .min_by_key(|e| e.level)
            .unwrap()
            .id;
        let mpos = world.entities[&mob].pos;
        let p = world.entities.get_mut(&player).unwrap();
        p.pos = ground(mpos.x - 2.5, mpos.z);
        p.yaw = yaw_towards(p.pos, mpos);
        p.target = Some(mob);
        mob
    }

    /// Gives a mob enough health to survive a test.
    fn sturdy(w: &mut World, mob: EntityId) {
        let m = w.entities.get_mut(&mob).unwrap();
        m.max_hp = 100_000.0;
        m.hp = m.max_hp;
    }

    fn face(w: &mut World, p: EntityId, at: EntityId) {
        let to = w.entities[&at].pos;
        let e = w.entities.get_mut(&p).unwrap();
        e.yaw = yaw_towards(e.pos, to);
    }

    #[test]
    fn camps_are_on_dry_land() {
        let w = World::new(1);
        // Water mobs aside: they live in the shallows.
        let dry = |e: &&Entity| {
            e.mob()
                .is_some_and(|m| MobKind::water(Zone::at(e.pos)) != Some(m.kind))
        };
        for e in w.entities.values().filter(dry) {
            assert!(
                e.pos.y > WATER_LEVEL,
                "{} at {:?} is under water",
                e.name,
                e.pos
            );
        }
    }

    #[test]
    fn characters_round_trip_through_the_world() {
        let mut w = World::new(1);
        let mut c = test_character("Keeper", Class::Rogue, 4);
        c.money = 1234;
        c.bags[3] = Some((items::LIGHT_LEATHER, 7));
        c.gear[Slot::Chest.index()] = Some(items::LEATHER_VEST);
        c.pos = [60.0, 0.0, 10.0];
        let id = w.add_player(&c);
        assert_eq!(
            w.entities[&id].max_hp,
            Class::Rogue.max_hp(4) + 4.0 * HP_PER_STAMINA
        );
        let back = w.remove_player(id).unwrap();
        assert_eq!(back.money, 1234);
        assert_eq!(back.level, 4);
        assert_eq!(back.bags[3], Some((items::LIGHT_LEATHER, 7)));
        assert_eq!(back.gear, c.gear);
        assert_eq!(back.pos[0], 60.0);
        assert!(!w.entities.contains_key(&id));
    }

    #[test]
    fn abilities_unlock_with_levels() {
        let mut w = World::new(2);
        let p = join(&mut w, "Novice", Class::Barbarian, 1);
        engage(&mut w, p, MobKind::Boar);
        w.entities.get_mut(&p).unwrap().power = 100.0;
        assert_eq!(
            w.try_use(p, ids::REND),
            Err("You haven't learned that yet.")
        );
        w.try_use(p, ids::HEROIC_STRIKE).unwrap();
        w.drain_outbox();
        w.give_xp(p, xp_to_next(1), "test");
        assert_eq!(w.entities[&p].level, 2);
        let learned: Vec<_> = w
            .drain_outbox()
            .into_iter()
            .filter_map(|(_, m)| match m {
                ServerMsg::Event(GameEvent::Learned(a)) => Some(a),
                _ => None,
            })
            .collect();
        assert_eq!(learned, vec![ids::REND]);
        run(&mut w, GCD + 0.1);
        w.entities.get_mut(&p).unwrap().power = 100.0;
        assert_eq!(w.try_use(p, ids::REND), Ok(()));
    }

    #[test]
    fn warrior_kills_a_boar_and_gains_xp_and_loot() {
        let mut w = World::new(7);
        let p = join(&mut w, "Tank", Class::Barbarian, 1);
        let boar = engage(&mut w, p, MobKind::Boar);
        w.handle(p, ClientMsg::StartAttack);
        for _ in 0..600 {
            if w.entities[&boar].dead {
                break;
            }
            let _ = w.try_use(p, ids::HEROIC_STRIKE);
            // Stay next to the boar as it moves.
            let bpos = w.entities[&boar].pos;
            let e = w.entities.get_mut(&p).unwrap();
            e.pos = ground(bpos.x - 2.0, bpos.z);
            e.yaw = yaw_towards(e.pos, bpos);
            w.tick(DT);
        }
        assert!(w.entities[&boar].dead, "boar should die");
        assert!(
            !w.entities[&p].dead,
            "warrior should survive a level 1 boar"
        );
        assert!(w.entities[&p].player().unwrap().xp > 0);
        assert!(
            w.snapshot_for(p)
                .unwrap()
                .entities
                .iter()
                .any(|e| e.id == boar && e.lootable)
        );

        // Too far away, then close enough.
        w.entities.get_mut(&p).unwrap().pos.x += 30.0;
        w.handle(p, ClientMsg::Loot(boar));
        assert!(w.entities[&boar].mob().unwrap().loot.is_some());
        w.entities.get_mut(&p).unwrap().pos.x -= 30.0;
        w.handle(p, ClientMsg::Loot(boar));
        assert!(w.entities[&boar].mob().unwrap().loot.is_none());
        let pd = w.entities[&p].player().unwrap();
        assert!(pd.money > 0);
        let outbox = w.drain_outbox();
        assert!(
            outbox
                .iter()
                .any(|(_, m)| matches!(m, ServerMsg::Event(GameEvent::Looted { .. })))
        );
        assert!(
            outbox
                .iter()
                .any(|(_, m)| matches!(m, ServerMsg::Event(GameEvent::Died { .. })))
        );
    }

    #[test]
    fn boars_drop_leather_that_makes_armor() {
        let mut w = World::new(8);
        let looters = vec![1];
        let mut leather = 0;
        for _ in 0..50 {
            let loot = w.roll_loot(MobKind::Boar, 2, looters.clone()).unwrap();
            leather += loot
                .items
                .iter()
                .filter(|(i, _)| *i == items::LIGHT_LEATHER)
                .map(|(_, n)| *n)
                .sum::<u16>();
        }
        assert!(
            leather >= 30,
            "boars should usually drop leather, got {leather} from 50"
        );

        let p = join(&mut w, "Crafter", Class::Rogue, 1);
        assert_eq!(w.craft(p, 1), Err("You don't have the materials."));
        let pd = w.entities.get_mut(&p).unwrap().player_mut().unwrap();
        add_item(&mut pd.bags, items::LIGHT_LEATHER, 7);
        w.craft(p, 1).unwrap();
        let pd = w.entities[&p].player().unwrap();
        assert_eq!(count_item(&pd.bags, items::LIGHT_LEATHER), 1);
        let vest_slot = pd
            .bags
            .iter()
            .position(|s| *s == Some((items::LEATHER_VEST, 1)))
            .unwrap();
        let hp = w.entities[&p].max_hp;
        w.handle(p, ClientMsg::Equip(vest_slot));
        let pd = w.entities[&p].player().unwrap();
        assert_eq!(pd.gear[Slot::Chest.index()], Some(items::LEATHER_VEST));
        assert_eq!(pd.stats.armor, 12.0);
        assert!(w.entities[&p].max_hp > hp);
        w.handle(p, ClientMsg::Unequip(Slot::Chest));
        assert_eq!(w.entities[&p].max_hp, hp);
    }

    #[test]
    fn armor_reduces_physical_damage() {
        let mut w = World::new(9);
        let naked = join(&mut w, "Naked", Class::Mage, 5);
        let mut c = test_character("Armored", Class::Mage, 5);
        c.gear = [
            Some(items::LEATHER_CAP),
            Some(items::LEATHER_VEST),
            Some(items::LEATHER_GLOVES),
            Some(items::LEATHER_PANTS),
            Some(items::LEATHER_BOOTS),
        ];
        let armored = w.add_player(&c);
        let wolf = engage(&mut w, naked, MobKind::Wolf);
        for who in [naked, armored] {
            let before = w.entities[&who].hp;
            w.apply_hit(wolf, who, Hit::Damage(20.0, false), None);
            let taken = before - w.entities[&who].hp;
            if who == naked {
                assert_eq!(taken, 20.0);
            } else {
                assert!(
                    taken < 17.0,
                    "armor should soak some of the hit, took {taken}"
                );
            }
        }
    }

    #[test]
    fn rend_stacks_three_times() {
        let mut w = World::new(10);
        let p = join(&mut w, "Bleeder", Class::Barbarian, 10);
        let boar = engage(&mut w, p, MobKind::Boar);
        sturdy(&mut w, boar);
        for _ in 0..5 {
            w.entities.get_mut(&p).unwrap().power = 100.0;
            face(&mut w, p, boar);
            w.try_use(p, ids::REND).unwrap();
            run(&mut w, GCD + 0.1);
        }
        let rend = w.entities[&boar]
            .auras
            .iter()
            .find(|a| a.ability == ids::REND)
            .unwrap();
        assert_eq!(rend.stacks, 3);
        assert!(
            w.snapshot_for(p)
                .unwrap()
                .entities
                .iter()
                .any(|e| e.auras.iter().any(|a| a.stacks == 3))
        );
    }

    #[test]
    fn fireball_leaves_a_burn() {
        let mut w = World::new(11);
        let p = join(&mut w, "Pyro", Class::Mage, 1);
        let boar = engage(&mut w, p, MobKind::Boar);
        w.try_use(p, ids::FIREBALL).unwrap();
        run(&mut w, 3.2);
        assert!(
            w.entities[&boar]
                .auras
                .iter()
                .any(|a| a.ability == ids::FIREBALL && a.kind.harmful())
        );
        let hp = w.entities[&boar].hp;
        run(&mut w, 2.1);
        assert!(w.entities[&boar].hp < hp, "the burn should tick");
    }

    #[test]
    fn rogues_build_and_spend_combo_points() {
        let mut w = World::new(12);
        let p = join(&mut w, "Shade", Class::Rogue, 10);
        let boar = engage(&mut w, p, MobKind::Boar);
        sturdy(&mut w, boar);
        assert_eq!(
            w.try_use(p, ids::EVISCERATE),
            Err("That ability requires combo points.")
        );
        for _ in 0..2 {
            w.entities.get_mut(&p).unwrap().power = 100.0;
            face(&mut w, p, boar);
            w.try_use(p, ids::SINISTER_STRIKE).unwrap();
            run(&mut w, GCD + 0.1);
        }
        assert_eq!(w.entities[&p].player().unwrap().combo_points, 2);
        w.entities.get_mut(&p).unwrap().power = 100.0;
        face(&mut w, p, boar);
        w.try_use(p, ids::EVISCERATE).unwrap();
        assert_eq!(w.entities[&p].player().unwrap().combo_points, 0);

        // Backstab only works from behind: the boar is facing the rogue.
        run(&mut w, GCD + 0.1);
        w.entities.get_mut(&p).unwrap().power = 100.0;
        face(&mut w, boar, p);
        face(&mut w, p, boar);
        assert_eq!(
            w.try_use(p, ids::BACKSTAB),
            Err("You must be behind your target.")
        );
        let bpos = w.entities[&boar].pos;
        let e = w.entities.get_mut(&boar).unwrap();
        e.yaw = yaw_towards(bpos, bpos + Vec3::X * 5.0);
        let e = w.entities.get_mut(&p).unwrap();
        e.pos = ground(bpos.x - 2.0, bpos.z);
        e.yaw = yaw_towards(e.pos, bpos);
        w.entities.get_mut(&boar).unwrap().auras.push(Aura {
            ability: ids::GOUGE,
            kind: AuraKind::Stun,
            duration: 5.0,
            remaining: 5.0,
            tick_timer: 0.0,
            source: p,
            stacks: 1,
        });
        assert_eq!(w.try_use(p, ids::BACKSTAB), Ok(()));
    }

    #[test]
    fn evasion_halves_damage() {
        let mut w = World::new(13);
        let p = join(&mut w, "Dodger", Class::Rogue, 10);
        let wolf = engage(&mut w, p, MobKind::Wolf);
        w.try_use(p, ids::EVASION).unwrap();
        let before = w.entities[&p].hp;
        w.apply_hit(wolf, p, Hit::Damage(20.0, false), None);
        assert_eq!(before - w.entities[&p].hp, 10.0);
    }

    #[test]
    fn attacked_mobs_turn_on_their_attacker() {
        // Every opening attack, from range or melee, pulls the mob onto you.
        for (class, ability_id, range) in [
            (Class::Mage, ids::FIREBALL, 25.0),
            (Class::Mage, ids::FIRE_BLAST, 18.0),
            (Class::Cleric, ids::SMITE, 25.0),
            (Class::Cleric, ids::SHADOW_WORD_PAIN, 25.0),
            (Class::Barbarian, ids::HEROIC_STRIKE, 2.5),
            (Class::Rogue, ids::SINISTER_STRIKE, 2.5),
        ] {
            let mut w = World::new(14);
            let p = join(&mut w, "Puller", class, 10);
            let boar = engage(&mut w, p, MobKind::Boar);
            w.entities.retain(|id, e| *id == boar || e.mob().is_none());
            sturdy(&mut w, boar);
            let bpos = w.entities[&boar].pos;
            let e = w.entities.get_mut(&p).unwrap();
            e.pos = ground(bpos.x - range, bpos.z);
            e.power = e.max_power;
            face(&mut w, p, boar);
            w.try_use(p, ability_id).unwrap();
            run(&mut w, 4.0);
            let b = &w.entities[&boar];
            assert_eq!(
                b.mob().unwrap().state,
                MobState::Combat,
                "{class:?} {}",
                ability(ability_id).name
            );
            assert_eq!(b.target, Some(p));
            let close = b.pos.distance(w.entities[&p].pos);
            assert!(
                close < range.max(MELEE_RANGE + 1.0),
                "{class:?}: the boar should come at the attacker"
            );
        }
    }

    #[test]
    fn neutral_mobs_ignore_passers_by_and_wolves_do_not() {
        let mut w = World::new(3);
        let p = join(&mut w, "Walker", Class::Mage, 1);
        let boar = engage(&mut w, p, MobKind::Boar);
        run(&mut w, 1.0);
        assert_eq!(w.entities[&boar].mob().unwrap().state, MobState::Idle);
        let wolf = engage(&mut w, p, MobKind::Wolf);
        run(&mut w, 0.2);
        assert_eq!(w.entities[&wolf].mob().unwrap().state, MobState::Combat);
        assert!(w.entities[&p].in_combat);
    }

    #[test]
    fn only_people_call_for_help() {
        let fighting = |w: &World| {
            w.entities
                .values()
                .filter(|e| e.mob().is_some_and(|m| m.state == MobState::Combat))
                .count()
        };
        let mut w = World::new(29);
        let p = join(&mut w, "Puller", Class::Mage, 1);
        let wolf = engage(&mut w, p, MobKind::Wolf);
        w.provoke(p, wolf, 1.0);
        assert_eq!(fighting(&w), 1);

        let mut w = World::new(29);
        let p = join(&mut w, "Puller", Class::Mage, 1);
        let bandit = w
            .entities
            .values()
            .filter(|e| e.mob().is_some_and(|m| m.kind == MobKind::Bandit))
            .find(|b| {
                w.entities.values().any(|o| {
                    o.id != b.id
                        && o.mob().is_some_and(|m| m.camp == b.mob().unwrap().camp)
                        && o.pos.distance(b.pos) <= 8.0
                })
            })
            .map(|e| e.id);
        if let Some(bandit) = bandit {
            w.provoke(p, bandit, 1.0);
            assert!(fighting(&w) > 1);
        }
    }

    /// Plays one class against one mob of its level with a simple rotation.
    fn solo(class: Class, kind: MobKind, rotation: &[AbilityId]) -> (bool, bool) {
        let mut w = World::new(31);
        let p = join(&mut w, "Solo", class, 1);
        let mob = w
            .entities
            .values()
            .filter(|e| e.mob().is_some_and(|m| m.kind == kind))
            .min_by(|a, b| {
                a.pos
                    .distance(Vec3::ZERO)
                    .total_cmp(&b.pos.distance(Vec3::ZERO))
            })
            .unwrap()
            .id;
        let m = w.entities.get_mut(&mob).unwrap();
        m.level = 1;
        m.max_hp = kind.max_hp(1);
        m.hp = m.max_hp;
        let mpos = m.pos;
        // A careful player pulls one mob at a time.
        w.entities
            .retain(|id, e| *id == mob || e.mob().is_none() || e.pos.distance(mpos) > 40.0);
        let range = class.auto_attack().range - 2.0;
        w.entities.get_mut(&p).unwrap().pos = ground(mpos.x - range, mpos.z);
        w.entities.get_mut(&p).unwrap().target = Some(mob);
        w.handle(p, ClientMsg::StartAttack);
        let melee = class.is_melee();
        for _ in 0..(90.0 / DT) as usize {
            if w.entities[&mob].dead || w.entities[&p].dead {
                break;
            }
            let mpos = w.entities[&mob].pos;
            let e = w.entities.get_mut(&p).unwrap();
            if melee {
                e.pos = ground(mpos.x - 2.0, mpos.z);
            }
            e.yaw = yaw_towards(e.pos, mpos);
            for &a in rotation {
                if w.try_use(p, a).is_ok() {
                    break;
                }
            }
            w.tick(DT);
        }
        (w.entities[&mob].dead, !w.entities[&p].dead)
    }

    #[test]
    fn every_class_can_solo_a_mob_of_its_level_with_its_first_ability() {
        for kind in [MobKind::Wolf, MobKind::Boar] {
            for class in Class::ALL {
                let first = class.abilities()[0];
                assert_eq!(
                    solo(class, kind, &[first]),
                    (true, true),
                    "{class:?} vs {kind:?}"
                );
            }
        }
    }

    #[test]
    fn casts_take_time_and_moving_interrupts() {
        let mut w = World::new(5);
        let p = join(&mut w, "Caster", Class::Mage, 10);
        let boar = engage(&mut w, p, MobKind::Boar);
        w.try_use(p, ids::FIREBALL).unwrap();
        assert!(w.entities[&p].cast.is_some());
        assert_eq!(
            w.try_use(p, ids::FIRE_BLAST),
            Err("You are already casting.")
        );
        let hp = w.entities[&boar].hp;
        run(&mut w, 1.0);
        assert_eq!(w.entities[&boar].hp, hp);
        // Step away: the cast stops.
        let pos = w.entities[&p].pos + Vec3::new(0.5, 0.0, 0.0);
        w.tick(DT);
        w.handle(
            p,
            ClientMsg::Move {
                pos,
                yaw: 0.0,
                moving: true,
            },
        );
        assert!(w.entities[&p].cast.is_none());
        // Stand still and let one finish.
        run(&mut w, 1.6);
        face(&mut w, p, boar);
        w.try_use(p, ids::FIREBALL).unwrap();
        run(&mut w, 3.2);
        assert!(w.entities[&boar].hp < hp);
        assert!(w.entities[&boar].mob().unwrap().state == MobState::Combat);
    }

    #[test]
    fn range_facing_and_cooldowns_are_checked() {
        let mut w = World::new(9);
        let p = join(&mut w, "Checker", Class::Barbarian, 10);
        assert_eq!(w.try_use(p, ids::HEROIC_STRIKE), Err("You have no target."));
        let boar = engage(&mut w, p, MobKind::Boar);
        w.entities.get_mut(&p).unwrap().power = 100.0;
        let bpos = w.entities[&boar].pos;
        w.entities.get_mut(&p).unwrap().pos = ground(bpos.x - 20.0, bpos.z);
        assert_eq!(w.try_use(p, ids::HEROIC_STRIKE), Err("Out of range."));
        let e = w.entities.get_mut(&p).unwrap();
        e.pos = ground(bpos.x - 2.0, bpos.z);
        e.yaw = yaw_towards(bpos, e.pos);
        assert_eq!(
            w.try_use(p, ids::HEROIC_STRIKE),
            Err("You are facing the wrong way!")
        );
        face(&mut w, p, boar);
        w.try_use(p, ids::SKULL_BASH).unwrap();
        assert_eq!(
            w.try_use(p, ids::HEROIC_STRIKE),
            Err("Ability is not ready yet.")
        );
        run(&mut w, GCD + 0.1);
        assert_eq!(
            w.try_use(p, ids::SKULL_BASH),
            Err("Ability is not ready yet.")
        );
        assert!(w.entities[&boar].stunned() || w.entities[&boar].dead);
        assert_eq!(
            w.try_use(p, ids::FIREBALL),
            Err("You don't know that ability.")
        );
    }

    #[test]
    fn mobs_leash_and_heal_up() {
        let mut w = World::new(11);
        let p = join(&mut w, "Kiter", Class::Mage, 1);
        let wolf = engage(&mut w, p, MobKind::Wolf);
        run(&mut w, 0.2);
        assert_eq!(w.entities[&wolf].mob().unwrap().state, MobState::Combat);
        w.entities.get_mut(&wolf).unwrap().hp = 1.0;
        let home = w.entities[&wolf].mob().unwrap().home;
        let far = ground(home.x + LEASH_RANGE + 5.0, home.z);
        w.entities.get_mut(&wolf).unwrap().pos = far;
        w.entities.get_mut(&p).unwrap().pos = ground(far.x + 2.0, far.z);
        w.tick(DT);
        assert_eq!(w.entities[&wolf].mob().unwrap().state, MobState::Evading);
        assert_eq!(w.entities[&wolf].hp, w.entities[&wolf].max_hp);
        run(&mut w, 15.0);
        assert_eq!(w.entities[&wolf].mob().unwrap().state, MobState::Idle);
    }

    #[test]
    fn heals_shields_and_levels() {
        let mut w = World::new(13);
        let healer = join(&mut w, "Healer", Class::Cleric, 10);
        let tank = join(&mut w, "Tank", Class::Barbarian, 1);
        w.entities.get_mut(&tank).unwrap().hp = 10.0;
        w.handle(healer, ClientMsg::SetTarget(Some(tank)));
        w.try_use(healer, ids::HEAL).unwrap();
        run(&mut w, 2.6);
        assert!(w.entities[&tank].hp > 40.0);
        run(&mut w, 6.0);
        w.try_use(healer, ids::POWER_WORD_SHIELD).unwrap();
        assert!(
            w.entities[&tank]
                .auras
                .iter()
                .any(|a| matches!(a.kind, AuraKind::Absorb(_)))
        );
        run(&mut w, 2.0);
        w.handle(healer, ClientMsg::SetTarget(None));
        w.try_use(healer, ids::RENEW).unwrap();
        assert!(
            w.entities[&healer]
                .auras
                .iter()
                .any(|a| a.ability == ids::RENEW)
        );

        w.give_xp(tank, 1000, "test");
        let t = &w.entities[&tank];
        assert!(t.level >= 4);
        assert_eq!(t.max_hp, Class::Barbarian.max_hp(t.level));
    }

    #[test]
    fn speed_hacks_are_rejected() {
        let mut w = World::new(17);
        let p = join(&mut w, "Runner", Class::Mage, 1);
        run(&mut w, 1.0);
        let start = w.entities[&p].pos;
        w.handle(
            p,
            ClientMsg::Move {
                pos: start + Vec3::new(50.0, 0.0, 0.0),
                yaw: 0.0,
                moving: true,
            },
        );
        assert_eq!(w.entities[&p].pos, start);
        w.drain_outbox();
        w.handle(
            p,
            ClientMsg::Move {
                pos: start + Vec3::new(3.0, 0.0, 0.0),
                yaw: 0.0,
                moving: true,
            },
        );
        assert_ne!(w.entities[&p].pos, start);
    }

    #[test]
    fn death_and_release() {
        let mut w = World::new(19);
        let p = join(&mut w, "Unlucky", Class::Mage, 1);
        let golem = engage(&mut w, p, MobKind::Golem);
        w.handle(p, ClientMsg::StartAttack);
        run(&mut w, 60.0);
        assert!(w.entities[&p].dead);
        run(&mut w, 10.0);
        assert_eq!(w.entities[&golem].mob().unwrap().state, MobState::Idle);
        w.handle(p, ClientMsg::ReleaseSpirit);
        let e = &w.entities[&p];
        assert!(!e.dead);
        assert_eq!(flat_distance(e.pos, Zone::Amberfall.graveyard()), 0.0);
    }

    #[test]
    fn snapshots_only_include_nearby_entities() {
        let mut w = World::new(23);
        let p = join(&mut w, "Viewer", Class::Cleric, 1);
        let snap = w.snapshot_for(p).unwrap();
        assert!(snap.entities.iter().any(|e| e.id == p));
        assert!(snap.entities.len() < w.entities.len());
        assert!(
            snap.entities
                .iter()
                .all(|e| e.pos.distance(w.entities[&p].pos) <= VIEW_DISTANCE)
        );
    }

    #[test]
    fn every_zone_has_its_own_mobs_and_a_merchant() {
        let w = World::new(41);
        for zone in Zone::ALL {
            let kinds = MobKind::for_zone(zone);
            let mobs: Vec<_> = w
                .entities
                .values()
                .filter(|e| e.mob().is_some() && Zone::at(e.pos) == zone)
                .collect();
            assert!(mobs.len() >= 50, "{zone:?} has {} mobs", mobs.len());
            assert!(mobs.iter().all(|m| {
                let kind = m.mob().unwrap().kind;
                kinds.contains(&kind) || MobKind::water(zone) == Some(kind)
            }));
            let merchant = w
                .entities
                .values()
                .find(|e| matches!(e.brain, Brain::Npc(_)) && Zone::at(e.pos) == zone)
                .expect("merchant");
            assert!(zone.in_town(merchant.pos));
            assert_eq!(merchant.kind(), EntityKind::Merchant(zone.race()));
        }
    }

    #[test]
    fn races_start_in_their_own_zone() {
        let mut w = World::new(42);
        for race in Race::ALL {
            let c = Character::new(
                "t",
                "Newbie",
                Class::Monk,
                Appearance {
                    race,
                    ..Default::default()
                },
            );
            let id = w.add_player(&c);
            assert_eq!(Zone::at(w.entities[&id].pos), race.zone());
            assert!(race.zone().in_town(w.entities[&id].pos));
        }
    }

    #[test]
    fn charge_rushes_to_the_target() {
        let mut w = World::new(43);
        let p = join(&mut w, "Charger", Class::Barbarian, 3);
        let boar = engage(&mut w, p, MobKind::Boar);
        sturdy(&mut w, boar);
        assert_eq!(w.try_use(p, ids::CHARGE), Err("Too close."));
        let bpos = w.entities[&boar].pos;
        let e = w.entities.get_mut(&p).unwrap();
        e.pos = ground(bpos.x - 15.0, bpos.z);
        e.yaw = yaw_towards(e.pos, bpos);
        w.drain_outbox();
        w.try_use(p, ids::CHARGE).unwrap();
        let e = &w.entities[&p];
        assert!(e.pos.distance(bpos) < MELEE_RANGE, "charged to {:?}", e.pos);
        assert!((e.power - 15.0).abs() < 0.01);
        assert!(w.entities[&boar].stunned());
        assert!(
            w.drain_outbox()
                .iter()
                .any(|(to, m)| matches!(to, Audience::Only(id) if *id == p)
                    && matches!(m, ServerMsg::SetPosition { .. }))
        );
    }

    #[test]
    fn leaps_go_forward_and_back() {
        let mut w = World::new(44);
        let mage = join(&mut w, "Blinker", Class::Mage, 3);
        let start = w.entities[&mage].pos;
        w.entities.get_mut(&mage).unwrap().yaw = 0.0;
        w.try_use(mage, ids::BLINK).unwrap();
        let moved = w.entities[&mage].pos - start;
        assert!(
            (moved.z - 15.0).abs() < 0.01 && moved.x.abs() < 0.01,
            "{moved:?}"
        );

        let ranger = join(&mut w, "Leaper", Class::Ranger, 3);
        let start = w.entities[&ranger].pos;
        w.entities.get_mut(&ranger).unwrap().yaw = 0.0;
        w.try_use(ranger, ids::DISENGAGE).unwrap();
        assert!((w.entities[&ranger].pos.z - (start.z - 12.0)).abs() < 0.01);
    }

    #[test]
    fn multi_shot_hits_everything_around_the_target() {
        let mut w = World::new(45);
        let p = join(&mut w, "Archer", Class::Ranger, 6);
        let boar = engage(&mut w, p, MobKind::Boar);
        let bpos = w.entities[&boar].pos;
        let other = w
            .entities
            .values()
            .find(|e| e.id != boar && e.mob().is_some_and(|m| m.kind == MobKind::Boar))
            .unwrap()
            .id;
        let far = w
            .entities
            .values()
            .find(|e| e.mob().is_some_and(|m| m.kind == MobKind::Wolf))
            .unwrap()
            .id;
        w.entities.get_mut(&other).unwrap().pos = ground(bpos.x, bpos.z + 3.0);
        w.entities.get_mut(&far).unwrap().pos = ground(bpos.x, bpos.z + 20.0);
        for id in [boar, other, far] {
            sturdy(&mut w, id);
        }
        let e = w.entities.get_mut(&p).unwrap();
        e.pos = ground(bpos.x - 20.0, bpos.z);
        e.yaw = yaw_towards(e.pos, bpos);
        w.try_use(p, ids::MULTI_SHOT).unwrap();
        let hurt = |w: &World, id| w.entities[&id].hp < w.entities[&id].max_hp;
        // It lands when the arrows get there, not when they're loosed.
        assert!(!hurt(&w, boar));
        run(&mut w, 1.0);
        assert!(hurt(&w, boar) && hurt(&w, other));
        assert!(!hurt(&w, far));
    }

    #[test]
    fn drain_life_heals_the_warlock() {
        let mut w = World::new(46);
        let p = join(&mut w, "Leech", Class::Warlock, 4);
        let boar = engage(&mut w, p, MobKind::Boar);
        sturdy(&mut w, boar);
        w.entities.get_mut(&p).unwrap().hp = 20.0;
        w.try_use(p, ids::DRAIN_LIFE).unwrap();
        run(&mut w, 1.6);
        assert!(w.entities[&p].hp > 20.0 + 8.0);
        assert!(w.entities[&boar].hp < w.entities[&boar].max_hp);
    }

    #[test]
    fn damage_buffs_and_curses() {
        let mut w = World::new(47);
        let p = join(&mut w, "Berserk", Class::Barbarian, 10);
        let boar = engage(&mut w, p, MobKind::Boar);
        sturdy(&mut w, boar);
        let hit = |w: &mut World, from, to| {
            let before = w.entities[&to].hp;
            w.apply_hit(from, to, Hit::Damage(20.0, false), Some(ids::FIREBALL));
            before - w.entities[&to].hp
        };
        assert_eq!(hit(&mut w, p, boar), 20.0);
        w.try_use(p, ids::RECKLESSNESS).unwrap();
        assert_eq!(hit(&mut w, p, boar), 26.0);

        let warlock = join(&mut w, "Curser", Class::Warlock, 10);
        engage(&mut w, warlock, MobKind::Boar);
        w.entities.get_mut(&warlock).unwrap().target = Some(boar);
        face(&mut w, warlock, boar);
        w.try_use(warlock, ids::CURSE_OF_WEAKNESS).unwrap();
        assert_eq!(hit(&mut w, boar, warlock), 14.0);
    }

    #[test]
    fn merchants_sell_potions_and_buy_loot() {
        let mut w = World::new(48);
        let p = join(&mut w, "Shopper", Class::Paladin, 1);
        let merchant = w
            .entities
            .values()
            .find(|e| matches!(e.brain, Brain::Npc(_)) && Zone::at(e.pos) == Zone::Amberfall)
            .unwrap()
            .id;
        let mpos = w.entities[&merchant].pos;
        w.entities.get_mut(&p).unwrap().pos = mpos + Vec3::new(30.0, 0.0, 0.0);
        assert_eq!(
            w.buy(p, merchant, items::HEALING_POTION),
            Err("You are too far away.")
        );
        w.entities.get_mut(&p).unwrap().pos = mpos + Vec3::new(2.0, 0.0, 0.0);
        assert_eq!(
            w.buy(p, merchant, items::HEALING_POTION),
            Err("You don't have enough money.")
        );
        assert_eq!(
            w.buy(p, merchant, items::ANCIENT_CORE),
            Err("That isn't for sale.")
        );

        let pd = w.entities.get_mut(&p).unwrap().player_mut().unwrap();
        pd.money = 120;
        add_item(&mut pd.bags, items::LIGHT_LEATHER, 4);
        w.buy(p, merchant, items::HEALING_POTION).unwrap();
        let pd = w.entities[&p].player().unwrap();
        assert_eq!(pd.money, 70);
        assert_eq!(count_item(&pd.bags, items::HEALING_POTION), 1);
        let leather = pd
            .bags
            .iter()
            .position(|s| matches!(s, Some((i, _)) if *i == items::LIGHT_LEATHER))
            .unwrap();
        w.sell(p, merchant, leather).unwrap();
        assert_eq!(
            w.entities[&p].player().unwrap().money,
            70 + 4 * item(items::LIGHT_LEATHER).sell_price()
        );

        // Drinking it heals and starts the cooldown.
        w.entities.get_mut(&p).unwrap().hp = 10.0;
        let slot = w.entities[&p]
            .player()
            .unwrap()
            .bags
            .iter()
            .position(|s| matches!(s, Some((i, _)) if *i == items::HEALING_POTION))
            .unwrap();
        w.use_item(p, slot).unwrap();
        assert!(w.entities[&p].hp > 10.0 + 0.3 * w.entities[&p].max_hp);
        assert_eq!(
            count_item(
                &w.entities[&p].player().unwrap().bags,
                items::HEALING_POTION
            ),
            0
        );
        assert_eq!(w.use_item(p, slot), Err("That slot is empty."));

        // Merchants can't be attacked.
        let hp = w.entities[&merchant].hp;
        w.apply_hit(p, merchant, Hit::Damage(50.0, false), None);
        assert_eq!(w.entities[&merchant].hp, hp);
    }

    #[test]
    fn mobs_cast_player_spells_for_free() {
        let mut w = World::new(49);
        let p = join(&mut w, "Target", Class::Mage, 5);
        let shaman = w
            .entities
            .values()
            .find(|e| e.mob().is_some_and(|m| m.kind == MobKind::FrostTrollShaman))
            .unwrap()
            .id;
        let spos = w.entities[&shaman].pos;
        w.entities.get_mut(&p).unwrap().pos = ground(spos.x + 15.0, spos.z);
        w.entities.get_mut(&shaman).unwrap().target = Some(p);
        assert_eq!(w.try_use(shaman, ids::FROSTBOLT), Ok(()));
        run(&mut w, 2.8);
        assert!(
            w.entities[&p]
                .auras
                .iter()
                .any(|a| a.ability == ids::FROSTBOLT)
        );
    }

    #[test]
    fn hotbars_are_saved_and_new_spells_land_on_them() {
        let mut w = World::new(32);
        let p = join(&mut w, "Bars", Class::Mage, 11);
        assert!(w.character(p).unwrap().hotbar.is_none(), "default layout");
        // Move Fireball to Shift+E and clear Q.
        let mut bar = Class::Mage.default_hotbar();
        let fireball = bar[0];
        bar[15] = fireball;
        bar[0] = None;
        bar[6] = None;
        // Junk is dropped: another class's ability, and a repeat.
        bar[13] = Some(ids::SMITE);
        bar[14] = fireball;
        w.handle(p, ClientMsg::SetHotbar(bar));
        let saved = w.character(p).unwrap().hotbar.unwrap();
        assert_eq!(saved[15], None, "the later copy of Fireball is dropped");
        assert_eq!(saved[14], fireball);
        assert_eq!(saved[13], None);
        assert_eq!(saved[0], None);
        // Level 12's spell isn't on the bars any more; learning it puts it
        // in the first free slot.
        let new = Class::Mage.spells()[0];
        assert!(!saved.contains(&Some(new)));
        w.give_xp(p, xp_to_next(11), "test");
        assert_eq!(w.entities[&p].level, 12);
        assert_eq!(w.character(p).unwrap().hotbar.unwrap()[0], Some(new));
    }

    #[test]
    fn new_spells_are_learned_from_level_12() {
        for class in Class::ALL {
            let mut w = World::new(33);
            let p = join(&mut w, "Learner", class, 11);
            let first = class.spells()[0];
            assert!(w.try_use(p, first).is_err(), "{class:?} too early");
            w.entities.get_mut(&p).unwrap().level = MAX_LEVEL;
            for id in class.spells() {
                assert_ne!(
                    w.try_use(p, id),
                    Err("You don't know that ability."),
                    "{class:?}"
                );
                assert_ne!(
                    w.try_use(p, id),
                    Err("You haven't learned that yet."),
                    "{class:?}"
                );
            }
        }
    }

    #[test]
    fn talents_are_learned_saved_and_change_combat() {
        let mut w = World::new(31);
        let p = join(&mut w, "Tal", Class::Fighter, 10);
        let base_hp = w.entities[&p].max_hp;
        // Last Stand needs points in Toughness and Shield Mastery first.
        w.handle(p, ClientMsg::LearnTalent(5));
        assert_eq!(w.entities[&p].player().unwrap().talents[5], 0);
        for i in [3, 3, 3, 4, 4, 5] {
            w.handle(p, ClientMsg::LearnTalent(i));
        }
        let pd = w.entities[&p].player().unwrap();
        assert_eq!(pd.talents, [0, 0, 0, 3, 2, 1, 0, 0, 0]);
        assert!(w.entities[&p].max_hp > base_hp * 1.09);
        // Toughness: less damage taken.
        assert!((pd.bonuses.toughness - 0.09).abs() < 1e-5);
        let c = w.character(p).unwrap();
        assert_eq!(c.talents, [0, 0, 0, 3, 2, 1, 0, 0, 0]);
        // Points run out at nine.
        for _ in 0..5 {
            w.handle(p, ClientMsg::LearnTalent(0));
        }
        assert_eq!(talents::spent(&w.entities[&p].player().unwrap().talents), 9);
        w.handle(p, ClientMsg::ResetTalents);
        assert_eq!(talents::spent(&w.entities[&p].player().unwrap().talents), 0);
        assert!((w.entities[&p].max_hp - base_hp).abs() < 0.01);
    }

    #[test]
    fn talents_shorten_cooldowns_and_casts() {
        let mut w = World::new(32);
        let p = join(&mut w, "Ice", Class::Mage, 10);
        w.handle(p, ClientMsg::LearnTalent(3));
        w.handle(p, ClientMsg::LearnTalent(3));
        w.handle(p, ClientMsg::LearnTalent(4));
        let mob = engage(&mut w, p, MobKind::Wolf);
        sturdy(&mut w, mob);
        assert_eq!(w.try_use(p, ids::FROSTBOLT), Ok(()));
        let total = w.entities[&p].cast.as_ref().unwrap().total;
        assert!((total - (ability(ids::FROSTBOLT).cast_time - 0.25)).abs() < 1e-4);
    }

    #[test]
    fn sandbox_cheats_only_work_in_sandbox_mode() {
        let mut w = World::new(33);
        let p = join(&mut w, "Box", Class::Rogue, 1);
        w.handle(p, ClientMsg::Sandbox(SandboxCmd::SetLevel(10)));
        assert_eq!(w.entities[&p].level, 1);
        w.sandbox = true;
        w.handle(p, ClientMsg::Sandbox(SandboxCmd::SetLevel(10)));
        assert_eq!(w.entities[&p].level, 10);
        w.handle(p, ClientMsg::Sandbox(SandboxCmd::AddMoney(5000)));
        assert_eq!(w.entities[&p].player().unwrap().money, 5000);
        w.handle(
            p,
            ClientMsg::Sandbox(SandboxCmd::GiveItem(items::HEALING_POTION)),
        );
        assert_eq!(
            count_item(
                &w.entities[&p].player().unwrap().bags,
                items::HEALING_POTION
            ),
            1
        );
        w.handle(p, ClientMsg::Sandbox(SandboxCmd::Teleport(Zone::Frostcog)));
        assert_eq!(Zone::at(w.entities[&p].pos), Zone::Frostcog);
        // Out in the wilds, a summoned mob attacks, but god mode takes no damage.
        let wilds = Zone::Frostcog.ground_local(Zone::Frostcog.layout().fields[0]);
        w.entities.get_mut(&p).unwrap().pos = wilds;
        w.handle(p, ClientMsg::Sandbox(SandboxCmd::ToggleGod));
        let before = w.mob_count();
        w.handle(
            p,
            ClientMsg::Sandbox(SandboxCmd::SpawnMob {
                kind: MobKind::Yeti,
                level: 10,
            }),
        );
        assert_eq!(w.mob_count(), before + 1);
        let hp = w.entities[&p].hp;
        run(&mut w, 6.0);
        assert!(w.entities[&p].in_combat);
        assert_eq!(w.entities[&p].hp, hp);
        w.handle(p, ClientMsg::Sandbox(SandboxCmd::ClearSpawns));
        assert_eq!(w.mob_count(), before);
    }

    /// The quest giver in a zone, with the player moved next to them.
    fn visit_giver(w: &mut World, p: EntityId, zone: Zone) -> EntityId {
        let giver = w
            .entities
            .values()
            .find(|e| e.kind() == EntityKind::QuestGiver(zone.race()) && Zone::at(e.pos) == zone)
            .unwrap()
            .id;
        let at = w.entities[&giver].pos;
        w.entities.get_mut(&p).unwrap().pos = at + Vec3::new(1.5, 0.0, 0.0);
        giver
    }

    /// A kill that counts for `p`.
    fn credit_kill(w: &mut World, p: EntityId, kind: MobKind) {
        let mob = w
            .entities
            .values()
            .find(|e| e.mob().is_some_and(|m| m.kind == kind) && !e.dead)
            .unwrap()
            .id;
        mob_of(&mut w.entities.get_mut(&mob).unwrap().brain)
            .threat
            .push((p, 1.0));
        w.kill(mob, Some(p));
    }

    #[test]
    fn hunting_quests_count_kills_and_pay_out() {
        let mut w = World::new(41);
        let p = join(&mut w, "Hunter", Class::Ranger, 2);
        let giver = visit_giver(&mut w, p, Zone::Amberfall);
        let q = quests::zone_quests(Zone::Amberfall)[0].id;
        // Not done yet: can't hand in.
        w.handle(p, ClientMsg::AcceptQuest { giver, quest: q });
        assert_eq!(w.entities[&p].player().unwrap().quests.progress(q), Some(0));
        assert!(w.turn_in_quest(p, giver, q).is_err());
        for _ in 0..8 {
            credit_kill(&mut w, p, MobKind::Wolf);
        }
        // Boars don't count.
        credit_kill(&mut w, p, MobKind::Boar);
        assert_eq!(w.entities[&p].player().unwrap().quests.progress(q), Some(8));
        let money = w.entities[&p].player().unwrap().money;
        assert_eq!(w.turn_in_quest(p, giver, q), Ok(()));
        let pd = w.entities[&p].player().unwrap();
        assert!(pd.quests.is_done(q));
        assert!(pd.money >= money + 150);
        assert_eq!(count_item(&pd.bags, items::TRAILBLAZER_BOOTS), 1);
        // Once only.
        assert!(w.accept_quest(p, giver, q).is_err());
        // And saved with the character.
        assert!(w.character(p).unwrap().quests.is_done(q));
    }

    #[test]
    fn crafting_quests_take_the_crafted_armor() {
        let mut w = World::new(42);
        let p = join(&mut w, "Smith", Class::Fighter, 3);
        let giver = visit_giver(&mut w, p, Zone::Grubdeep);
        let q = &quests::zone_quests(Zone::Grubdeep)[1];
        let Goal::TurnIn { item: wanted } = q.goal else {
            panic!()
        };
        assert_eq!(w.accept_quest(p, giver, q.id), Ok(()));
        assert!(w.turn_in_quest(p, giver, q.id).is_err());
        add_item(
            &mut w.entities.get_mut(&p).unwrap().player_mut().unwrap().bags,
            wanted,
            1,
        );
        assert_eq!(w.turn_in_quest(p, giver, q.id), Ok(()));
        let pd = w.entities[&p].player().unwrap();
        assert_eq!(count_item(&pd.bags, wanted), 0);
        assert_eq!(count_item(&pd.bags, q.reward.unwrap()), 1);
    }

    #[test]
    fn quest_givers_only_offer_their_own_quests_up_close() {
        let mut w = World::new(43);
        let p = join(&mut w, "Wanderer", Class::Mage, 10);
        let giver = visit_giver(&mut w, p, Zone::Frostcog);
        let human_quest = quests::zone_quests(Zone::Amberfall)[0].id;
        assert!(w.accept_quest(p, giver, human_quest).is_err());
        let elite = quests::zone_quests(Zone::Frostcog)[2].id;
        assert_eq!(w.accept_quest(p, giver, elite), Ok(()));
        w.entities.get_mut(&p).unwrap().pos =
            Zone::Frostcog.graveyard() + Vec3::new(30.0, 0.0, 0.0);
        let hunt = quests::zone_quests(Zone::Frostcog)[0].id;
        assert!(w.accept_quest(p, giver, hunt).is_err(), "too far away");
        // Merchants aren't quest givers, and quest givers don't trade.
        let merchant = w
            .entities
            .values()
            .find(|e| e.kind() == EntityKind::Merchant(Race::Gnome))
            .unwrap()
            .id;
        assert!(w.accept_quest(p, merchant, hunt).is_err());
        assert!(w.buy(p, giver, items::HEALING_POTION).is_err());
        w.handle(p, ClientMsg::AbandonQuest(elite));
        assert_eq!(w.entities[&p].player().unwrap().quests.active.len(), 0);
    }

    #[test]
    fn dungeon_master_joe_sends_you_after_the_sunken_king() {
        let mut w = World::new(45);
        let p = join(&mut w, "Hero", Class::Mage, 10);
        let ours = quests::dungeon_quest(DungeonId::SunkenVault).id;
        // The town quest givers don't offer it.
        let giver = visit_giver(&mut w, p, Zone::Amberfall);
        assert!(w.accept_quest(p, giver, ours).is_err());
        at_waystone(&mut w, p);
        w.handle(
            p,
            ClientMsg::Travel(Destination::Dungeon(DungeonId::SunkenVault)),
        );
        let Place::Dungeon(index) = place(&w, p) else {
            panic!("not in the vault")
        };
        let joe = w
            .entities
            .values()
            .find(|e| e.name == quests::VAULT_GIVER && Place::at(e.pos) == Place::Dungeon(index))
            .unwrap()
            .id;
        assert_eq!(w.entities[&joe].kind(), EntityKind::QuestGiver(Race::Human));
        assert!(
            w.accept_quest(p, joe, quests::zone_quests(Zone::Amberfall)[0].id)
                .is_err()
        );
        assert_eq!(w.accept_quest(p, joe, ours), Ok(()));
        // Kill the King, come back to Joe, and the trident is yours.
        let king = vault_mobs(&w, index)
            .into_iter()
            .find(|m| w.entities[m].mob().unwrap().kind == MobKind::SunkenKing)
            .unwrap();
        w.provoke(p, king, 10.0);
        w.kill(king, Some(p));
        assert_eq!(
            w.entities[&p].player().unwrap().quests.progress(ours),
            Some(1)
        );
        assert_eq!(w.turn_in_quest(p, joe, ours), Ok(()));
        let bags = &w.entities[&p].player().unwrap().bags;
        assert!(bags.contains(&Some((items::TIDEBREAKER_TRIDENT, 1))));
        // Joe goes when the vault closes.
        w.entities.get_mut(&p).unwrap().pos = dungeon::to_world(index, dungeon::EXIT_STONE);
        w.handle(p, ClientMsg::Travel(Destination::Town(Zone::Amberfall)));
        run(&mut w, dungeon::EMPTY_RESET + 5.0);
        assert!(!w.entities.contains_key(&joe));
    }

    #[test]
    fn mobs_sometimes_drop_green_gear() {
        let mut w = World::new(44);
        let mut greens = 0;
        for _ in 0..400 {
            if let Some(loot) = w.roll_loot(MobKind::Wolf, 5, vec![1]) {
                greens += loot
                    .items
                    .iter()
                    .filter(|(id, _)| item(*id).quality == Quality::Uncommon)
                    .count();
            }
        }
        assert!(
            (4..=40).contains(&greens),
            "{greens} greens from 400 wolves"
        );
    }

    #[test]
    fn weapons_are_held_saved_and_put_away() {
        let mut w = World::new(12);
        let p = join(&mut w, "Swordy", Class::Fighter, 3);
        let pd = w.entities.get_mut(&p).unwrap().player_mut().unwrap();
        add_item(&mut pd.bags, items::IRON_SWORD, 1);
        add_item(&mut pd.bags, items::HEARTSTONE_GREATSWORD, 1);
        let hp = w.entities[&p].max_hp;
        w.equip(p, 0).unwrap();
        w.equip(p, 1).unwrap();
        // The greatsword swapped places with the sword.
        let pd = w.entities[&p].player().unwrap();
        assert_eq!(pd.weapon, Some(items::HEARTSTONE_GREATSWORD));
        assert_eq!(pd.bags[1], Some((items::IRON_SWORD, 1)));
        assert_eq!(pd.stats.damage, 7.0);
        assert!(w.entities[&p].max_hp > hp);
        let snap = w.snapshot_for(p).unwrap();
        let me = snap.entities.iter().find(|e| e.id == p).unwrap();
        assert_eq!(me.weapon, Some(items::HEARTSTONE_GREATSWORD));
        let saved = w.character(p).unwrap();
        assert_eq!(saved.weapon, Some(items::HEARTSTONE_GREATSWORD));
        w.handle(p, ClientMsg::UnequipWeapon);
        let pd = w.entities[&p].player().unwrap();
        assert_eq!(pd.weapon, None);
        assert_eq!(pd.stats.damage, 0.0);
        assert_eq!(w.entities[&p].max_hp, hp);
        assert_eq!(count_item(&pd.bags, items::HEARTSTONE_GREATSWORD), 1);
    }

    #[test]
    fn weapons_add_auto_attack_damage() {
        let dealt = |weapon: Option<ItemId>| {
            let mut w = World::new(31);
            let p = join(&mut w, "Hitter", Class::Barbarian, 1);
            w.entities.get_mut(&p).unwrap().player_mut().unwrap().weapon = weapon;
            w.refresh_stats(p);
            let mob = engage(&mut w, p, MobKind::Boar);
            sturdy(&mut w, mob);
            w.entities.get_mut(&p).unwrap().player_mut().unwrap().god = true;
            w.handle(p, ClientMsg::StartAttack);
            for _ in 0..(30.0 / DT) as usize {
                let mpos = w.entities[&mob].pos;
                w.entities.get_mut(&p).unwrap().pos = ground(mpos.x - 2.5, mpos.z);
                face(&mut w, p, mob);
                w.tick(DT);
            }
            w.entities[&mob].max_hp - w.entities[&mob].hp
        };
        let bare = dealt(None);
        let armed = dealt(Some(items::HEARTSTONE_GREATSWORD));
        assert!(bare > 0.0);
        assert!(
            armed > bare * 1.3,
            "{armed} with a greatsword, {bare} without"
        );
    }

    #[test]
    fn casters_drop_potions_and_elites_drop_scrap() {
        let mut w = World::new(17);
        let mut potions = 0;
        for _ in 0..400 {
            let loot = w.roll_loot(MobKind::BanditMystic, 3, vec![1]).unwrap();
            for (id, n) in loot.items {
                if id == items::HEALING_POTION {
                    assert_eq!(n, 1);
                    potions += 1;
                }
            }
        }
        assert!((100..=180).contains(&potions), "{potions} potions from 400");
        for kind in [MobKind::Golem, MobKind::Yeti, MobKind::StoneWarden] {
            let loot = w.roll_loot(kind, 5, vec![1]).unwrap();
            assert!(loot.items.contains(&(items::IRON_SCRAP, 5)), "{kind:?}");
        }
        for _ in 0..20 {
            let loot = w.roll_loot(MobKind::SunkenKing, 10, vec![1]).unwrap();
            for mat in [items::IRON_SCRAP, items::LIGHT_LEATHER] {
                let n = loot.items.iter().find(|(i, _)| *i == mat).unwrap().1;
                assert!((3..=5).contains(&n));
            }
        }
    }

    #[test]
    fn fighters_drop_scrap_for_weapons_and_mobs_drop_weapons() {
        let mut w = World::new(9);
        let mut scrap = 0;
        let mut weapons = 0;
        for _ in 0..300 {
            let loot = w.roll_loot(MobKind::Bandit, 3, vec![1]).unwrap();
            for (id, n) in loot.items {
                if id == items::IRON_SCRAP {
                    scrap += n;
                }
                if WEAPON_DROPS.contains(&id) {
                    weapons += 1;
                }
            }
        }
        assert!(scrap >= 100, "{scrap} scrap from 300 bandits");
        assert!((1..=25).contains(&weapons), "{weapons} weapons");

        let p = join(&mut w, "Smith", Class::Fighter, 1);
        let sword = RECIPES
            .iter()
            .position(|r| r.result == items::IRON_SWORD)
            .unwrap();
        let pd = w.entities.get_mut(&p).unwrap().player_mut().unwrap();
        add_item(&mut pd.bags, items::IRON_SCRAP, 5);
        add_item(&mut pd.bags, items::LIGHT_LEATHER, 2);
        w.craft(p, sword).unwrap();
        let pd = w.entities[&p].player().unwrap();
        assert_eq!(count_item(&pd.bags, items::IRON_SWORD), 1);
        assert_eq!(count_item(&pd.bags, items::IRON_SCRAP), 0);
    }

    #[test]
    fn water_mobs_live_in_the_shallows() {
        let w = World::new(3);
        for zone in Zone::ALL {
            let water: Vec<_> = w
                .entities
                .values()
                .filter(|e| {
                    e.mob()
                        .is_some_and(|m| Some(m.kind) == MobKind::water(zone))
                })
                .collect();
            let lakes = zone.layout().shores.len();
            assert_eq!(water.len(), lakes * shared::layout::SHORE_COUNT, "{zone:?}");
            assert_eq!(lakes > 0, MobKind::water(zone).is_some(), "{zone:?}");
            for e in water {
                assert!((3..=5).contains(&e.level));
                assert_eq!(Zone::at(e.pos), zone);
                assert!(e.pos.y < zone.water_level() + 0.5, "{zone:?} {}", e.pos);
            }
        }
    }

    #[test]
    fn boars_and_water_mobs_drop_food_to_cook_and_eat() {
        let mut w = World::new(23);
        let count = |w: &mut World, kind: MobKind, food: ItemId| {
            (0..200)
                .map(|_| {
                    let loot = w.roll_loot(kind, 4, vec![1]).unwrap();
                    loot.items.iter().filter(|(i, _)| *i == food).count()
                })
                .sum::<usize>()
        };
        for boar in [MobKind::Boar, MobKind::PlagueBoar] {
            assert!(count(&mut w, boar, items::BOAR_MEAT) > 60, "{boar:?}");
        }
        assert_eq!(count(&mut w, MobKind::Hyena, items::BOAR_MEAT), 0);
        for fish in [MobKind::MudsnapCrab, MobKind::BogLurker] {
            assert!(count(&mut w, fish, items::RAW_FISH) > 80, "{fish:?}");
        }

        let p = join(&mut w, "Cook", Class::Fighter, 5);
        let pd = w.entities.get_mut(&p).unwrap().player_mut().unwrap();
        add_item(&mut pd.bags, items::RAW_FISH, 2);
        let (fish, _) = Skill::Cooking
            .recipes()
            .find(|(_, r)| r.result == items::COOKED_FISH)
            .unwrap();
        w.craft(p, fish).unwrap();
        w.craft(p, fish).unwrap();
        let pd = w.entities[&p].player().unwrap();
        assert_eq!(count_item(&pd.bags, items::COOKED_FISH), 2);

        // Eating heals three quarters of a potion, and shares its cooldown.
        let e = w.entities.get_mut(&p).unwrap();
        e.hp = 1.0;
        let max = e.max_hp;
        let slot = e
            .player()
            .unwrap()
            .bags
            .iter()
            .position(|s| matches!(s, Some((i, _)) if *i == items::COOKED_FISH))
            .unwrap();
        w.use_item(p, slot).unwrap();
        let healed = w.entities[&p].hp - 1.0;
        assert!(
            (healed - max * 0.35 * 0.75).abs() < 1.0,
            "healed {healed} of {max}"
        );
        assert!(w.use_item(p, slot).is_err());
    }

    /// A Sunken King summoned next to a player, already fighting them.
    fn boss_fight(w: &mut World, p: EntityId) -> EntityId {
        boss_fight_with(w, p, MobKind::SunkenKing)
    }

    /// A boss summoned next to a player, already fighting them.
    fn boss_fight_with(w: &mut World, p: EntityId, kind: MobKind) -> EntityId {
        w.sandbox = true;
        // Out of town, where mobs keep their threat.
        let e = w.entities.get_mut(&p).unwrap();
        let zone = Zone::at(e.pos);
        let c = zone.center();
        e.pos = ground(c.x + shared::world::TOWN_RADIUS + 12.0, c.y);
        assert!(!zone.in_town(e.pos));
        w.handle(
            p,
            ClientMsg::Sandbox(SandboxCmd::SpawnMob { kind, level: 10 }),
        );
        let boss = w
            .entities
            .values()
            .find(|e| e.mob().is_some_and(|m| m.kind == kind))
            .unwrap()
            .id;
        sturdy(w, boss);
        w.provoke(p, boss, 100.0);
        boss
    }

    #[test]
    fn the_sunken_king_marks_the_ground_and_it_hurts_to_stand_in() {
        for dodge in [false, true] {
            let mut w = World::new(5);
            let p = join(&mut w, "Tank", Class::Fighter, 10);
            w.entities.get_mut(&p).unwrap().player_mut().unwrap().god = false;
            let boss = boss_fight(&mut w, p);
            // Wait for the first Tidal Crash to be marked.
            let mut marked = None;
            for _ in 0..(10.0 / DT) as usize {
                w.tick(DT);
                let e = w.entities.get_mut(&p).unwrap();
                e.hp = e.max_hp;
                if let Some(h) = w.hazards.first() {
                    marked = Some(h.pos);
                    break;
                }
            }
            let at = marked.expect("no Tidal Crash");
            assert!(at.distance(w.entities[&p].pos) < 0.5);
            assert!(!w.snapshot_for(p).unwrap().hazards.is_empty());
            // It comes down twice in a row, the second time wherever you are
            // when the first lands.
            let hp = w.entities[&p].hp;
            for crash in 0..boss::kit(MobKind::SunkenKing).hazard.times {
                if dodge {
                    let e = w.entities.get_mut(&p).unwrap();
                    e.pos = ground(e.pos.x + 8.0, e.pos.z);
                }
                // Keep the boss from swinging, so only the crash can hurt.
                for _ in 0..(boss::kit(MobKind::SunkenKing).hazard.warning / DT) as usize + 2 {
                    w.entities.get_mut(&boss).unwrap().swing_timer = 10.0;
                    w.entities.get_mut(&boss).unwrap().cast = None;
                    w.tick(DT);
                }
                if crash + 1 < boss::kit(MobKind::SunkenKing).hazard.times {
                    let next = w.hazards.first().expect("no second Tidal Crash");
                    assert!(next.pos.distance(w.entities[&p].pos) < 0.5);
                }
            }
            assert!(w.hazards.is_empty());
            let lost = hp - w.entities[&p].hp;
            if dodge {
                assert_eq!(lost, 0.0, "dodged but lost {lost}");
            } else {
                assert!(lost > 60.0, "stood in both and lost only {lost}");
            }
        }
    }

    #[test]
    fn the_sunken_kings_spells_can_be_interrupted() {
        let mut w = World::new(6);
        let p = join(&mut w, "Rogue", Class::Rogue, 10);
        let boss = boss_fight(&mut w, p);
        w.entities.get_mut(&p).unwrap().player_mut().unwrap().god = true;
        let mut seen = Vec::new();
        for _ in 0..(40.0 / DT) as usize {
            // Hurt him enough that he wants to heal.
            let b = w.entities.get_mut(&boss).unwrap();
            b.hp = b.hp.min(b.max_hp * 0.5);
            w.tick(DT);
            let Some(cast) = w.entities[&boss].cast.as_ref().map(|c| c.ability) else {
                continue;
            };
            if !seen.contains(&cast) {
                seen.push(cast);
            }
            let me = w.entities.get_mut(&p).unwrap();
            me.target = Some(boss);
            me.gcd = 0.0;
            me.cooldowns.clear();
            let bpos = w.entities[&boss].pos;
            let me = w.entities.get_mut(&p).unwrap();
            me.pos = ground(bpos.x - 2.5, bpos.z);
            me.yaw = yaw_towards(me.pos, bpos);
            me.power = me.max_power;
            w.try_use(p, ids::KICK).unwrap();
            assert!(
                w.entities[&boss].cast.is_none(),
                "{cast:?} wasn't interrupted"
            );
        }
        assert!(seen.contains(&ids::DROWNING_GRASP), "{seen:?}");
        assert!(seen.contains(&ids::CALL_OF_THE_DEEP), "{seen:?}");
        assert!(MobKind::SunkenKing.template().boss);
    }

    #[test]
    fn dungeon_gear_drops_for_each_player_on_their_own() {
        let mut w = World::new(8);
        let a = join(&mut w, "Ann", Class::Fighter, 10);
        let b = join(&mut w, "Bo", Class::Mage, 10);
        let loot = w.roll_loot(MobKind::SunkenKing, 10, vec![a, b]).unwrap();
        // The King always drops green armor, so both get some.
        assert_eq!(loot.personal.len(), 2);
        assert!(loot.items.iter().all(|(i, _)| !is_gear(*i)));
        let bs_gear = loot
            .personal
            .iter()
            .find(|(p, _)| *p == b)
            .unwrap()
            .1
            .clone();
        // Ann loots the corpse: she gets the shared loot and her own gear.
        let boss = boss_fight(&mut w, a);
        let e = w.entities.get_mut(&boss).unwrap();
        e.dead = true;
        let bpos = e.pos;
        mob_of(&mut e.brain).loot = Some(Loot { turn: None, ..loot });
        for p in [a, b] {
            w.entities.get_mut(&p).unwrap().pos = ground(bpos.x - 1.0, bpos.z);
        }
        w.loot(a, boss).unwrap();
        let left = w.entities[&boss].mob().unwrap().loot.as_ref().unwrap();
        assert!(left.allows(b) && !left.allows(a));
        w.loot(b, boss).unwrap();
        assert!(w.entities[&boss].mob().unwrap().loot.is_none());
        for (item, n) in bs_gear {
            assert!(count_item(&w.entities[&b].player().unwrap().bags, item) >= n as u32);
        }
        // Out in the world, gear is still shared.
        let wolf = w.roll_loot(MobKind::Golem, 10, vec![a, b]).unwrap();
        assert!(wolf.personal.is_empty());
    }

    fn party_of_two(w: &mut World) -> (EntityId, EntityId) {
        let a = join(w, "Ann", Class::Barbarian, 3);
        let b = join(w, "Bo", Class::Cleric, 3);
        w.handle(a, ClientMsg::PartyInvite("bo".into()));
        assert!(w.entities[&b].player().unwrap().invite.is_some());
        assert_eq!(w.snapshot_for(b).unwrap().me.invite.as_deref(), Some("Ann"));
        w.handle(b, ClientMsg::PartyAccept);
        (a, b)
    }

    #[test]
    fn invite_accept_leave() {
        let mut w = World::new(3);
        let (a, b) = party_of_two(&mut w);
        let party = w.party_of(a).unwrap();
        assert_eq!(w.party_of(b), Some(party));
        let view = w.snapshot_for(b).unwrap().me.party.unwrap();
        assert_eq!(view.leader, a);
        assert_eq!(view.members.len(), 2);

        // Only the leader invites; a third joins through the chat command.
        let c = join(&mut w, "Cy", Class::Barbarian, 2);
        w.handle(b, ClientMsg::PartyInvite("Cy".into()));
        assert!(w.entities[&c].player().unwrap().invite.is_none());
        w.handle(a, ClientMsg::Chat("/invite Cy".into()));
        w.handle(c, ClientMsg::Chat("/accept".into()));
        assert_eq!(w.parties[&party].members, vec![a, b, c]);

        // The leader leaving hands the lead on; kicking down to one disbands.
        w.handle(a, ClientMsg::PartyLeave);
        assert_eq!(w.party_of(a), None);
        assert_eq!(w.parties[&party].leader, b);
        w.handle(b, ClientMsg::PartyKick("Cy".into()));
        assert!(w.parties.is_empty());
        assert_eq!(w.party_of(b), None);
        assert_eq!(w.party_of(c), None);
    }

    #[test]
    fn invites_run_out_and_parties_fill_up() {
        let mut w = World::new(3);
        let a = join(&mut w, "Lead", Class::Barbarian, 3);
        let b = join(&mut w, "Slow", Class::Barbarian, 3);
        w.handle(a, ClientMsg::PartyInvite("Slow".into()));
        run(&mut w, PARTY_INVITE_TIME + 1.0);
        w.handle(b, ClientMsg::PartyAccept);
        assert_eq!(w.party_of(b), None);

        let mut ids = vec![];
        for i in 0..MAX_PARTY_SIZE {
            let name = format!("M{i}");
            let id = join(&mut w, &name, Class::Barbarian, 3);
            w.handle(a, ClientMsg::PartyInvite(name));
            w.handle(id, ClientMsg::PartyAccept);
            ids.push(id);
        }
        let party = w.party_of(a).unwrap();
        assert_eq!(w.parties[&party].members.len(), MAX_PARTY_SIZE);
        assert_eq!(w.party_of(*ids.last().unwrap()), None);
    }

    #[test]
    fn logging_out_leaves_the_party() {
        let mut w = World::new(3);
        let (a, b) = party_of_two(&mut w);
        w.remove_player(a);
        assert_eq!(w.party_of(b), None);
        assert!(w.parties.is_empty());
    }

    #[test]
    fn party_members_nearby_share_kills_and_take_turns_looting() {
        let mut w = World::new(5);
        let (a, b) = party_of_two(&mut w);
        let far = join(&mut w, "Far", Class::Cleric, 3);
        w.handle(a, ClientMsg::PartyInvite("Far".into()));
        w.handle(far, ClientMsg::PartyAccept);
        let stranger = join(&mut w, "Stranger", Class::Barbarian, 3);

        for round in 0..2 {
            let boar = engage(&mut w, a, MobKind::Boar);
            let at = w.entities[&boar].pos;
            for (id, dx) in [(b, 5.0), (stranger, 5.0), (far, PARTY_RANGE + 20.0)] {
                w.entities.get_mut(&id).unwrap().pos = ground(at.x + dx, at.z);
            }
            let xp: Vec<u32> = [a, b, far, stranger]
                .iter()
                .map(|id| w.entities[id].player().unwrap().xp)
                .collect();
            w.entities.get_mut(&boar).unwrap().mob_mut().unwrap().threat = vec![(a, 10.0)];
            w.kill(boar, Some(a));
            let gained: Vec<bool> = [a, b, far, stranger]
                .iter()
                .zip(&xp)
                .map(|(id, x)| w.entities[id].player().unwrap().xp != *x)
                .collect();
            assert_eq!(gained, [true, true, false, false], "round {round}");
            // Nobody's share is cut: each gets what they'd get solo.
            let solo = kill_xp(3, w.entities[&boar].level, false);
            for (id, before) in [(a, xp[0]), (b, xp[1])] {
                let now = w.entities[&id].player().unwrap().xp;
                assert_eq!(now - before, solo);
            }

            let loot = w.entities[&boar].mob().unwrap().loot.as_ref().unwrap();
            assert_eq!(loot.looters, vec![a, b]);
            // Ann loots first, then Bo.
            let first = if round == 0 { a } else { b };
            let second = if round == 0 { b } else { a };
            assert_eq!(loot.turn.map(|t| t.0), Some(first));
            assert!(loot.allows(first) && !loot.allows(second));
            run(&mut w, LOOT_TURN_TIME + 0.5);
            let loot = w.entities[&boar].mob().unwrap().loot.as_ref().unwrap();
            assert!(loot.allows(second));
            // Bring it back for the next round.
            let mut e = w.entities.remove(&boar).unwrap();
            w.reset_mob(&mut e);
            w.entities.insert(boar, e);
        }
    }

    #[test]
    fn party_chat_reaches_only_the_party() {
        let mut w = World::new(3);
        let (a, b) = party_of_two(&mut w);
        let c = join(&mut w, "Out", Class::Barbarian, 3);
        w.drain_outbox();
        w.handle(b, ClientMsg::Chat("/p pull the boar".into()));
        let to: Vec<EntityId> = w
            .drain_outbox()
            .into_iter()
            .filter_map(|(aud, m)| match (aud, m) {
                (Audience::Only(id), ServerMsg::Event(GameEvent::PartyChat { .. })) => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(to, vec![a, b]);
        assert!(!to.contains(&c));
    }

    // ---- Waystones and the Sunken Vault ----

    use shared::dungeon;
    use shared::props::WAYSTONE_SPOT;

    /// Puts a player beside their zone's waystone.
    fn at_waystone(w: &mut World, p: EntityId) {
        let zone = Zone::at(w.entities[&p].pos);
        w.entities.get_mut(&p).unwrap().pos = zone.ground_local(WAYSTONE_SPOT + vec2(0.0, -2.0));
    }

    fn place(w: &World, p: EntityId) -> Place {
        Place::at(w.entities[&p].pos)
    }

    fn vault_mobs(w: &World, index: u32) -> Vec<EntityId> {
        w.entities
            .values()
            .filter(|e| e.mob().is_some() && Place::at(e.pos) == Place::Dungeon(index))
            .map(|e| e.id)
            .collect()
    }

    #[test]
    fn killing_the_boss_opens_a_teleporter_out() {
        let mut w = World::new(12);
        let p = join(&mut w, "Wren", Class::Mage, 10);
        at_waystone(&mut w, p);
        w.handle(
            p,
            ClientMsg::Travel(Destination::Dungeon(DungeonId::SunkenVault)),
        );
        let Place::Dungeon(index) = place(&w, p) else {
            panic!("not in the vault")
        };
        let king = vault_mobs(&w, index)
            .into_iter()
            .find(|m| w.entities[m].mob().unwrap().kind == MobKind::SunkenKing)
            .unwrap();
        let portal = dungeon::to_world(index, dungeon::SUNKEN_VAULT.portal);
        w.entities.get_mut(&p).unwrap().pos = portal;
        assert!(w.snapshot_for(p).unwrap().portal.is_none());
        w.handle(p, ClientMsg::Travel(Destination::Town(Zone::Amberfall)));
        assert_eq!(place(&w, p), Place::Dungeon(index), "no teleporter yet");
        w.kill(king, Some(p));
        assert_eq!(w.snapshot_for(p).unwrap().portal, Some(portal));
        w.handle(p, ClientMsg::Travel(Destination::Town(Zone::Amberfall)));
        assert_eq!(place(&w, p), Place::Zone(Zone::Amberfall));
    }

    #[test]
    fn waystones_carry_players_between_towns() {
        let mut w = World::new(11);
        let p = join(&mut w, "Wren", Class::Mage, 2);
        // Only at a waystone.
        w.handle(p, ClientMsg::Travel(Destination::Town(Zone::Frostcog)));
        assert_eq!(place(&w, p), Place::Zone(Zone::Amberfall));
        at_waystone(&mut w, p);
        w.drain_outbox();
        w.handle(p, ClientMsg::Travel(Destination::Town(Zone::Amberfall)));
        assert_eq!(place(&w, p), Place::Zone(Zone::Amberfall));
        w.handle(p, ClientMsg::Travel(Destination::Town(Zone::Frostcog)));
        let pos = w.entities[&p].pos;
        assert_eq!(Place::at(pos), Place::Zone(Zone::Frostcog));
        assert!(Zone::Frostcog.in_town(pos));
        assert!(
            w.drain_outbox()
                .iter()
                .any(|(to, m)| matches!(to, Audience::Only(i) if *i == p)
                    && matches!(m, ServerMsg::SetPosition { pos: q, .. } if *q == pos))
        );
        // Not while fighting.
        w.entities.get_mut(&p).unwrap().in_combat = true;
        w.handle(p, ClientMsg::Travel(Destination::Town(Zone::Grubdeep)));
        assert_eq!(place(&w, p), Place::Zone(Zone::Frostcog));
    }

    #[test]
    fn a_party_shares_a_copy_of_the_vault_and_others_get_their_own() {
        let mut w = World::new(12);
        let (a, b) = party_of_two(&mut w);
        let c = join(&mut w, "Cy", Class::Rogue, 9);
        let low = join(
            &mut w,
            "Lo",
            Class::Rogue,
            dungeon::SUNKEN_VAULT.min_level - 1,
        );
        for p in [a, b] {
            w.entities.get_mut(&p).unwrap().level = 9;
        }
        for p in [a, b, c, low] {
            at_waystone(&mut w, p);
            w.handle(
                p,
                ClientMsg::Travel(Destination::Dungeon(DungeonId::SunkenVault)),
            );
        }
        assert!(matches!(place(&w, low), Place::Zone(_)), "too low to enter");
        let Place::Dungeon(ours) = place(&w, a) else {
            panic!("not in the vault")
        };
        assert_eq!(place(&w, b), Place::Dungeon(ours));
        let Place::Dungeon(theirs) = place(&w, c) else {
            panic!("not in the vault")
        };
        assert_ne!(ours, theirs);
        let per_copy: usize = dungeon::SUNKEN_VAULT
            .packs
            .iter()
            .flat_map(|p| p.mobs.iter().map(|m| m.2))
            .sum();
        assert_eq!(vault_mobs(&w, ours).len(), per_copy);
        assert_eq!(vault_mobs(&w, theirs).len(), per_copy);
        // Nobody sees into another copy.
        let seen = w.snapshot_for(c).unwrap().entities;
        assert!(seen.iter().all(|e| e.id != a && e.id != b));
        // Entrances are clear of mobs, so a few seconds there are safe.
        run(&mut w, 3.0);
        assert!(!w.entities[&a].in_combat);
    }

    #[test]
    fn vault_mobs_stay_dead_and_an_empty_copy_closes() {
        let mut w = World::new(13);
        let p = join(&mut w, "Vex", Class::Barbarian, 10);
        at_waystone(&mut w, p);
        w.handle(
            p,
            ClientMsg::Travel(Destination::Dungeon(DungeonId::SunkenVault)),
        );
        let Place::Dungeon(index) = place(&w, p) else {
            panic!("not in the vault")
        };
        let hound = engage(&mut w, p, MobKind::VaultHound);
        w.kill(hound, Some(p));
        run(&mut w, 200.0);
        assert!(w.entities[&hound].dead, "vault mobs don't respawn");
        // Dying in the vault brings you back at its entrance.
        let e = w.entities.get_mut(&p).unwrap();
        e.dead = true;
        e.pos = dungeon::to_world(index, vec2(42.0, 84.0));
        w.handle(p, ClientMsg::ReleaseSpirit);
        assert_eq!(
            w.entities[&p].pos,
            dungeon::to_world(index, dungeon::ENTRANCE)
        );
        // Leave by the stone at the entrance, back to the town you came from.
        w.entities.get_mut(&p).unwrap().pos = dungeon::to_world(index, dungeon::EXIT_STONE);
        w.handle(
            p,
            ClientMsg::Travel(Destination::Dungeon(DungeonId::SunkenVault)),
        );
        assert_eq!(place(&w, p), Place::Dungeon(index), "already inside");
        w.handle(p, ClientMsg::Travel(Destination::Town(Zone::Amberfall)));
        assert_eq!(place(&w, p), Place::Zone(Zone::Amberfall));
        run(&mut w, dungeon::EMPTY_RESET - 5.0);
        assert!(w.instances.contains_key(&index));
        run(&mut w, 10.0);
        assert!(!w.instances.contains_key(&index));
        assert!(vault_mobs(&w, index).is_empty());
        assert!(!w.entities.contains_key(&hound));
        // A fresh copy next time, with every mob back.
        at_waystone(&mut w, p);
        w.handle(
            p,
            ClientMsg::Travel(Destination::Dungeon(DungeonId::SunkenVault)),
        );
        let Place::Dungeon(again) = place(&w, p) else {
            panic!("not in the vault")
        };
        assert!(vault_mobs(&w, again).iter().all(|m| !w.entities[m].dead));
    }

    #[test]
    fn leaving_the_party_takes_you_out_of_its_vault() {
        let mut w = World::new(14);
        let (a, b) = party_of_two(&mut w);
        let c = join(&mut w, "Cy", Class::Rogue, 9);
        w.handle(a, ClientMsg::PartyInvite("cy".into()));
        w.handle(c, ClientMsg::PartyAccept);
        for p in [a, b, c] {
            w.entities.get_mut(&p).unwrap().level = 9;
            at_waystone(&mut w, p);
            w.handle(
                p,
                ClientMsg::Travel(Destination::Dungeon(DungeonId::SunkenVault)),
            );
        }
        let Place::Dungeon(index) = place(&w, a) else {
            panic!("not in the vault")
        };
        w.handle(c, ClientMsg::PartyLeave);
        run(&mut w, 0.2);
        assert_eq!(place(&w, c), Place::Zone(Zone::Amberfall));
        assert_eq!(place(&w, a), Place::Dungeon(index));
        // When the party breaks up, the last one inside keeps the copy.
        w.handle(b, ClientMsg::PartyLeave);
        run(&mut w, 0.2);
        assert_eq!(place(&w, a), Place::Dungeon(index));
        assert_eq!(place(&w, b), Place::Zone(Zone::Amberfall));
        assert_eq!(w.instances[&index].owner, Owner::Solo("Ann".into()));
    }

    #[test]
    fn logging_out_in_the_vault_saves_you_in_town() {
        let mut w = World::new(15);
        let p = join(&mut w, "Ivo", Class::Mage, 9);
        w.entities.get_mut(&p).unwrap().pos =
            Zone::Grubdeep.ground_local(WAYSTONE_SPOT + vec2(1.0, 0.0));
        w.handle(
            p,
            ClientMsg::Travel(Destination::Dungeon(DungeonId::SunkenVault)),
        );
        assert!(matches!(place(&w, p), Place::Dungeon(_)));
        let c = w.remove_player(p).unwrap();
        let pos = Vec3::from(c.pos);
        assert!(Zone::Grubdeep.in_town(pos));
        // A save made inside (a crash) loads in town too.
        let mut c = c;
        c.pos = dungeon::to_world(3, dungeon::ENTRANCE).to_array();
        let q = w.add_player(&c);
        assert!(matches!(place(&w, q), Place::Zone(_)));
    }

    #[test]
    fn the_sunken_king_always_drops_green_gear() {
        let mut w = World::new(16);
        let mut crowns = 0;
        for _ in 0..400 {
            let loot = w.roll_loot(MobKind::SunkenKing, 10, vec![1]).unwrap();
            // Gear is the looter's own.
            let mine = &loot.personal[0].1;
            assert!(mine.iter().any(|(i, _)| RARE_DROPS.contains(i)));
            assert!(loot.items.contains(&(items::ANCIENT_CORE, 2)));
            if mine.contains(&(items::CROWN_OF_THE_SUNKEN_KING, 1)) {
                crowns += 1;
            }
        }
        // The crown drops a quarter of the time.
        assert!((70..130).contains(&crowns), "{crowns} crowns in 400 kills");
    }

    // ---- The Cinderforge and Frosthowl Cavern ----

    #[test]
    fn each_dungeon_needs_its_level_and_a_party_can_hold_one_of_each() {
        let mut w = World::new(17);
        let (a, b) = party_of_two(&mut w);
        let mut copies = Vec::new();
        for which in [DungeonId::Cinderforge, DungeonId::Frosthowl] {
            let d = which.get();
            for p in [a, b] {
                w.entities.get_mut(&p).unwrap().level = d.min_level - 1;
                at_waystone(&mut w, p);
                w.handle(p, ClientMsg::Travel(Destination::Dungeon(which)));
                assert!(
                    matches!(place(&w, p), Place::Zone(_)),
                    "too low for {}",
                    d.name
                );
                w.entities.get_mut(&p).unwrap().level = d.min_level;
                w.handle(p, ClientMsg::Travel(Destination::Dungeon(which)));
            }
            let Place::Dungeon(index) = place(&w, a) else {
                panic!("not in {}", d.name)
            };
            assert_eq!(place(&w, b), Place::Dungeon(index));
            assert_eq!(dungeon::of(index).id, which);
            // Every pack is there, the boss among them, and Joe offers this
            // dungeon's quest.
            let per_copy: usize = d
                .packs
                .iter()
                .flat_map(|p| p.mobs.iter().map(|m| m.2))
                .sum();
            let mobs = vault_mobs(&w, index);
            assert_eq!(mobs.len(), per_copy);
            assert!(
                mobs.iter()
                    .any(|m| w.entities[m].mob().unwrap().kind == d.boss())
            );
            let joe = w
                .entities
                .values()
                .find(|e| {
                    e.name == quests::VAULT_GIVER && Place::at(e.pos) == Place::Dungeon(index)
                })
                .expect("no quest giver");
            assert_eq!(
                quests::offered(Place::at(joe.pos))[0].id,
                quests::dungeon_quest(which).id
            );
            copies.push(index);
            // Back out to town, so the next dungeon can be entered.
            for p in [a, b] {
                w.entities.get_mut(&p).unwrap().pos = dungeon::to_world(index, dungeon::EXIT_STONE);
                w.handle(p, ClientMsg::Travel(Destination::Town(Zone::Amberfall)));
            }
        }
        // Both copies stay open, and going back finds the same one.
        assert_eq!(w.instances.len(), 2);
        at_waystone(&mut w, a);
        w.handle(
            a,
            ClientMsg::Travel(Destination::Dungeon(DungeonId::Cinderforge)),
        );
        assert_eq!(place(&w, a), Place::Dungeon(copies[0]));
    }

    #[test]
    fn gorraks_spells_can_be_interrupted_and_he_rages_when_low() {
        let mut w = World::new(18);
        let p = join(&mut w, "Rogue", Class::Rogue, 10);
        let boss = boss_fight_with(&mut w, p, MobKind::GorrakAshfist);
        w.entities.get_mut(&p).unwrap().player_mut().unwrap().god = true;
        let mut seen = Vec::new();
        for _ in 0..(40.0 / DT) as usize {
            w.tick(DT);
            let Some(cast) = w.entities[&boss].cast.as_ref().map(|c| c.ability) else {
                continue;
            };
            if !seen.contains(&cast) {
                seen.push(cast);
            }
            let bpos = w.entities[&boss].pos;
            let me = w.entities.get_mut(&p).unwrap();
            me.target = Some(boss);
            me.gcd = 0.0;
            me.cooldowns.clear();
            me.pos = ground(bpos.x - 2.5, bpos.z);
            me.yaw = yaw_towards(me.pos, bpos);
            me.power = me.max_power;
            w.try_use(p, ids::KICK).unwrap();
            assert!(
                w.entities[&boss].cast.is_none(),
                "{cast:?} wasn't interrupted"
            );
        }
        assert!(seen.contains(&ids::MOLTEN_BLAST), "{seen:?}");
        assert!(seen.contains(&ids::FLAME_WAVE), "{seen:?}");
        let raging = |w: &World| {
            w.entities[&boss]
                .auras
                .iter()
                .any(|a| a.ability == ids::BLOODRAGE)
        };
        assert!(!raging(&w), "not yet");
        let b = w.entities.get_mut(&boss).unwrap();
        b.hp = b.max_hp * 0.25;
        run(&mut w, 1.0);
        assert!(raging(&w));
    }

    #[test]
    fn hrimja_calls_her_pack_at_half_health_and_it_leaves_when_she_resets() {
        let mut w = World::new(19);
        let p = join(&mut w, "Tank", Class::Fighter, 10);
        w.entities.get_mut(&p).unwrap().level = 20;
        at_waystone(&mut w, p);
        w.handle(
            p,
            ClientMsg::Travel(Destination::Dungeon(DungeonId::Frosthowl)),
        );
        let Place::Dungeon(index) = place(&w, p) else {
            panic!("not in Frosthowl Cavern")
        };
        w.entities.get_mut(&p).unwrap().player_mut().unwrap().god = true;
        let boss = vault_mobs(&w, index)
            .into_iter()
            .find(|m| w.entities[m].mob().unwrap().kind == MobKind::Hrimja)
            .unwrap();
        sturdy(&mut w, boss);
        let bpos = w.entities[&boss].pos;
        w.entities.get_mut(&p).unwrap().pos = ground(bpos.x, bpos.z - 3.0);
        w.provoke(p, boss, 100.0);
        let pack = |w: &World| -> Vec<EntityId> {
            w.entities
                .values()
                .filter(|e| e.mob().is_some_and(|m| m.summoner == Some(boss)))
                .map(|e| e.id)
                .collect()
        };
        run(&mut w, 2.0);
        assert!(pack(&w).is_empty(), "not before half health");
        let b = w.entities.get_mut(&boss).unwrap();
        b.hp = b.max_hp * 0.45;
        run(&mut w, 0.5);
        let wolves = pack(&w);
        assert_eq!(wolves.len(), 3);
        for wolf in &wolves {
            let e = &w.entities[wolf];
            let m = e.mob().unwrap();
            assert_eq!(m.kind, MobKind::FrostfangWolf);
            assert_eq!(m.state, MobState::Combat);
            assert!(m.threat.iter().any(|(t, _)| *t == p));
            assert_eq!(Place::at(e.pos), Place::Dungeon(index));
        }
        // Only once per fight.
        run(&mut w, 2.0);
        assert_eq!(pack(&w).len(), 3);
        // The player gets away: she resets and her pack is gone.
        w.entities.get_mut(&p).unwrap().dead = true;
        run(&mut w, 1.0);
        assert!(pack(&w).is_empty());
        assert_eq!(w.entities[&boss].hp, w.entities[&boss].max_hp);
    }

    #[test]
    fn avalanche_chases_you_three_times() {
        let mut w = World::new(20);
        let p = join(&mut w, "Tank", Class::Fighter, 10);
        let boss = boss_fight_with(&mut w, p, MobKind::Hrimja);
        let h = &boss::kit(MobKind::Hrimja).hazard;
        assert_eq!(h.times, 3);
        let calm = |w: &mut World| {
            let b = w.entities.get_mut(&boss).unwrap();
            b.swing_timer = 10.0;
            b.cast = None;
            let e = w.entities.get_mut(&p).unwrap();
            e.hp = e.max_hp;
        };
        for _ in 0..(10.0 / DT) as usize {
            calm(&mut w);
            w.tick(DT);
            if !w.hazards.is_empty() {
                break;
            }
        }
        assert_eq!(w.hazards.len(), 1, "no Avalanche");
        for k in 0..h.times {
            // Step aside each time: the next one follows.
            let e = w.entities.get_mut(&p).unwrap();
            e.pos = ground(e.pos.x + 6.0, e.pos.z);
            for _ in 0..(h.warning / DT) as usize + 2 {
                calm(&mut w);
                w.tick(DT);
            }
            if k + 1 < h.times {
                let next = w.hazards.first().expect("Avalanche stopped early");
                assert!(next.pos.distance(w.entities[&p].pos) < 0.5);
            }
        }
        assert!(w.hazards.is_empty());
    }

    #[test]
    fn dropped_items_are_the_droppers_for_five_seconds() {
        let mut w = World::new(81);
        let a = join(&mut w, "Dropper", Class::Fighter, 3);
        let b = join(&mut w, "Grabber", Class::Fighter, 3);
        let at = w.entities[&a].pos;
        w.entities.get_mut(&b).unwrap().pos = at;
        let pa = w.entities.get_mut(&a).unwrap().player_mut().unwrap();
        pa.bags[59] = Some((items::LIGHT_LEATHER, 7));
        w.handle(a, ClientMsg::DropItem(59));
        assert_eq!(w.entities[&a].player().unwrap().bags[59], None);
        let g = w.ground[0].id;
        assert_eq!(w.snapshot_for(b).unwrap().ground[0].locked, DROP_PROTECTION);
        assert_eq!(w.snapshot_for(a).unwrap().ground[0].locked, 0.0);

        // Too soon for anyone else.
        w.handle(b, ClientMsg::PickUp(g));
        assert_eq!(w.ground.len(), 1);
        run(&mut w, DROP_PROTECTION + 0.1);
        w.handle(b, ClientMsg::PickUp(g));
        assert!(w.ground.is_empty());
        let bags = &w.entities[&b].player().unwrap().bags;
        assert_eq!(count_item(bags, items::LIGHT_LEATHER), 7);
    }

    #[test]
    fn the_dropper_can_take_it_back_and_the_ground_clears() {
        let mut w = World::new(82);
        let a = join(&mut w, "Dropper", Class::Fighter, 3);
        let pa = w.entities.get_mut(&a).unwrap().player_mut().unwrap();
        pa.bags[0] = Some((items::LIGHT_LEATHER, 2));
        pa.bags[1] = Some((items::LIGHT_LEATHER, 3));
        w.handle(a, ClientMsg::DropItem(0));
        w.handle(a, ClientMsg::DropItem(1));
        w.handle(a, ClientMsg::DropItem(2)); // empty: nothing happens
        assert_eq!(w.ground.len(), 2);
        let g = w.ground[0].id;
        w.handle(a, ClientMsg::PickUp(g));
        assert_eq!(w.ground.len(), 1);
        let bags = &w.entities[&a].player().unwrap().bags;
        assert_eq!(count_item(bags, items::LIGHT_LEATHER), 2);
        run(&mut w, DROP_LIFETIME + 0.1);
        assert!(w.ground.is_empty());
    }

    #[test]
    fn emotes_tell_everyone_nearby_and_sitting_ends_on_moving() {
        let mut w = World::new(83);
        let a = join(&mut w, "Waver", Class::Bard, 3);
        let b = join(&mut w, "Friend", Class::Bard, 3);
        w.entities.get_mut(&a).unwrap().target = Some(b);
        w.drain_outbox();
        w.handle(a, ClientMsg::Chat("/wave".into()));
        let said: Vec<String> = w
            .drain_outbox()
            .into_iter()
            .filter_map(|(to, m)| match (to, m) {
                (Audience::Near(_), ServerMsg::Event(GameEvent::Emote { who, text, .. })) => {
                    assert_eq!(who, a);
                    Some(text)
                }
                _ => None,
            })
            .collect();
        assert_eq!(said, vec!["Waver waves at Friend.".to_string()]);
        let view = w.snapshot_for(b).unwrap();
        let seen = view.entities.iter().find(|e| e.id == a).unwrap();
        assert_eq!(seen.emote.map(|e| e.emote), Some(Emote::Wave));
        run(&mut w, 3.0);
        assert_eq!(w.entities[&a].emote, None);

        w.handle(a, ClientMsg::Chat("/sit".into()));
        run(&mut w, 30.0);
        assert_eq!(w.entities[&a].emote.map(|e| e.0), Some(Emote::Sit));
        let pos = w.entities[&a].pos + Vec3::X;
        w.handle(
            a,
            ClientMsg::Move {
                pos,
                yaw: 0.0,
                moving: true,
            },
        );
        run(&mut w, 0.1);
        assert_eq!(w.entities[&a].emote, None);
    }

    #[test]
    fn dungeon_bosses_have_double_health_and_their_mobs_a_quarter_more() {
        for kind in MobKind::DUNGEON {
            let listed = kind.template().hp;
            let want = if kind.template().boss { 2.0 } else { 1.25 };
            assert_eq!(kind.max_hp(1), listed * want, "{kind:?}");
        }
        assert_eq!(MobKind::Boar.max_hp(1), MobKind::Boar.template().hp);
        let bosses = MobKind::DUNGEON
            .iter()
            .filter(|k| k.template().boss)
            .count();
        assert_eq!(bosses, shared::dungeon::DungeonId::ALL.len());
    }

    #[test]
    fn spiders_web_you_on_about_a_quarter_of_their_hits() {
        let mut w = World::new(84);
        let p = join(&mut w, "Prey", Class::Fighter, 3);
        w.entities.get_mut(&p).unwrap().player_mut().unwrap().god = true;
        let spider = engage(&mut w, p, MobKind::CaveSpider);
        sturdy(&mut w, spider);
        w.handle(p, ClientMsg::StopAttack);
        w.handle(
            p,
            ClientMsg::UseAbility {
                ability: ids::HEROIC_STRIKE,
                target: Some(spider),
            },
        );
        w.drain_outbox();
        let (mut hits, mut webs) = (0, 0);
        for _ in 0..(200.0 / DT) as usize {
            w.tick(DT);
            for (_, m) in w.drain_outbox() {
                match m {
                    ServerMsg::Event(GameEvent::Damage {
                        source,
                        target,
                        ability: None,
                        ..
                    }) if source == spider && target == p => hits += 1,
                    ServerMsg::Event(GameEvent::AbilityUsed {
                        caster, ability, ..
                    }) if caster == spider && ability == ids::WEB => webs += 1,
                    _ => {}
                }
            }
        }
        assert!(hits >= 60, "{hits} hits");
        let rate = webs as f32 / hits as f32;
        assert!((0.12..0.4).contains(&rate), "{webs} webs in {hits} hits");
    }
}
