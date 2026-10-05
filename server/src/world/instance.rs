//! Travel by waystone, and copies ("instances") of the Sunken Vault: each
//! party, or player on their own, gets a vault of their own, with its mobs
//! fresh. Dead vault mobs stay dead until the copy closes, which happens
//! once nobody has been inside for `dungeon::EMPTY_RESET` seconds.

use super::*;
use shared::dungeon;
use shared::props::{ARRIVAL_SPOT, WAYSTONE_RANGE, WAYSTONE_SPOT};

/// Who a copy of the vault belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner {
    Party(PartyId),
    /// A player on their own, by character name (so it survives logging
    /// out and back in).
    Solo(String),
}

pub struct Instance {
    pub owner: Owner,
    /// Indices into `World::camps` of this copy's packs.
    camps: Vec<usize>,
    /// Seconds since anyone was inside.
    empty_for: f32,
}

impl World {
    /// Who `id`'s copy of the vault belongs to: their party, or themselves.
    fn owner_for(&self, id: EntityId) -> Owner {
        match self.party_of(id) {
            Some(p) => Owner::Party(p),
            None => Owner::Solo(self.entities[&id].name.clone()),
        }
    }

    fn instance_owned_by(&self, owner: &Owner) -> Option<u32> {
        self.instances
            .iter()
            .find(|(_, i)| i.owner == *owner)
            .map(|(n, _)| *n)
    }

    /// Opens a fresh copy of the vault, with every pack in place.
    fn open_instance(&mut self, owner: Owner) -> Result<u32, &'static str> {
        let index = (0..dungeon::MAX_INSTANCES)
            .find(|n| !self.instances.contains_key(n))
            .ok_or("The Sunken Vault is crowded. Try again in a few minutes.")?;
        let mut camps = Vec::new();
        for pack in &dungeon::PACKS {
            let center = dungeon::to_world(index, pack.center);
            let mut group = None;
            for &(kind, levels, count) in pack.mobs {
                let camp = Camp {
                    center: vec2(center.x, center.z),
                    radius: pack.radius,
                    kind,
                    levels,
                    count,
                    group: 0,
                    instance: Some(index),
                };
                let slot = match self.free_camps.pop() {
                    Some(slot) => {
                        self.camps[slot] = camp;
                        slot
                    }
                    None => {
                        self.camps.push(camp);
                        self.camps.len() - 1
                    }
                };
                // Everyone in a pack fights together.
                let group = *group.get_or_insert(slot);
                self.camps[slot].group = group;
                camps.push(slot);
                for _ in 0..count {
                    self.spawn_mob(slot);
                }
            }
        }
        self.instances.insert(
            index,
            Instance {
                owner,
                camps,
                empty_for: 0.0,
            },
        );
        Ok(index)
    }

    /// Closes a copy of the vault: its mobs and corpses are gone.
    fn close_instance(&mut self, index: u32) {
        let Some(inst) = self.instances.remove(&index) else {
            return;
        };
        let gone: Vec<EntityId> = self
            .entities
            .values()
            .filter(|e| e.mob().is_some_and(|m| inst.camps.contains(&m.camp)))
            .map(|e| e.id)
            .collect();
        for id in gone {
            self.entities.remove(&id);
            self.forget(id);
        }
        self.free_camps.extend(inst.camps);
    }

    /// Moves a player somewhere new (another town, the vault), out of any
    /// fight, and tells their game.
    pub(super) fn teleport(&mut self, id: EntityId, pos: Vec3, yaw: f32) {
        let e = self.entities.get_mut(&id).unwrap();
        e.pos = pos;
        e.yaw = yaw;
        e.cast = None;
        e.target = None;
        e.moving = false;
        if let Some(p) = e.player_mut() {
            p.auto_attack = false;
        }
        self.forget(id);
        self.outbox
            .push((Audience::Only(id), ServerMsg::SetPosition { pos, yaw }));
    }

    /// The waystone `id` is standing at, if any: the place it's in.
    fn waystone_near(&self, id: EntityId) -> Option<Place> {
        let pos = self.entities[&id].pos;
        let place = Place::at(pos);
        let stone = match place {
            Place::Zone(z) => z.ground_local(WAYSTONE_SPOT),
            Place::Dungeon(i) => dungeon::to_world(i, dungeon::EXIT_STONE),
        };
        (flat_distance(pos, stone) <= WAYSTONE_RANGE).then_some(place)
    }

    /// Where travelers to a town arrive, and which way they face.
    fn arrival(zone: Zone) -> (Vec3, f32) {
        let pos = zone.ground_local(ARRIVAL_SPOT);
        (pos, yaw_towards(pos, zone.ground_local(Vec2::ZERO)))
    }

    /// Sends a player in the vault back to the town they came from.
    fn leave_dungeon(&mut self, id: EntityId) {
        let zone = self.entities[&id].player().unwrap().home_town;
        let (pos, yaw) = Self::arrival(zone);
        self.teleport(id, pos, yaw);
    }

    pub(super) fn travel(&mut self, id: EntityId, to: Destination) -> Result<(), &'static str> {
        let e = &self.entities[&id];
        if e.dead {
            return Err("You can't travel while dead.");
        }
        if e.in_combat {
            return Err("You can't travel while in combat.");
        }
        let here = self
            .waystone_near(id)
            .ok_or("You need to be at a waystone.")?;
        match to {
            Destination::Town(zone) => {
                if here == Place::Zone(zone) {
                    return Err("You are already here.");
                }
                let (pos, yaw) = Self::arrival(zone);
                self.teleport(id, pos, yaw);
                self.send(
                    Audience::Only(id),
                    GameEvent::System(format!("You travel to {}.", zone.town_name())),
                );
            }
            Destination::Dungeon => {
                let Place::Zone(from) = here else {
                    return Err("You are already in the vault.");
                };
                if e.level < dungeon::MIN_LEVEL {
                    return Err("You must be level 10 to enter the Sunken Vault.");
                }
                let owner = self.owner_for(id);
                let index = match self.instance_owned_by(&owner) {
                    Some(index) => index,
                    None => self.open_instance(owner)?,
                };
                self.entities
                    .get_mut(&id)
                    .unwrap()
                    .player_mut()
                    .unwrap()
                    .home_town = from;
                self.teleport(id, dungeon::to_world(index, dungeon::ENTRANCE), 0.0);
                self.send(
                    Audience::Only(id),
                    GameEvent::System(format!("You enter {}.", dungeon::NAME)),
                );
            }
        }
        Ok(())
    }

    /// When a party breaks up, its last member keeps its copy of the vault.
    pub(super) fn party_disbanded(&mut self, party: PartyId, last: EntityId) {
        let owner = self.owner_for(last);
        if self.instance_owned_by(&owner).is_some() {
            return;
        }
        if let Some(index) = self.instance_owned_by(&Owner::Party(party)) {
            self.instances.get_mut(&index).unwrap().owner = owner;
        }
    }

    /// Hands copies of the vault to whoever now owns them, takes out anyone
    /// who no longer belongs in theirs, and closes copies left empty.
    pub(super) fn tick_instances(&mut self, dt: f32) {
        let mut inside: BTreeMap<u32, Vec<EntityId>> = BTreeMap::new();
        for e in self.entities.values() {
            if e.player().is_some()
                && let Place::Dungeon(i) = Place::at(e.pos)
            {
                inside.entry(i).or_default().push(e.id);
            }
        }
        let indices: Vec<u32> = self.instances.keys().copied().collect();
        for index in indices {
            let players = inside.remove(&index).unwrap_or_default();
            let owner = self.instances[&index].owner.clone();
            // A player who formed a party while inside brings their party.
            let new_owner = match &owner {
                Owner::Solo(name) => players
                    .iter()
                    .find(|&&p| self.entities[&p].name == *name)
                    .and_then(|&p| self.party_of(p))
                    .map(Owner::Party)
                    .filter(|o| self.instance_owned_by(o).is_none()),
                Owner::Party(_) => None,
            };
            let owner = match new_owner {
                Some(o) => {
                    self.instances.get_mut(&index).unwrap().owner = o.clone();
                    o
                }
                None => owner,
            };
            let mut stayed = 0;
            for p in players {
                if self.owner_for(p) == owner {
                    stayed += 1;
                } else {
                    self.leave_dungeon(p);
                    self.send(
                        Audience::Only(p),
                        GameEvent::System(
                            "You're no longer in this vault's party, so you've been sent back."
                                .into(),
                        ),
                    );
                }
            }
            let inst = self.instances.get_mut(&index).unwrap();
            if stayed > 0 {
                inst.empty_for = 0.0;
            } else {
                inst.empty_for += dt;
                if inst.empty_for >= dungeon::EMPTY_RESET {
                    self.close_instance(index);
                }
            }
        }
        // Anyone in a copy that has closed (or never opened) goes home.
        for p in inside.into_values().flatten() {
            self.leave_dungeon(p);
        }
    }
}
