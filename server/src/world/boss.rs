//! Dungeon boss mechanics. Morvane the Sunken King marks the ground under
//! everyone fighting him with Tidal Crash (get out before it lands, then
//! again, since it comes twice in a row), and
//! casts two spells worth interrupting: Drowning Grasp, a heavy hit on his
//! target, and Call of the Deep, which heals him.

use super::*;

/// How much faster Tidal Crash comes than it first did (12 seconds apart
/// with 2.5 seconds' warning).
const CRASH_SPEED: f32 = 1.15;
/// Seconds between Tidal Crashes, how long the warning lasts, how many land
/// in a row, how wide each circle is, and what it hits for (before scaling
/// with the boss's level).
pub const CRASH_INTERVAL: f32 = 12.0 / CRASH_SPEED;
pub const CRASH_WARNING: f32 = 2.5 / CRASH_SPEED;
pub const CRASH_TIMES: u8 = 2;
pub const CRASH_RADIUS: f32 = 4.0;
pub const CRASH_DAMAGE: f32 = 30.0;
/// Seconds between Drowning Grasps, and between Calls of the Deep (which
/// he only casts once hurt).
pub const GRASP_INTERVAL: f32 = 14.0;
pub const DEEP_INTERVAL: f32 = 25.0;
const DEEP_BELOW_HEALTH: f32 = 0.9;

/// A boss's own timers, started when the fight starts.
#[derive(Clone, Copy, Debug)]
pub struct BossTimers {
    crash: f32,
    grasp: f32,
    deep: f32,
}

impl Default for BossTimers {
    fn default() -> Self {
        // The first crash comes quickly, so the fight opens with a dodge.
        BossTimers {
            crash: 5.0,
            grasp: 8.0,
            deep: 15.0,
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
        let hurt = e.hp < e.max_hp * DEEP_BELOW_HEALTH;
        let m = mob_of(&mut e.brain);
        let fighting: Vec<EntityId> = m.threat.iter().map(|(t, _)| *t).collect();
        let timers = &mut m.boss;
        timers.crash -= dt;
        timers.grasp -= dt;
        timers.deep -= dt;
        let crash = timers.crash <= 0.0;
        if crash {
            timers.crash = CRASH_INTERVAL;
        }
        let deep = hurt && timers.deep <= 0.0;
        if deep {
            timers.deep = DEEP_INTERVAL;
        }
        let grasp = !deep && timers.grasp <= 0.0;
        if grasp {
            timers.grasp = GRASP_INTERVAL;
        }
        if crash {
            let damage = CRASH_DAMAGE * level_scale(level);
            for t in fighting {
                self.mark(id, t, damage, CRASH_TIMES - 1);
            }
        }
        if deep && self.try_use(id, ids::CALL_OF_THE_DEEP).is_ok() {
            return true;
        }
        grasp && self.try_use(id, ids::DROWNING_GRASP).is_ok()
    }

    /// Marks the ground under `target` for a Tidal Crash, if it's near
    /// enough to the boss.
    fn mark(&mut self, boss: EntityId, target: EntityId, damage: f32, again: u8) {
        let Some(t) = self.entities.get(&target).filter(|t| !t.dead) else {
            return;
        };
        if t.pos.distance(self.pos_of(boss)) < 60.0 {
            self.hazards.push(Hazard {
                pos: t.pos,
                radius: CRASH_RADIUS,
                remaining: CRASH_WARNING,
                total: CRASH_WARNING,
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
