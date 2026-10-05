//! Travel by waystone, and copies ("instances") of the dungeons: each
//! party, or player on their own, gets a copy of each dungeon of their own,
//! with its mobs fresh. Dead dungeon mobs stay dead until the copy closes,
//! which happens once nobody has been inside for `dungeon::EMPTY_RESET`
//! seconds.

use super::*;
use shared::dungeon::{self, DungeonId};
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
    pub dungeon: DungeonId,
    /// Indices into `World::camps` of this copy's packs.
    camps: Vec<usize>,
    /// Seconds since anyone was inside.
    empty_for: f32,
    /// The boss is dead and the teleporter out is open.
    pub portal: bool,
}

impl World {
    /// Who `id`'s copies of the dungeons belong to: their party, or
    /// themselves.
    fn owner_for(&self, id: EntityId) -> Owner {
        match self.party_of(id) {
            Some(p) => Owner::Party(p),
            None => Owner::Solo(self.entities[&id].name.clone()),
        }
    }

    fn instance_owned_by(&self, owner: &Owner, dungeon: DungeonId) -> Option<u32> {
        self.instances
            .iter()
            .find(|(_, i)| i.owner == *owner && i.dungeon == dungeon)
            .map(|(n, _)| *n)
    }

    /// Adds a camp to the world (reusing a free slot), returning its slot.
    fn add_camp(&mut self, camp: Camp) -> usize {
        match self.free_camps.pop() {
            Some(slot) => {
                self.camps[slot] = camp;
                slot
            }
            None => {
                self.camps.push(camp);
                self.camps.len() - 1
            }
        }
    }

    /// Opens a fresh copy of a dungeon, with every pack in place.
    fn open_instance(&mut self, owner: Owner, which: DungeonId) -> Result<u32, String> {
        let d = which.get();
        let index = (0..dungeon::MAX_INSTANCES)
            .map(|copy| which.instance(copy))
            .find(|n| !self.instances.contains_key(n))
            .ok_or_else(|| format!("{} is crowded. Try again in a few minutes.", d.name))?;
        let mut camps = Vec::new();
        for pack in d.packs {
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
                let slot = self.add_camp(camp);
                // Everyone in a pack fights together.
                let group = *group.get_or_insert(slot);
                self.camps[slot].group = group;
                camps.push(slot);
                for _ in 0..count {
                    self.spawn_mob(slot);
                }
            }
        }
        // A human quest giver waits by the way in.
        let pos = dungeon::to_world(index, dungeon::QUEST_GIVER);
        let entrance = dungeon::to_world(index, dungeon::ENTRANCE);
        self.spawn_npc_at(
            Zone::Amberfall,
            NpcRole::QuestGiver,
            quests::VAULT_GIVER,
            pos,
            yaw_towards(pos, entrance),
        );
        self.instances.insert(
            index,
            Instance {
                owner,
                dungeon: which,
                camps,
                empty_for: 0.0,
                portal: false,
            },
        );
        Ok(index)
    }

    /// Brings `count` more `kind`s into the fight in the dungeon at `at`,
    /// around `at`, helping the mobs of camp `group` (a boss calling for
    /// help). They go when the copy closes, and don't come back when killed.
    pub(super) fn summon_into_instance(
        &mut self,
        at: Vec3,
        kind: MobKind,
        level: u8,
        count: usize,
        group: usize,
    ) -> Vec<EntityId> {
        let Place::Dungeon(index) = Place::at(at) else {
            return Vec::new();
        };
        if !self.instances.contains_key(&index) {
            return Vec::new();
        }
        let slot = self.add_camp(Camp {
            center: vec2(at.x, at.z),
            radius: 4.0,
            kind,
            levels: (level, level),
            count: 0,
            group,
            instance: Some(index),
        });
        self.instances.get_mut(&index).unwrap().camps.push(slot);
        (0..count).map(|_| self.spawn_mob(slot)).collect()
    }

    /// Closes a copy of a dungeon: its mobs, corpses and quest giver are
    /// gone.
    fn close_instance(&mut self, index: u32) {
        let Some(inst) = self.instances.remove(&index) else {
            return;
        };
        let gone: Vec<EntityId> = self
            .entities
            .values()
            .filter(|e| {
                e.mob().is_some_and(|m| inst.camps.contains(&m.camp))
                    || (matches!(e.brain, Brain::Npc(_))
                        && Place::at(e.pos) == Place::Dungeon(index))
            })
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

    /// The waystone (or the vault's teleporter) `id` is standing at, if
    /// any: the place it's in.
    fn waystone_near(&self, id: EntityId) -> Option<Place> {
        let pos = self.entities[&id].pos;
        let place = Place::at(pos);
        let stone = match place {
            Place::Zone(z) => z.ground_local(WAYSTONE_SPOT),
            Place::Dungeon(i) => dungeon::to_world(i, dungeon::EXIT_STONE),
        };
        let near = |at: Vec3| flat_distance(pos, at) <= WAYSTONE_RANGE;
        (near(stone) || self.portal_at(pos).is_some_and(near)).then_some(place)
    }

    /// The open teleporter in the copy of a dungeon at `pos`, if any.
    pub(super) fn portal_at(&self, pos: Vec3) -> Option<Vec3> {
        let Place::Dungeon(i) = Place::at(pos) else {
            return None;
        };
        self.instances
            .get(&i)
            .filter(|inst| inst.portal)
            .map(|_| dungeon::to_world(i, dungeon::of(i).portal))
    }

    /// A boss died: open its dungeon's teleporter.
    pub(super) fn open_portal(&mut self, at: Vec3) {
        let Place::Dungeon(i) = Place::at(at) else {
            return;
        };
        if let Some(inst) = self.instances.get_mut(&i)
            && !inst.portal
        {
            inst.portal = true;
            self.send(
                Audience::Near(at),
                GameEvent::System("A teleporter opens in the boss's chamber.".into()),
            );
        }
    }

    /// Where travelers to a town arrive, and which way they face.
    fn arrival(zone: Zone) -> (Vec3, f32) {
        let pos = zone.ground_local(ARRIVAL_SPOT);
        (pos, yaw_towards(pos, zone.ground_local(Vec2::ZERO)))
    }

    /// Sends a player in a dungeon back to the town they came from.
    fn leave_dungeon(&mut self, id: EntityId) {
        let zone = self.entities[&id].player().unwrap().home_town;
        let (pos, yaw) = Self::arrival(zone);
        self.teleport(id, pos, yaw);
    }

    pub(super) fn travel(&mut self, id: EntityId, to: Destination) -> Result<(), String> {
        let e = &self.entities[&id];
        if e.dead {
            return Err("You can't travel while dead.".into());
        }
        if e.in_combat {
            return Err("You can't travel while in combat.".into());
        }
        let here = self
            .waystone_near(id)
            .ok_or("You need to be at a waystone.")?;
        match to {
            Destination::Town(zone) => {
                if here == Place::Zone(zone) {
                    return Err("You are already here.".into());
                }
                let (pos, yaw) = Self::arrival(zone);
                self.teleport(id, pos, yaw);
                self.send(
                    Audience::Only(id),
                    GameEvent::System(format!("You travel to {}.", zone.town_name())),
                );
            }
            Destination::Dungeon(which) => {
                let d = which.get();
                let Place::Zone(from) = here else {
                    return Err("You are already in a dungeon.".into());
                };
                if e.level < d.min_level {
                    return Err(format!(
                        "You must be level {} to enter {}.",
                        d.min_level, d.name
                    ));
                }
                let owner = self.owner_for(id);
                let index = match self.instance_owned_by(&owner, which) {
                    Some(index) => index,
                    None => self.open_instance(owner, which)?,
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
                    GameEvent::System(format!("You enter {}.", d.name)),
                );
            }
        }
        Ok(())
    }

    /// When a party breaks up, its last member keeps its copies of the
    /// dungeons (unless they already have their own).
    pub(super) fn party_disbanded(&mut self, party: PartyId, last: EntityId) {
        let owner = self.owner_for(last);
        for which in DungeonId::ALL {
            if self.instance_owned_by(&owner, which).is_some() {
                continue;
            }
            if let Some(index) = self.instance_owned_by(&Owner::Party(party), which) {
                self.instances.get_mut(&index).unwrap().owner = owner.clone();
            }
        }
    }

    /// Hands copies of the dungeons to whoever now owns them, takes out anyone
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
            let which = self.instances[&index].dungeon;
            // A player who formed a party while inside brings their party.
            let new_owner = match &owner {
                Owner::Solo(name) => players
                    .iter()
                    .find(|&&p| self.entities[&p].name == *name)
                    .and_then(|&p| self.party_of(p))
                    .map(Owner::Party)
                    .filter(|o| self.instance_owned_by(o, which).is_none()),
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
                            "You're no longer in this dungeon's party, so you've been sent back."
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
