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
use shared::props::MERCHANT_SPOT;

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
    /// Sandbox god mode: no damage taken, abilities are free.
    pub god: bool,
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
    pub loot: Option<Loot>,
    /// Who summoned this mob in sandbox mode; it doesn't come back.
    pub summoner: Option<EntityId>,
}

/// A townsperson who trades.
pub struct NpcData {
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
            Brain::Npc(n) => EntityKind::Merchant(n.race),
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
        let (appearance, gear) = match &self.brain {
            Brain::Player(p) => (p.appearance, p.gear),
            Brain::Mob(_) => (Appearance::default(), [None; 5]),
            Brain::Npc(n) => (
                Appearance {
                    race: n.race,
                    body: (self.id % 2) as u8,
                    skin: (self.id % 5) as u8,
                    hair_style: 1 + (self.id % 4) as u8,
                    hair_color: (self.id % 6) as u8,
                },
                [None; 5],
            ),
        };
        let lootable = self
            .mob()
            .and_then(|m| m.loot.as_ref())
            .is_some_and(|l| l.looters.contains(&viewer));
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
            lootable,
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
        };
        for camp in 0..world.camps.len() {
            for _ in 0..world.camps[camp].count {
                world.spawn_mob(camp);
            }
        }
        for zone in Zone::ALL {
            world.spawn_merchant(zone);
        }
        world
    }

    /// A merchant in a zone's town, facing the square.
    fn spawn_merchant(&mut self, zone: Zone) -> EntityId {
        let id = self.alloc_id();
        let pos = zone.ground_local(MERCHANT_SPOT);
        let center = zone.ground_local(Vec2::ZERO);
        let yaw = yaw_towards(pos, center);
        self.entities.insert(
            id,
            Entity {
                id,
                name: zone.merchant_name().to_string(),
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
                brain: Brain::Npc(NpcData {
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
        let pos = clamp_to_world(Vec3::from(c.pos));
        let stats = gear_stats(&c.gear);
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
                brain: Brain::Player(PlayerData {
                    account: c.account.clone(),
                    class: c.class,
                    appearance: c.appearance,
                    xp: c.xp,
                    money: c.money,
                    bags: c.bags.clone(),
                    gear: c.gear,
                    stats,
                    combo_points: 0,
                    auto_attack: false,
                    move_budget: 0.0,
                    since_spend: MANA_REGEN_DELAY,
                    potion_cooldown: 0.0,
                    talents: ranks,
                    bonuses,
                    god: false,
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
        Some(Character {
            account: p.account.clone(),
            name: e.name.clone(),
            class: p.class,
            appearance: p.appearance,
            level: e.level,
            xp: p.xp,
            money: p.money,
            pos: e.pos.to_array(),
            yaw: e.yaw,
            bags: p.bags.clone(),
            gear: p.gear,
            talents: p.talents,
        })
    }

    /// Takes a player out of the world, returning their character to save.
    pub fn remove_player(&mut self, id: EntityId) -> Option<Character> {
        let mut c = self.character(id)?;
        self.clear_spawns(id);
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
        if text.starts_with('/') {
            self.send(
                Audience::Only(id),
                GameEvent::System("Commands: /who".into()),
            );
            return;
        }
        let from = self.entities[&id].name.clone();
        self.send(Audience::Everyone, GameEvent::Chat { from, text });
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
        e.pos = Zone::at(e.pos).graveyard();
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
            .is_some_and(|l| l.looters.contains(&id));
        if !c.dead || !allowed {
            return Err("There's nothing to loot.");
        }
        if c.pos.distance(me.pos) > LOOT_RANGE + 1.0 {
            return Err("You are too far away.");
        }
        let loot = mob_of(&mut self.entities.get_mut(&corpse).unwrap().brain)
            .loot
            .take()
            .unwrap();
        let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
        p.money += loot.money;
        let mut got = Vec::new();
        let mut left = Vec::new();
        for (item, n) in loot.items {
            let rest = add_item(&mut p.bags, item, n);
            if rest < n {
                got.push((item, n - rest));
            }
            if rest > 0 {
                left.push((item, rest));
            }
        }
        if !left.is_empty() {
            mob_of(&mut self.entities.get_mut(&corpse).unwrap().brain).loot = Some(Loot {
                money: 0,
                items: left,
                looters: loot.looters,
            });
            self.error(id, "Your bags are full.");
        }
        self.send(
            Audience::Only(id),
            GameEvent::Looted {
                money: loot.money,
                items: got,
            },
        );
        Ok(())
    }

    /// Recomputes stats and health after gear changes.
    fn refresh_stats(&mut self, id: EntityId) {
        let e = self.entities.get_mut(&id).unwrap();
        let level = e.level;
        let p = e.player_mut().unwrap();
        p.stats = gear_stats(&p.gear);
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
        let ItemKind::Armor { slot, .. } = item(item_id).kind else {
            return Err("You can't wear that.");
        };
        // Swap: what you were wearing goes where the new piece was.
        let old = p.gear[slot.index()].replace(item_id);
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
                let pos = zone.graveyard();
                let e = self.entities.get_mut(&id).unwrap();
                e.pos = pos;
                e.cast = None;
                e.target = None;
                e.player_mut().unwrap().auto_attack = false;
                let yaw = e.yaw;
                self.forget(id);
                self.outbox
                    .push((Audience::Only(id), ServerMsg::SetPosition { pos, yaw }));
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

    fn merchant_near(&self, id: EntityId, merchant: EntityId) -> Result<(), &'static str> {
        let me = &self.entities[&id];
        let m = self
            .entities
            .get(&merchant)
            .ok_or("There's no one to trade with.")?;
        if !matches!(m.brain, Brain::Npc(_)) {
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
        let ItemKind::Potion { health, power } = item(item_id).kind else {
            return Err("You can't use that.");
        };
        if p.potion_cooldown > 0.0 {
            return Err("Potions are not ready yet.");
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
        let a = ABILITIES.get(id.0 as usize).ok_or("Unknown ability.")?;
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
        let pos = if Zone::at(pos) == Zone::at(e.pos) {
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
        let (camp, pos, social) = {
            let e = &self.entities[&mob];
            let m = e.mob().unwrap();
            (m.camp, e.pos, m.kind.template().social)
        };
        if !social {
            return;
        }
        for e in self.entities.values_mut() {
            if e.dead || e.id == mob || e.pos.distance(pos) > 8.0 {
                continue;
            }
            if let Some(m) = e.mob_mut()
                && m.camp == camp
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
            if self.rng.chance(chance) {
                items.push((item, self.rng.int(min as u32, max as u32) as u16));
            }
        }
        (!looters.is_empty()).then_some(Loot {
            money,
            items,
            looters,
        })
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
        // Everyone who fought the mob shares the kill and may loot it.
        let players: Vec<EntityId> = fighters
            .into_iter()
            .filter(|id| self.entities.get(id).is_some_and(|e| e.player().is_some()))
            .collect();
        let loot = self.roll_loot(kind, level, players.clone());
        mob_of(&mut self.entities.get_mut(&victim).unwrap().brain).loot = loot;
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
        for (ability, unlock) in class.abilities().into_iter().zip(UNLOCK_LEVELS) {
            if unlock > old_level && unlock <= level {
                self.send(Audience::Only(id), GameEvent::Learned(ability));
            }
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.tick += 1;
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
            let (amount, crit) = self.roll_with(aa.min, aa.max, scale, bonus.crit);
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
            e.cast = None;
            e.hp = e.max_hp;
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

        if let Some((spell, interval)) = t.spell {
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
        }
    }
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
        for e in w.entities.values().filter(|e| e.mob().is_some()) {
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
        run(&mut w, 2.6);
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
            run(&mut w, 3.0);
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
        run(&mut w, 2.6);
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
            assert!(mobs.iter().all(|m| kinds.contains(&m.mob().unwrap().kind)));
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
        run(&mut w, 2.1);
        assert!(
            w.entities[&p]
                .auras
                .iter()
                .any(|a| a.ability == ids::FROSTBOLT)
        );
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
}
