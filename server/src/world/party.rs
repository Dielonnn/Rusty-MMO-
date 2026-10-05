//! Parties: invites, leaving and kicking, party chat, and how a party shares
//! kills (experience, quest credit and turns at the loot).

use super::*;

pub type PartyId = u32;

pub struct Party {
    pub leader: EntityId,
    /// In the order they joined.
    pub members: Vec<EntityId>,
    /// Index into `members` of whoever loots the next corpse first.
    next_loot: usize,
}

impl World {
    pub fn party_of(&self, id: EntityId) -> Option<PartyId> {
        self.entities.get(&id)?.player()?.party
    }

    /// Everyone in `id`'s party, or just `id` when they're on their own.
    pub fn party_members(&self, id: EntityId) -> Vec<EntityId> {
        self.party_of(id)
            .and_then(|p| self.parties.get(&p))
            .map_or_else(|| vec![id], |p| p.members.clone())
    }

    fn player_named(&self, name: &str) -> Option<EntityId> {
        let name = name.trim();
        self.entities
            .values()
            .find(|e| e.player().is_some() && e.name.eq_ignore_ascii_case(name))
            .map(|e| e.id)
    }

    fn tell_party(&mut self, party: PartyId, text: String) {
        for id in self.parties[&party].members.clone() {
            self.send(Audience::Only(id), GameEvent::System(text.clone()));
        }
    }

    fn set_party(&mut self, id: EntityId, party: Option<PartyId>) {
        if let Some(p) = self.entities.get_mut(&id).and_then(|e| e.player_mut()) {
            p.party = party;
        }
    }

    pub(super) fn party_invite(&mut self, id: EntityId, name: &str) -> Result<(), &'static str> {
        let target = self
            .player_named(name)
            .ok_or("There's no player by that name.")?;
        if target == id {
            return Err("You can't invite yourself.");
        }
        if let Some(party) = self.party_of(id) {
            let p = &self.parties[&party];
            if p.leader != id {
                return Err("Only the party leader can invite.");
            }
            if p.members.len() >= MAX_PARTY_SIZE {
                return Err("Your party is full.");
            }
        }
        let t = &self.entities[&target];
        let tp = t.player().unwrap();
        if tp.party.is_some() {
            return Err("They are already in a party.");
        }
        if tp.invite.is_some() {
            return Err("They are already considering an invite.");
        }
        let their_name = t.name.clone();
        let my_name = self.entities[&id].name.clone();
        self.entities
            .get_mut(&target)
            .unwrap()
            .player_mut()
            .unwrap()
            .invite = Some((id, PARTY_INVITE_TIME));
        self.send(
            Audience::Only(target),
            GameEvent::System(format!("{my_name} invites you to a party.")),
        );
        self.send(
            Audience::Only(id),
            GameEvent::System(format!("You invite {their_name} to your party.")),
        );
        Ok(())
    }

    fn take_invite(&mut self, id: EntityId) -> Option<EntityId> {
        let p = self.entities.get_mut(&id)?.player_mut()?;
        p.invite.take().map(|(from, _)| from)
    }

    pub(super) fn party_accept(&mut self, id: EntityId) -> Result<(), &'static str> {
        let inviter = self
            .take_invite(id)
            .ok_or("You haven't been invited to a party.")?;
        if self.party_of(id).is_some() {
            return Err("You are already in a party.");
        }
        if !self.entities.contains_key(&inviter) {
            return Err("They are no longer online.");
        }
        let party = match self.party_of(inviter) {
            Some(party) => {
                let p = &self.parties[&party];
                if p.leader != inviter {
                    return Err("They are no longer the party leader.");
                }
                if p.members.len() >= MAX_PARTY_SIZE {
                    return Err("That party is full.");
                }
                party
            }
            None => {
                let party = self.next_party;
                self.next_party += 1;
                self.parties.insert(
                    party,
                    Party {
                        leader: inviter,
                        members: vec![inviter],
                        next_loot: 0,
                    },
                );
                self.set_party(inviter, Some(party));
                party
            }
        };
        self.parties.get_mut(&party).unwrap().members.push(id);
        self.set_party(id, Some(party));
        let name = self.entities[&id].name.clone();
        self.tell_party(party, format!("{name} joins the party."));
        Ok(())
    }

    pub(super) fn party_decline(&mut self, id: EntityId) {
        let Some(inviter) = self.take_invite(id) else {
            return;
        };
        let name = self.entities[&id].name.clone();
        self.send(
            Audience::Only(inviter),
            GameEvent::System(format!("{name} declines your party invite.")),
        );
    }

    /// Drops the invites a player sent (they're logging out).
    pub(super) fn decline_invites_from(&mut self, id: EntityId) {
        for e in self.entities.values_mut() {
            if let Some(p) = e.player_mut()
                && p.invite.is_some_and(|(from, _)| from == id)
            {
                p.invite = None;
            }
        }
    }

    /// Takes `id` out of their party, handing on the lead and breaking the
    /// party up when only one member would be left.
    pub(super) fn party_leave(&mut self, id: EntityId, verb: &str) {
        let Some(party) = self.party_of(id) else {
            return;
        };
        let name = self.entities[&id].name.clone();
        self.tell_party(party, format!("{name} {verb} the party."));
        self.set_party(id, None);
        let p = self.parties.get_mut(&party).unwrap();
        p.members.retain(|m| *m != id);
        if p.next_loot >= p.members.len() {
            p.next_loot = 0;
        }
        if p.members.len() <= 1 {
            let rest = self.parties.remove(&party).unwrap().members;
            for m in rest {
                self.set_party(m, None);
                self.party_disbanded(party, m);
                self.send(
                    Audience::Only(m),
                    GameEvent::System("Your party has disbanded.".into()),
                );
            }
            return;
        }
        if p.leader == id {
            p.leader = p.members[0];
            let leader = p.leader;
            let name = self.entities[&leader].name.clone();
            self.tell_party(party, format!("{name} is now the party leader."));
        }
    }

    /// The party a leader leads, and the member called `name` in it.
    fn leader_and_member(
        &self,
        id: EntityId,
        name: &str,
    ) -> Result<(PartyId, EntityId), &'static str> {
        let party = self.party_of(id).ok_or("You are not in a party.")?;
        if self.parties[&party].leader != id {
            return Err("Only the party leader can do that.");
        }
        let member = self
            .player_named(name)
            .filter(|m| self.parties[&party].members.contains(m))
            .ok_or("They are not in your party.")?;
        Ok((party, member))
    }

    pub(super) fn party_kick(&mut self, id: EntityId, name: &str) -> Result<(), &'static str> {
        let (_, member) = self.leader_and_member(id, name)?;
        if member == id {
            return Err("Use /leave to leave the party.");
        }
        self.party_leave(member, "is removed from");
        Ok(())
    }

    pub(super) fn party_promote(&mut self, id: EntityId, name: &str) -> Result<(), &'static str> {
        let (party, member) = self.leader_and_member(id, name)?;
        self.parties.get_mut(&party).unwrap().leader = member;
        let name = self.entities[&member].name.clone();
        self.tell_party(party, format!("{name} is now the party leader."));
        Ok(())
    }

    pub(super) fn party_chat(&mut self, id: EntityId, text: &str) -> Result<(), &'static str> {
        let party = self.party_of(id).ok_or("You are not in a party.")?;
        let from = self.entities[&id].name.clone();
        for m in self.parties[&party].members.clone() {
            self.send(
                Audience::Only(m),
                GameEvent::PartyChat {
                    from: from.clone(),
                    text: text.to_string(),
                },
            );
        }
        Ok(())
    }

    /// Runs out unanswered invites and loot turns.
    pub(super) fn tick_parties(&mut self, dt: f32) {
        let mut expired = Vec::new();
        for e in self.entities.values_mut() {
            match &mut e.brain {
                Brain::Player(p) => {
                    if let Some((from, left)) = &mut p.invite {
                        *left -= dt;
                        if *left <= 0.0 {
                            expired.push((e.id, *from));
                            p.invite = None;
                        }
                    }
                }
                Brain::Mob(m) => {
                    if let Some(loot) = &mut m.loot
                        && let Some((_, left)) = &mut loot.turn
                    {
                        *left -= dt;
                        if *left <= 0.0 {
                            loot.turn = None;
                        }
                    }
                }
                Brain::Npc(_) => {}
            }
        }
        for (id, from) in expired {
            self.send(
                Audience::Only(id),
                GameEvent::System("The party invite has run out.".into()),
            );
            let name = self.entities[&id].name.clone();
            self.send(
                Audience::Only(from),
                GameEvent::System(format!("{name} didn't answer your party invite.")),
            );
        }
    }

    /// Who shares a kill: everyone who fought it, and their party members
    /// who are alive and within `PARTY_RANGE` of it.
    pub(super) fn kill_credit(&self, fighters: &[EntityId], at: Vec3) -> Vec<EntityId> {
        let mut credit: Vec<EntityId> = Vec::new();
        for &f in fighters {
            for m in self.party_members(f) {
                let ok = m == f
                    || self
                        .entities
                        .get(&m)
                        .is_some_and(|e| !e.dead && e.pos.distance(at) <= PARTY_RANGE);
                if ok && !credit.contains(&m) {
                    credit.push(m);
                }
            }
        }
        credit
    }

    /// When everyone who may loot a corpse is in one party, whose turn it is
    /// to loot it first. Moves the party's turn along.
    pub(super) fn loot_turn(&mut self, looters: &[EntityId]) -> Option<EntityId> {
        let party = self.party_of(*looters.first()?)?;
        if looters.len() < 2 || looters.iter().any(|l| self.party_of(*l) != Some(party)) {
            return None;
        }
        let p = self.parties.get_mut(&party)?;
        let n = p.members.len();
        let pick = (0..n)
            .map(|i| (p.next_loot + i) % n)
            .find(|&i| looters.contains(&p.members[i]))?;
        p.next_loot = (pick + 1) % n;
        Some(p.members[pick])
    }

    /// Your party as the HUD shows it.
    pub(super) fn party_view(&self, id: EntityId) -> Option<PartyView> {
        let party = &self.parties[&self.party_of(id)?];
        let me = self.entities.get(&id)?.pos;
        let members = party
            .members
            .iter()
            .filter_map(|m| self.entities.get(m))
            .filter_map(|e| Some((e, e.player()?.class)))
            .map(|(e, class)| PartyMember {
                id: e.id,
                name: e.name.clone(),
                class,
                level: e.level,
                hp: e.hp,
                max_hp: e.max_hp,
                power: e.power,
                max_power: e.max_power,
                dead: e.dead,
                near: e.pos.distance(me) <= PARTY_RANGE,
            })
            .collect();
        Some(PartyView {
            leader: party.leader,
            members,
        })
    }
}
