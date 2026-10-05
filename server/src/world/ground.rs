//! Items dropped on the ground from a player's bags. For the first
//! `DROP_PROTECTION` seconds only the dropper may pick one up; then anyone
//! may, until it disappears after `DROP_LIFETIME` seconds.

use super::*;

pub struct GroundItem {
    pub id: u32,
    pub stack: Stack,
    pub pos: Vec3,
    /// Who dropped it.
    pub owner: EntityId,
    /// Seconds since it was dropped.
    pub age: f32,
}

impl GroundItem {
    fn may_take(&self, id: EntityId) -> bool {
        self.owner == id || self.age >= DROP_PROTECTION
    }
}

impl World {
    pub(super) fn drop_item(&mut self, id: EntityId, slot: usize) -> Result<(), &'static str> {
        let e = self.entities.get_mut(&id).unwrap();
        if e.dead {
            return Err("You are dead.");
        }
        let forward = Vec3::new(e.yaw.sin(), 0.0, e.yaw.cos());
        let pos = e.pos + forward * 1.2;
        let p = e.player_mut().unwrap();
        let stack = p
            .bags
            .get_mut(slot)
            .and_then(Option::take)
            .ok_or("That slot is empty.")?;
        let item_id = self.next_ground;
        self.next_ground += 1;
        self.ground.push(GroundItem {
            id: item_id,
            stack,
            pos,
            owner: id,
            age: 0.0,
        });
        let (it, n) = stack;
        let what = if n > 1 {
            format!("{} x{n}", item(it).name)
        } else {
            item(it).name.to_string()
        };
        self.send(
            Audience::Only(id),
            GameEvent::System(format!("You drop {what}.")),
        );
        Ok(())
    }

    pub(super) fn pick_up(&mut self, id: EntityId, ground_id: u32) -> Result<(), &'static str> {
        let me = &self.entities[&id];
        if me.dead {
            return Err("You are dead.");
        }
        let i = self
            .ground
            .iter()
            .position(|g| g.id == ground_id)
            .ok_or("It's gone.")?;
        let g = &self.ground[i];
        if g.pos.distance(me.pos) > LOOT_RANGE + 1.0 {
            return Err("You are too far away.");
        }
        if !g.may_take(id) {
            return Err("Someone just dropped that. Wait a moment.");
        }
        let (item_id, n) = g.stack;
        let p = self.entities.get_mut(&id).unwrap().player_mut().unwrap();
        let rest = add_item(&mut p.bags, item_id, n);
        if rest == n {
            return Err("Your bags are full.");
        }
        if rest > 0 {
            self.ground[i].stack.1 = rest;
            self.error(id, "Your bags are full.");
        } else {
            self.ground.remove(i);
        }
        self.send(
            Audience::Only(id),
            GameEvent::Looted {
                money: 0,
                items: vec![(item_id, n - rest)],
            },
        );
        Ok(())
    }

    pub(super) fn tick_ground(&mut self, dt: f32) {
        for g in &mut self.ground {
            g.age += dt;
        }
        self.ground.retain(|g| g.age < DROP_LIFETIME);
    }

    pub(super) fn ground_near(&self, viewer: EntityId, at: Vec3) -> Vec<GroundItemView> {
        self.ground
            .iter()
            .filter(|g| g.pos.distance(at) <= VIEW_DISTANCE)
            .map(|g| GroundItemView {
                id: g.id,
                stack: g.stack,
                pos: g.pos,
                locked: if g.owner == viewer {
                    0.0
                } else {
                    (DROP_PROTECTION - g.age).max(0.0)
                },
            })
            .collect()
    }
}
