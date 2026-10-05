//! Dungeon boss mechanics. Every boss marks the ground under everyone
//! fighting it (get out before it lands), and casts two spells worth
//! interrupting. Some have a trick of their own at low health.
//!
//! - Morvane the Sunken King: Tidal Crash comes down twice in a row, the
//!   second where you stand when the first lands. Drowning Grasp is a heavy
//!   hit on his target; Call of the Deep heals him.
//! - Warlord Gorrak Ashfist: Magma Rain, wide circles that land once.
//!   Molten Blast hits his target hard; Flame Wave burns everyone. Below 30%
//!   health he flies into a Bloodrage and hits half again as hard.
//! - Hrimja the Frostmother: Avalanche lands three times in a row, chasing
//!   you. Frozen Tomb hits and roots her target; Glacial Howl hits and slows
//!   everyone. At half health she calls three snow wolves to her side.

use super::*;

/// Marked ground: how often, how much warning, how many times in a row,
/// how wide and how hard (before scaling with the boss's level).
pub struct HazardKit {
    pub interval: f32,
    pub warning: f32,
    pub times: u8,
    pub radius: f32,
    pub damage: f32,
}

/// Everything a boss does on its own.
pub struct Kit {
    pub hazard: HazardKit,
    /// An interruptible spell at its target, and how often.
    pub hit: (AbilityId, f32),
    /// Its other interruptible spell, how often, and the health (fraction)
    /// below which it starts casting it.
    pub other: (AbilityId, f32, f32),
    /// A spell it casts once, when its health first drops below this.
    pub enrage: Option<(f32, AbilityId)>,
    /// Help it calls once, when its health first drops below this: what
    /// kind, and how many.
    pub pack: Option<(f32, MobKind, usize)>,
}

/// How much faster Tidal Crash comes than it first did (12 seconds apart
/// with 2.5 seconds' warning).
const CRASH_SPEED: f32 = 1.15;

const SUNKEN_KING: Kit = Kit {
    hazard: HazardKit {
        // Tidal Crash
        interval: 12.0 / CRASH_SPEED,
        warning: 2.5 / CRASH_SPEED,
        times: 2,
        radius: 4.0,
        damage: 30.0,
    },
    hit: (ids::DROWNING_GRASP, 14.0),
    other: (ids::CALL_OF_THE_DEEP, 25.0, 0.9),
    enrage: None,
    pack: None,
};

const GORRAK: Kit = Kit {
    hazard: HazardKit {
        // Magma Rain
        interval: 11.0,
        warning: 2.2,
        times: 1,
        radius: 5.0,
        damage: 35.0,
    },
    hit: (ids::MOLTEN_BLAST, 14.0),
    other: (ids::FLAME_WAVE, 22.0, 1.0),
    enrage: Some((0.3, ids::BLOODRAGE)),
    pack: None,
};

const HRIMJA: Kit = Kit {
    hazard: HazardKit {
        // Avalanche
        interval: 13.0,
        warning: 2.0,
        times: 3,
        radius: 3.5,
        damage: 32.0,
    },
    hit: (ids::FROZEN_TOMB, 15.0),
    other: (ids::GLACIAL_HOWL, 24.0, 1.0),
    enrage: None,
    pack: Some((0.5, MobKind::FrostfangWolf, 3)),
};

/// A boss's mechanics.
pub fn kit(kind: MobKind) -> &'static Kit {
    match kind {
        MobKind::GorrakAshfist => &GORRAK,
        MobKind::Hrimja => &HRIMJA,
        _ => &SUNKEN_KING,
    }
}

/// A boss's own timers, started when the fight starts.
#[derive(Clone, Copy, Debug)]
pub struct BossTimers {
    hazard: f32,
    hit: f32,
    other: f32,
    enraged: bool,
    called: bool,
}

impl Default for BossTimers {
    fn default() -> Self {
        // The first hazard comes quickly, so the fight opens with a dodge.
        BossTimers {
            hazard: 5.0,
            hit: 8.0,
            other: 15.0,
            enraged: false,
            called: false,
        }
    }
}

/// A circle of ground about to be hit.
pub struct Hazard {
    pub pos: Vec3,
    pub radius: f32,
    pub remaining: f32,
    pub total: f32,
    pub damage: f32,
    /// The boss that made it; its hazards vanish when it dies or resets.
    pub source: EntityId,
    /// The player it was aimed at, and how many more times it comes down
    /// on them after this.
    pub aimed_at: EntityId,
    pub again: u8,
}

impl World {
    /// Runs a boss's mechanics during its fight. True if it started a cast
    /// (and does nothing else this tick).
    pub(super) fn boss_combat(&mut self, id: EntityId, dt: f32) -> bool {
        let e = self.entities.get_mut(&id).unwrap();
        let level = e.level;
        let health = e.hp / e.max_hp;
        let m = mob_of(&mut e.brain);
        let kit = kit(m.kind);
        let camp = m.camp;
        let fighting: Vec<EntityId> = m.threat.iter().map(|(t, _)| *t).collect();
        let timers = &mut m.boss;
        timers.hazard -= dt;
        timers.hit -= dt;
        timers.other -= dt;
        let hazard = timers.hazard <= 0.0;
        if hazard {
            timers.hazard = kit.hazard.interval;
        }
        let other = health < kit.other.2 && timers.other <= 0.0;
        if other {
            timers.other = kit.other.1;
        }
        let hit = !other && timers.hit <= 0.0;
        if hit {
            timers.hit = kit.hit.1;
        }
        // Tried every tick until it works (it can't while on cooldown).
        let enrage = kit
            .enrage
            .filter(|&(below, _)| health < below && !timers.enraged);
        let pack = kit
            .pack
            .filter(|&(below, _, _)| health < below && !timers.called);
        if pack.is_some() {
            timers.called = true;
        }
        if hazard {
            let damage = kit.hazard.damage * level_scale(level);
            for &t in &fighting {
                self.mark(id, t, damage, kit.hazard.times - 1);
            }
        }
        if let Some((_, spell)) = enrage
            && self.try_use(id, spell).is_ok()
        {
            let e = self.entities.get_mut(&id).unwrap();
            mob_of(&mut e.brain).boss.enraged = true;
            let name = e.name.clone();
            self.send(
                Audience::Near(self.pos_of(id)),
                GameEvent::System(format!("{name} flies into a rage!")),
            );
        }
        if let Some((_, kind, count)) = pack {
            self.call_pack(id, camp, kind, level, count, &fighting);
        }
        if other && self.try_use(id, kit.other.0).is_ok() {
            return true;
        }
        hit && self.try_use(id, kit.hit.0).is_ok()
    }

    /// A boss calls for help: `count` `kind`s run into the middle of its
    /// room and join the fight against everyone fighting it.
    fn call_pack(
        &mut self,
        boss: EntityId,
        camp: usize,
        kind: MobKind,
        level: u8,
        count: usize,
        fighting: &[EntityId],
    ) {
        let at = self.pos_of(boss);
        let Some(index) = shared::dungeon::instance_at(at) else {
            return;
        };
        let room = shared::dungeon::of(index).halls.last().unwrap().center;
        let spot = shared::dungeon::to_world(index, room);
        let group = self.camps[camp].group;
        let summoned = self.summon_into_instance(spot, kind, level, count, group);
        for mob in summoned {
            let m = self.entities.get_mut(&mob).unwrap().mob_mut().unwrap();
            m.summoner = Some(boss);
            m.state = MobState::Combat;
            for &t in fighting {
                add_threat(m, t, 1.0);
            }
        }
        let name = self.entities[&boss].name.clone();
        self.send(
            Audience::Near(at),
            GameEvent::System(format!("{name} calls for help!")),
        );
    }

    /// A boss gave up its fight: whatever it called goes away.
    pub(super) fn dismiss_pack(&mut self, boss: EntityId) {
        let gone: Vec<EntityId> = self
            .entities
            .values()
            .filter(|e| e.mob().is_some_and(|m| m.summoner == Some(boss)))
            .map(|e| e.id)
            .collect();
        for id in gone {
            self.entities.remove(&id);
            self.forget(id);
        }
    }

    /// Marks the ground under `target` for its boss's hazard, if it's near
    /// enough to the boss.
    fn mark(&mut self, boss: EntityId, target: EntityId, damage: f32, again: u8) {
        let Some(t) = self.entities.get(&target).filter(|t| !t.dead) else {
            return;
        };
        let Some(kind) = self
            .entities
            .get(&boss)
            .and_then(|b| b.mob())
            .map(|m| m.kind)
        else {
            return;
        };
        let h = &kit(kind).hazard;
        if t.pos.distance(self.pos_of(boss)) < 60.0 {
            self.hazards.push(Hazard {
                pos: t.pos,
                radius: h.radius,
                remaining: h.warning,
                total: h.warning,
                damage,
                source: boss,
                aimed_at: target,
                again,
            });
        }
    }

    /// Counts down marked ground, and hits every player still standing in
    /// it when it lands.
    pub(super) fn tick_hazards(&mut self, dt: f32) {
        let ents = &self.entities;
        self.hazards.retain(|h| {
            ents.get(&h.source)
                .is_some_and(|b| !b.dead && b.mob().is_some_and(|m| m.state == MobState::Combat))
        });
        let mut landed = Vec::new();
        self.hazards.retain_mut(|h| {
            h.remaining -= dt;
            if h.remaining <= 0.0 {
                landed.push((h.pos, h.radius, h.damage, h.source, h.aimed_at, h.again));
                false
            } else {
                true
            }
        });
        for (pos, radius, damage, source, aimed_at, again) in landed {
            let hit: Vec<EntityId> = self
                .entities
                .values()
                .filter(|p| p.player().is_some() && !p.dead)
                .filter(|p| vec2(p.pos.x, p.pos.z).distance(vec2(pos.x, pos.z)) <= radius)
                .map(|p| p.id)
                .collect();
            for p in hit {
                self.apply_hit(source, p, Hit::Damage(damage, false), None);
            }
            // Straight away, the next one comes down where they are now.
            if again > 0 {
                self.mark(source, aimed_at, damage, again - 1);
            }
        }
    }

    /// The marked ground a player can see.
    pub(super) fn hazards_near(&self, at: Vec3) -> Vec<HazardView> {
        self.hazards
            .iter()
            .filter(|h| h.pos.distance(at) <= VIEW_DISTANCE)
            .map(|h| HazardView {
                pos: h.pos,
                radius: h.radius,
                remaining: h.remaining,
                total: h.total,
            })
            .collect()
    }
}
