//! The authoritative game world: players, mobs, combat and AI.
//!
//! The network layer feeds `handle` with client messages, calls `tick` at a
//! fixed rate, sends each player `snapshot_for`, and delivers whatever
//! `drain_outbox` returns.

use std::collections::{BTreeMap, HashMap};

use glam::{Vec2, Vec3, vec2};
use shared::data::*;
use shared::protocol::*;
use shared::world::*;

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
}

pub struct Cast {
    pub ability: AbilityId,
    pub target: Option<EntityId>,
    pub elapsed: f32,
    pub total: f32,
}

pub struct PlayerData {
    pub class: Class,
    pub xp: u32,
    pub auto_attack: bool,
    /// How far the player may still move; refills at run speed.
    move_budget: f32,
    /// Seconds since mana was last spent.
    since_spend: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobState {
    Idle,
    Combat,
    /// Running home after a chase went too far.
    Evading,
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
}

pub enum Brain {
    Player(PlayerData),
    Mob(MobData),
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
        }
    }

    pub fn player(&self) -> Option<&PlayerData> {
        match &self.brain {
            Brain::Player(p) => Some(p),
            Brain::Mob(_) => None,
        }
    }

    fn player_mut(&mut self) -> Option<&mut PlayerData> {
        match &mut self.brain {
            Brain::Player(p) => Some(p),
            Brain::Mob(_) => None,
        }
    }

    pub fn mob(&self) -> Option<&MobData> {
        match &self.brain {
            Brain::Mob(m) => Some(m),
            Brain::Player(_) => None,
        }
    }

    fn mob_mut(&mut self) -> Option<&mut MobData> {
        match &mut self.brain {
            Brain::Mob(m) => Some(m),
            Brain::Player(_) => None,
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
    fn slow(&self) -> f32 {
        self.auras
            .iter()
            .filter_map(|a| match a.kind {
                AuraKind::Slow(f) => Some(f),
                _ => None,
            })
            .fold(1.0, f32::min)
    }

    fn evading(&self) -> bool {
        self.mob().is_some_and(|m| m.state == MobState::Evading)
    }

    /// Melee reach, counting the size of big mobs.
    fn reach(&self) -> f32 {
        self.mob().map_or(0.0, |m| m.kind.template().size * 0.5)
    }

    fn view(&self) -> EntityView {
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
                })
                .collect(),
            dead: self.dead,
            in_combat: self.in_combat,
            moving: self.moving,
        }
    }
}

struct Camp {
    center: Vec2,
    radius: f32,
    kind: MobKind,
    levels: (u8, u8),
    count: usize,
}

const fn camp(x: f32, z: f32, radius: f32, kind: MobKind, levels: (u8, u8), count: usize) -> Camp {
    Camp {
        center: Vec2::new(x, z),
        radius,
        kind,
        levels,
        count,
    }
}

/// Mob camps. Levels rise the further you go from town.
fn camps() -> Vec<Camp> {
    use MobKind::*;
    vec![
        camp(50.0, 25.0, 14.0, Wolf, (1, 2), 5),
        camp(-45.0, 40.0, 16.0, Boar, (1, 3), 6),
        camp(15.0, -60.0, 14.0, Wolf, (2, 3), 5),
        camp(-85.0, -35.0, 16.0, Boar, (3, 5), 6),
        camp(90.0, -75.0, 14.0, Bandit, (4, 5), 4),
        camp(95.0, -70.0, 12.0, BanditMystic, (4, 5), 2),
        camp(20.0, 110.0, 16.0, Wolf, (5, 6), 6),
        camp(120.0, 90.0, 14.0, Bandit, (6, 8), 5),
        camp(125.0, 95.0, 12.0, BanditMystic, (6, 8), 3),
        camp(155.0, -20.0, 16.0, Wolf, (7, 8), 6),
        camp(-120.0, -120.0, 16.0, Bandit, (8, 9), 5),
        camp(-150.0, -150.0, 4.0, Golem, (10, 10), 1),
    ]
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
        };
        for camp in 0..world.camps.len() {
            for _ in 0..world.camps[camp].count {
                world.spawn_mob(camp);
            }
        }
        world
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
            camp.levels.0 + (self.rng.f32() * (camp.levels.1 - camp.levels.0 + 1) as f32) as u8;
        let level = level.min(camp.levels.1);
        let kind = m.kind;
        let t = kind.template();
        m.home = ground(p.x, p.y);
        m.threat.clear();
        m.state = MobState::Idle;
        m.wander_timer = self.rng.range(2.0, 8.0);
        m.wander_to = None;
        m.spell_timer = self.rng.range(1.0, 3.0);
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

    /// Adds a player and returns their id. Names are cleaned up and made unique.
    pub fn add_player(&mut self, name: &str, class: Class) -> EntityId {
        let name = self.unique_name(name);
        let id = self.alloc_id();
        let pos = ground(GRAVEYARD.x, GRAVEYARD.y);
        let max_hp = class.max_hp(1);
        let max_power = class.max_power(1);
        self.entities.insert(
            id,
            Entity {
                id,
                name: name.clone(),
                level: 1,
                pos,
                yaw: 0.0,
                hp: max_hp,
                max_hp,
                power: if class.power_kind() == PowerKind::Rage {
                    0.0
                } else {
                    max_power
                },
                max_power,
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
                    class,
                    xp: 0,
                    auto_attack: false,
                    move_budget: 0.0,
                    since_spend: MANA_REGEN_DELAY,
                }),
            },
        );
        self.send(
            Audience::Everyone,
            GameEvent::System(format!("{name} has entered the world.")),
        );
        id
    }

    pub fn remove_player(&mut self, id: EntityId) {
        if let Some(e) = self.entities.remove(&id) {
            self.forget(id);
            self.send(
                Audience::Everyone,
                GameEvent::System(format!("{} has left the world.", e.name)),
            );
        }
    }

    fn unique_name(&self, raw: &str) -> String {
        let mut name: String = raw
            .chars()
            .filter(|c| c.is_alphanumeric())
            .take(12)
            .collect::<String>()
            .to_lowercase();
        if name.is_empty() {
            name = "adventurer".into();
        }
        let mut chars = name.chars();
        let first = chars.next().unwrap().to_uppercase().collect::<String>();
        let name = first + chars.as_str();
        let taken = |n: &str| {
            self.entities
                .values()
                .any(|e| e.player().is_some() && e.name == n)
        };
        if !taken(&name) {
            return name;
        }
        (2..)
            .map(|i| format!("{name}{i}"))
            .find(|n| !taken(n))
            .unwrap()
    }

    /// Removes all references to an entity that is gone or dead.
    fn forget(&mut self, id: EntityId) {
        for e in self.entities.values_mut() {
            if let Some(m) = e.mob_mut() {
                m.threat.retain(|(t, _)| *t != id);
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
            .map(Entity::view)
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
            },
        })
    }

    pub fn handle(&mut self, id: EntityId, msg: ClientMsg) {
        if !self.entities.contains_key(&id) {
            return;
        }
        match msg {
            ClientMsg::Hello { .. } => {}
            ClientMsg::Move { pos, yaw, moving } => self.handle_move(id, pos, yaw, moving),
            ClientMsg::SetTarget(target) => {
                let target = target.filter(|t| self.entities.contains_key(t));
                self.entities.get_mut(&id).unwrap().target = target;
            }
            ClientMsg::UseAbility { ability, target } => {
                if let Some(target) = target
                    && self.entities.contains_key(&target)
                {
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
        if e.kind() != EntityKind::Player(Class::Warrior) {
            e.power = e.max_power * 0.5;
        }
        e.pos = ground(GRAVEYARD.x, GRAVEYARD.y);
        e.yaw = 0.0;
        let (pos, yaw) = (e.pos, e.yaw);
        self.outbox
            .push((Audience::Only(id), ServerMsg::SetPosition { pos, yaw }));
    }

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
        if let Some(p) = e.player()
            && !p.class.abilities().contains(&id)
        {
            return Err("You don't know that ability.");
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
        let target = self.resolve_target(caster, a, false)?;
        let e = &self.entities[&caster];
        if e.power < a.cost {
            return Err(match e.player().map(|p| p.class.power_kind()) {
                Some(PowerKind::Rage) => "Not enough rage.",
                _ => "Not enough mana.",
            });
        }
        let e = self.entities.get_mut(&caster).unwrap();
        e.gcd = GCD;
        if a.cast_time > 0.0 {
            e.cast = Some(Cast {
                ability: id,
                target,
                elapsed: 0.0,
                total: a.cast_time,
            });
        } else {
            self.complete(caster, id, target);
        }
        // Warriors start swinging when they use an ability on an enemy.
        if a.targeting == Targeting::Enemy
            && let Some(p) = self.entities.get_mut(&caster).unwrap().player_mut()
            && p.class == Class::Warrior
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
            Targeting::Enemy => {
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

    /// An ability goes off: pay for it and apply its effects.
    fn complete(&mut self, caster: EntityId, id: AbilityId, target: Option<EntityId>) {
        let a = ability(id);
        let (pos, level) = {
            let e = self.entities.get_mut(&caster).unwrap();
            e.power -= a.cost;
            if a.cooldown > 0.0 {
                e.cooldowns.insert(id, (a.cooldown, a.cooldown));
            }
            if let Some(p) = e.player_mut()
                && a.cost > 0.0
            {
                p.since_spend = 0.0;
            }
            (e.pos, e.level)
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
        };
        let scale = level_scale(level);
        for t in targets {
            for effect in a.effects {
                if self.entities.get(&t).is_none_or(|e| e.dead) {
                    break;
                }
                match *effect {
                    Effect::Damage { min, max } => {
                        let crit = self.rng.chance(CRIT_CHANCE);
                        let amount = self.rng.range(min, max)
                            * scale
                            * if crit { CRIT_MULTIPLIER } else { 1.0 };
                        self.apply_hit(caster, t, Hit::Damage(amount, crit), Some(id));
                    }
                    Effect::Heal { min, max } => {
                        let crit = self.rng.chance(CRIT_CHANCE);
                        let amount = self.rng.range(min, max)
                            * scale
                            * if crit { CRIT_MULTIPLIER } else { 1.0 };
                        self.apply_hit(caster, t, Hit::Heal(amount, crit), Some(id));
                    }
                    Effect::Aura { kind, duration } => {
                        self.apply_aura(caster, t, id, kind, duration, scale)
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
                            if m.state == MobState::Idle {
                                m.state = MobState::Combat;
                            }
                        }
                    }
                    Effect::RestorePower(frac) => {
                        let e = self.entities.get_mut(&t).unwrap();
                        e.power = (e.power + e.max_power * frac).min(e.max_power);
                    }
                }
            }
        }
    }

    fn apply_aura(
        &mut self,
        source: EntityId,
        target: EntityId,
        ability: AbilityId,
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
        let t = self.entities.get_mut(&target).unwrap();
        // Reapplying refreshes rather than stacks.
        t.auras
            .retain(|a| !(a.ability == ability && a.source == source));
        t.auras.push(Aura {
            ability,
            kind,
            duration,
            remaining: duration,
            tick_timer: interval,
            source,
        });
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

    /// `source` did something hostile to `target`: both enter combat.
    fn provoke(&mut self, source: EntityId, target: EntityId, threat: f32) {
        let Some(t) = self.entities.get_mut(&target) else {
            return;
        };
        if let Some(m) = t.mob_mut() {
            let first = m.state == MobState::Idle;
            add_threat(m, source, threat);
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
        ability: Option<AbilityId>,
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
                if t.evading() {
                    self.send(Audience::Near(pos), GameEvent::Evade { target });
                    return;
                }
                let t = self.entities.get_mut(&target).unwrap();
                let mut left = amount.round();
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
                if let Some(p) = t.player()
                    && p.class == Class::Warrior
                {
                    t.power = (t.power + left / level_scale(target_level) * 0.5).min(t.max_power);
                }
                let dead = t.hp <= 0.0;
                if let Some(s) = self.entities.get_mut(&source) {
                    let melee = ability.is_none();
                    if s.player().is_some_and(|p| p.class == Class::Warrior) && melee {
                        s.power = (s.power + amount / level_scale(s.level) * 1.2).min(s.max_power);
                    }
                }
                self.send(
                    Audience::Near(pos),
                    GameEvent::Damage {
                        source,
                        target,
                        amount: left as u32,
                        absorbed: absorbed as u32,
                        crit,
                        ability,
                    },
                );
                let mult = ability.map_or(1.0, |a| self::ability(a).threat);
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
                        ability: ability.unwrap_or(shared::data::ids::HEAL),
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
        let mut rewards = Vec::new();
        match &mut e.brain {
            Brain::Player(p) => p.auto_attack = false,
            Brain::Mob(m) => {
                let t = m.kind.template();
                m.respawn_timer = t.respawn;
                m.state = MobState::Idle;
                rewards = m
                    .threat
                    .drain(..)
                    .map(|(id, _)| (id, t.elite, t.name))
                    .collect();
            }
        }
        self.send(Audience::Near(pos), GameEvent::Died { id: victim, killer });
        self.forget(victim);
        // Everyone who fought the mob shares the kill.
        for (player, elite, name) in rewards {
            let Some(p) = self.entities.get(&player) else {
                continue;
            };
            let xp = kill_xp(p.level, level, elite);
            if xp > 0 {
                self.give_xp(player, xp, name);
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
        let mut leveled = false;
        while e.level < MAX_LEVEL && p.xp >= xp_to_next(e.level) {
            p.xp -= xp_to_next(e.level);
            e.level += 1;
            leveled = true;
        }
        if e.level >= MAX_LEVEL {
            p.xp = 0;
        }
        if leveled {
            e.max_hp = class.max_hp(e.level);
            e.max_power = class.max_power(e.level);
            e.hp = e.max_hp;
            if class.power_kind() == PowerKind::Mana {
                e.power = e.max_power;
            }
            let (pos, level) = (e.pos, e.level);
            self.send(Audience::Near(pos), GameEvent::LevelUp { id, level });
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.tick += 1;
        let ids: Vec<EntityId> = self.entities.keys().copied().collect();
        for &id in &ids {
            self.tick_timers(id, dt);
            self.tick_auras(id, dt);
            self.tick_cast(id, dt);
            if self.entities.get(&id).is_some_and(|e| e.player().is_some()) {
                self.tick_player(id, dt);
            } else {
                self.tick_mob(id, dt);
            }
        }
        // Players are in combat while any mob wants to hit them.
        let mut fighting = std::collections::HashSet::new();
        for e in self.entities.values() {
            if let Some(m) = e.mob()
                && !e.dead
                && m.state == MobState::Combat
            {
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
                    let hit = if matches!(a.kind, AuraKind::Dot { .. }) {
                        Hit::Damage(per_tick, false)
                    } else {
                        Hit::Heal(per_tick, false)
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
        match a.targeting {
            Targeting::Enemy => match self.resolve_target(id, a, true) {
                Ok(t) if t == target => self.complete(id, aid, target),
                Ok(_) | Err(_) => {
                    let still_there = target
                        .and_then(|t| self.entities.get(&t))
                        .is_some_and(|t| !t.dead);
                    let e = &self.entities[&id];
                    let in_range =
                        target.is_some_and(|t| e.pos.distance(self.pos_of(t)) <= a.range + 3.0);
                    if still_there && in_range {
                        self.complete(id, aid, target);
                    } else {
                        self.error(id, "Your target is no longer valid.");
                    }
                }
            },
            Targeting::Friendly => {
                let ok = target.and_then(|t| self.entities.get(&t)).is_some_and(|t| {
                    !t.dead && t.pos.distance(self.entities[&id].pos) <= a.range + 3.0
                });
                if ok {
                    self.complete(id, aid, target);
                } else {
                    self.error(id, "Your target is no longer valid.");
                }
            }
            _ => self.complete(id, aid, target),
        }
    }

    fn tick_player(&mut self, id: EntityId, dt: f32) {
        let e = self.entities.get_mut(&id).unwrap();
        let speed = RUN_SPEED * e.slow();
        let in_combat = e.in_combat;
        let p = e.player_mut().unwrap();
        p.move_budget = (p.move_budget + speed * 1.25 * dt).min(speed);
        p.since_spend += dt;
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
                e.power = (e.power + e.max_power * rate * dt).min(e.max_power);
            }
            PowerKind::Rage => {
                if !in_combat {
                    e.power = (e.power - 4.0 * dt).max(0.0);
                }
            }
        }
        // Auto attack.
        let p = e.player().unwrap();
        if !p.auto_attack {
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
            let scale = level_scale(e.level);
            let crit = self.rng.chance(CRIT_CHANCE);
            let amount =
                self.rng.range(aa.min, aa.max) * scale * if crit { CRIT_MULTIPLIER } else { 1.0 };
            self.entities.get_mut(&id).unwrap().swing_timer = aa.interval;
            self.apply_hit(id, target, Hit::Damage(amount, crit), None);
        }
    }

    fn tick_mob(&mut self, id: EntityId, dt: f32) {
        let Some(e) = self.entities.get_mut(&id) else {
            return;
        };
        let m = mob_of(&mut e.brain);
        let t = m.kind.template();
        if e.dead {
            m.respawn_timer -= dt;
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
                e.hp = e.max_hp;
                e.target = None;
                e.cast = None;
                let home = m.home;
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
                .filter(|p| p.player().is_some() && !p.dead)
                .filter(|p| {
                    let radius = (10.0 + (e.level as f32 - p.level as f32) * 1.5).clamp(5.0, 20.0);
                    p.pos.distance(e.pos) <= radius
                        && (p.pos.x * p.pos.x + p.pos.z * p.pos.z).sqrt() > TOWN_RADIUS
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
        let m = mob_of(&mut e.brain);
        let speed = m.kind.template().speed * 0.35;
        let radius = self.camps[m.camp].radius * 0.5;
        if let Some(to) = m.wander_to {
            if move_towards(e, to, speed * e.slow() * dt) {
                let m = mob_of(&mut e.brain);
                m.wander_to = None;
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
                .filter(|t| {
                    ents.get(t).is_some_and(|p| {
                        !p.dead
                            && (p.pos.x * p.pos.x + p.pos.z * p.pos.z).sqrt() > TOWN_RADIUS - 4.0
                    })
                })
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
                // Casting mobs fight from range; the rest still close in.
                if self.try_use(id, spell).is_ok() {
                    return;
                }
            }
        }

        let e = self.entities.get_mut(&id).unwrap();
        let reach = MELEE_RANGE * 0.7 + t.size * 0.5;
        if dist > reach && !e.rooted() {
            let step = t.speed * e.slow() * dt;
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

/// The mob half of an entity, borrowed separately from its other fields.
fn mob_of(brain: &mut Brain) -> &mut MobData {
    match brain {
        Brain::Mob(m) => m,
        Brain::Player(_) => panic!("not a mob"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::data::ids;

    const DT: f32 = 0.05;

    fn run(world: &mut World, seconds: f32) {
        for _ in 0..(seconds / DT) as usize {
            world.tick(DT);
        }
    }

    /// The nearest living mob of a kind, and moves the player next to it, facing it.
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
    fn names_are_cleaned_and_unique() {
        let mut w = World::new(1);
        let a = w.add_player("  bob!! ", Class::Warrior);
        let b = w.add_player("BOB", Class::Mage);
        let c = w.add_player("", Class::Cleric);
        assert_eq!(w.entities[&a].name, "Bob");
        assert_eq!(w.entities[&b].name, "Bob2");
        assert_eq!(w.entities[&c].name, "Adventurer");
    }

    #[test]
    fn warrior_kills_a_boar_and_gains_xp() {
        let mut w = World::new(7);
        let p = w.add_player("Tank", Class::Warrior);
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
        let outbox = w.drain_outbox();
        assert!(
            outbox
                .iter()
                .any(|(_, m)| matches!(m, ServerMsg::Event(GameEvent::Died { .. })))
        );
    }

    #[test]
    fn neutral_mobs_ignore_passers_by_and_wolves_do_not() {
        let mut w = World::new(3);
        let p = w.add_player("Walker", Class::Mage);
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
        let mut w = World::new(29);
        let p = w.add_player("Puller", Class::Mage);
        let fighting = |w: &World| {
            w.entities
                .values()
                .filter(|e| e.mob().is_some_and(|m| m.state == MobState::Combat))
                .count()
        };
        let wolf = engage(&mut w, p, MobKind::Wolf);
        w.provoke(p, wolf, 1.0);
        assert_eq!(fighting(&w), 1);

        let mut w = World::new(29);
        let p = w.add_player("Puller", Class::Mage);
        // Pull a bandit while standing far enough away that only help-calls can add more.
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
        let p = w.add_player("Solo", class);
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
        for _ in 0..(90.0 / DT) as usize {
            if w.entities[&mob].dead || w.entities[&p].dead {
                break;
            }
            let mpos = w.entities[&mob].pos;
            let e = w.entities.get_mut(&p).unwrap();
            e.yaw = yaw_towards(e.pos, mpos);
            if class == Class::Warrior {
                e.pos = ground(mpos.x - 2.0, mpos.z);
                e.yaw = yaw_towards(e.pos, mpos);
            }
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
    fn every_class_can_solo_a_mob_of_its_level() {
        use ids::*;
        for kind in [MobKind::Wolf, MobKind::Boar] {
            assert_eq!(
                solo(Class::Warrior, kind, &[REND, HEROIC_STRIKE]),
                (true, true),
                "warrior vs {kind:?}"
            );
            assert_eq!(
                solo(Class::Mage, kind, &[FIRE_BLAST, FIREBALL]),
                (true, true),
                "mage vs {kind:?}"
            );
            assert_eq!(
                solo(Class::Cleric, kind, &[SHADOW_WORD_PAIN, SMITE]),
                (true, true),
                "cleric vs {kind:?}"
            );
        }
    }

    #[test]
    fn casts_take_time_and_moving_interrupts() {
        let mut w = World::new(5);
        let p = w.add_player("Caster", Class::Mage);
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
        let bpos = w.entities[&boar].pos;
        let e = w.entities.get_mut(&p).unwrap();
        e.yaw = yaw_towards(e.pos, bpos);
        w.try_use(p, ids::FIREBALL).unwrap();
        run(&mut w, 2.6);
        assert!(w.entities[&boar].hp < hp);
        assert!(w.entities[&boar].mob().unwrap().state == MobState::Combat);
    }

    #[test]
    fn range_facing_and_cooldowns_are_checked() {
        let mut w = World::new(9);
        let p = w.add_player("Checker", Class::Warrior);
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
        let e = w.entities.get_mut(&p).unwrap();
        e.yaw = yaw_towards(e.pos, bpos);
        w.try_use(p, ids::SHIELD_BASH).unwrap();
        assert_eq!(
            w.try_use(p, ids::HEROIC_STRIKE),
            Err("Ability is not ready yet.")
        );
        run(&mut w, GCD + 0.1);
        assert_eq!(
            w.try_use(p, ids::SHIELD_BASH),
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
        let p = w.add_player("Kiter", Class::Mage);
        let wolf = engage(&mut w, p, MobKind::Wolf);
        run(&mut w, 0.2);
        assert_eq!(w.entities[&wolf].mob().unwrap().state, MobState::Combat);
        w.entities.get_mut(&wolf).unwrap().hp = 1.0;
        // Teleport the wolf far from home, as if it chased the player a long way.
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
        let healer = w.add_player("Healer", Class::Cleric);
        let tank = w.add_player("Tank", Class::Warrior);
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
        // No friendly target: heal yourself.
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
        assert_eq!(t.max_hp, Class::Warrior.max_hp(t.level));
    }

    #[test]
    fn speed_hacks_are_rejected() {
        let mut w = World::new(17);
        let p = w.add_player("Runner", Class::Mage);
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
        let p = w.add_player("Unlucky", Class::Mage);
        let golem = engage(&mut w, p, MobKind::Golem);
        w.handle(p, ClientMsg::StartAttack);
        run(&mut w, 60.0);
        assert!(w.entities[&p].dead);
        // The golem goes home once its only enemy is dead.
        run(&mut w, 10.0);
        assert_eq!(w.entities[&golem].mob().unwrap().state, MobState::Idle);
        w.handle(p, ClientMsg::ReleaseSpirit);
        let e = &w.entities[&p];
        assert!(!e.dead);
        assert_eq!(flat_distance(e.pos, ground(GRAVEYARD.x, GRAVEYARD.y)), 0.0);
    }

    #[test]
    fn snapshots_only_include_nearby_entities() {
        let mut w = World::new(23);
        let p = w.add_player("Viewer", Class::Cleric);
        let snap = w.snapshot_for(p).unwrap();
        assert!(snap.entities.iter().any(|e| e.id == p));
        assert!(snap.entities.len() < w.entities.len());
        assert!(
            snap.entities
                .iter()
                .all(|e| e.pos.distance(w.entities[&p].pos) <= VIEW_DISTANCE)
        );
    }
}
