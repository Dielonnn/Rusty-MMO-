//! Duels: one player challenges another, and once they accept and a short
//! countdown runs out the two can fight each other. Nobody dies or loses
//! anything: the duel ends when one of them is down to their last point of
//! health, stays outside the duel area too long, dies to something else or
//! logs out.

use super::*;

/// A player's side of a duel. Both duelists hold one, pointing at each other.
#[derive(Clone, Copy, Debug)]
pub struct Duel {
    pub opponent: EntityId,
    /// Seconds until the fighting starts; zero or below once it has.
    pub countdown: f32,
    /// Where the duel started.
    pub flag: Vec3,
    /// Seconds spent outside the duel area.
    pub away: f32,
}

impl Duel {
    pub fn view(&self) -> DuelView {
        DuelView {
            opponent: self.opponent,
            countdown: self.countdown,
            flag: self.flag,
            away: self.away,
        }
    }
}

/// How a duel ended, for the message everyone nearby sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DuelEnd {
    Beaten,
    Fled,
    Died,
    LoggedOut,
}

impl World {
    fn duel_of(&self, id: EntityId) -> Option<Duel> {
        self.entities.get(&id)?.player()?.duel
    }

    /// `a` and `b` are fighting a duel with each other (past the countdown).
    pub fn dueling(&self, a: EntityId, b: EntityId) -> bool {
        let live = |x: EntityId, y: EntityId| {
            self.duel_of(x)
                .is_some_and(|d| d.opponent == y && d.countdown <= 0.0)
        };
        live(a, b) && live(b, a)
    }

    /// Whether `a` may attack `b`: players and mobs always, and duelists each other.
    pub fn hostile(&self, a: EntityId, b: EntityId) -> bool {
        let (Some(x), Some(y)) = (self.entities.get(&a), self.entities.get(&b)) else {
            return false;
        };
        x.kind().hostile_to(y.kind()) || self.dueling(a, b)
    }

    pub(super) fn duel_request(&mut self, id: EntityId, name: &str) -> Result<(), &'static str> {
        let target = self
            .player_named(name)
            .ok_or("There's no player by that name.")?;
        if target == id {
            return Err("You can't duel yourself.");
        }
        let (me, them) = (&self.entities[&id], &self.entities[&target]);
        if me.dead || them.dead {
            return Err("The dead can't duel.");
        }
        if self.duel_of(id).is_some() {
            return Err("You are already in a duel.");
        }
        if self.duel_of(target).is_some() {
            return Err("They are already in a duel.");
        }
        if matches!(Place::at(me.pos), Place::Dungeon(_)) {
            return Err("You can't duel in a dungeon.");
        }
        if Place::at(me.pos) != Place::at(them.pos)
            || me.pos.distance(them.pos) > DUEL_REQUEST_RANGE
        {
            return Err("They are too far away to duel.");
        }
        let tp = them.player().unwrap();
        if tp.duel_invite.is_some() {
            return Err("They are already considering a duel.");
        }
        let their_name = them.name.clone();
        let my_name = me.name.clone();
        self.entities
            .get_mut(&target)
            .unwrap()
            .player_mut()
            .unwrap()
            .duel_invite = Some((id, DUEL_REQUEST_TIME));
        self.send(
            Audience::Only(target),
            GameEvent::System(format!("{my_name} challenges you to a duel.")),
        );
        self.send(
            Audience::Only(id),
            GameEvent::System(format!("You challenge {their_name} to a duel.")),
        );
        Ok(())
    }

    fn take_duel_invite(&mut self, id: EntityId) -> Option<EntityId> {
        let p = self.entities.get_mut(&id)?.player_mut()?;
        p.duel_invite.take().map(|(from, _)| from)
    }

    pub(super) fn duel_accept(&mut self, id: EntityId) -> Result<(), &'static str> {
        let from = self
            .take_duel_invite(id)
            .ok_or("Nobody has challenged you to a duel.")?;
        let them = self
            .entities
            .get(&from)
            .ok_or("They are no longer online.")?;
        let me = &self.entities[&id];
        if me.dead || them.dead {
            return Err("The dead can't duel.");
        }
        if self.duel_of(id).is_some() || self.duel_of(from).is_some() {
            return Err("One of you is already in a duel.");
        }
        if matches!(Place::at(me.pos), Place::Dungeon(_)) {
            return Err("You can't duel in a dungeon.");
        }
        if Place::at(me.pos) != Place::at(them.pos)
            || me.pos.distance(them.pos) > DUEL_REQUEST_RANGE
        {
            return Err("They are too far away to duel.");
        }
        let flag = (me.pos + them.pos) / 2.0;
        let (my_name, their_name) = (me.name.clone(), them.name.clone());
        for (a, b) in [(id, from), (from, id)] {
            self.entities
                .get_mut(&a)
                .unwrap()
                .player_mut()
                .unwrap()
                .duel = Some(Duel {
                opponent: b,
                countdown: DUEL_COUNTDOWN,
                flag,
                away: 0.0,
            });
        }
        self.send(
            Audience::Near(flag),
            GameEvent::System(format!("{my_name} accepts {their_name}'s duel challenge.")),
        );
        Ok(())
    }

    pub(super) fn duel_decline(&mut self, id: EntityId) {
        let Some(from) = self.take_duel_invite(id) else {
            return;
        };
        let name = self.entities[&id].name.clone();
        self.send(
            Audience::Only(from),
            GameEvent::System(format!("{name} declines your duel.")),
        );
    }

    pub(super) fn duel_forfeit(&mut self, id: EntityId) -> Result<(), &'static str> {
        if self.duel_of(id).is_none() {
            return Err("You are not in a duel.");
        }
        self.end_duel(id, DuelEnd::Fled);
        Ok(())
    }

    /// Ends `loser`'s duel. The two stop fighting each other and lose what
    /// they cast on each other.
    pub(super) fn end_duel(&mut self, loser: EntityId, how: DuelEnd) {
        let Some(duel) = self.duel_of(loser) else {
            return;
        };
        let winner = duel.opponent;
        for (a, b) in [(loser, winner), (winner, loser)] {
            let Some(e) = self.entities.get_mut(&a) else {
                continue;
            };
            e.auras.retain(|aura| aura.source != b);
            if e.cast.as_ref().is_some_and(|c| c.target == Some(b)) {
                e.cast = None;
            }
            let aimed = e.target == Some(b);
            if let Some(p) = e.player_mut() {
                p.duel = None;
                if aimed {
                    p.auto_attack = false;
                }
            }
        }
        self.missiles.retain(|m| {
            !(m.caster == loser && m.target == winner || m.caster == winner && m.target == loser)
        });
        let name = |id: EntityId| {
            self.entities
                .get(&id)
                .map_or_else(String::new, |e| e.name.clone())
        };
        let (w, l) = (name(winner), name(loser));
        let text = match how {
            DuelEnd::Beaten => format!("{w} has defeated {l} in a duel."),
            DuelEnd::Fled => format!("{l} has fled from {w} in a duel."),
            DuelEnd::Died => format!("{l} has died, and {w} wins the duel."),
            DuelEnd::LoggedOut => format!("{l} has left the world, and {w} wins the duel."),
        };
        self.send(Audience::Near(duel.flag), GameEvent::System(text));
    }

    /// Drops the duel challenges a player sent (they're logging out).
    pub(super) fn withdraw_duels_from(&mut self, id: EntityId) {
        for e in self.entities.values_mut() {
            if let Some(p) = e.player_mut()
                && p.duel_invite.is_some_and(|(from, _)| from == id)
            {
                p.duel_invite = None;
            }
        }
    }

    /// Runs out challenges, counts duels down, and ends the duels of
    /// players who stay out of the area.
    pub(super) fn tick_duels(&mut self, dt: f32) {
        let mut expired = Vec::new();
        let mut started = Vec::new();
        let mut fled = Vec::new();
        for e in self.entities.values_mut() {
            let (id, pos) = (e.id, e.pos);
            let Some(p) = e.player_mut() else { continue };
            if let Some((from, left)) = &mut p.duel_invite {
                *left -= dt;
                if *left <= 0.0 {
                    expired.push((id, *from));
                    p.duel_invite = None;
                }
            }
            if let Some(d) = &mut p.duel {
                let before = d.countdown;
                // Keep counting a little past zero so clients can show "Fight!".
                d.countdown = (d.countdown - dt).max(-1.0);
                if before > 0.0 && d.countdown <= 0.0 {
                    started.push(id);
                }
                if Place::at(pos) != Place::at(d.flag) {
                    fled.push(id);
                } else if pos.distance(d.flag) > DUEL_AREA {
                    d.away += dt;
                    if d.away >= DUEL_LEAVE_TIME {
                        fled.push(id);
                    }
                } else {
                    d.away = 0.0;
                }
            }
        }
        for (id, from) in expired {
            self.send(
                Audience::Only(id),
                GameEvent::System("The duel challenge has run out.".into()),
            );
            let name = self.entities[&id].name.clone();
            self.send(
                Audience::Only(from),
                GameEvent::System(format!("{name} didn't answer your duel challenge.")),
            );
        }
        for id in started {
            self.send(Audience::Only(id), GameEvent::System("Fight!".into()));
        }
        for id in fled {
            self.end_duel(id, DuelEnd::Fled);
        }
    }
}
