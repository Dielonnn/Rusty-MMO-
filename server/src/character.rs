//! A saved character: everything about a player that outlives a session.

use serde::{Deserialize, Serialize};
use shared::data::*;
use shared::protocol::{CharacterSummary, Stack, Stats};
use shared::talents::{self, Ranks, TALENTS};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Character {
    pub account: String,
    pub name: String,
    pub class: Class,
    pub appearance: Appearance,
    pub level: u8,
    pub xp: u32,
    /// In copper.
    pub money: u32,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub bags: Vec<Option<Stack>>,
    pub gear: [Option<ItemId>; 5],
    #[serde(default)]
    pub talents: Ranks,
}

impl Character {
    /// A new level 1 character, in their race's starting area.
    pub fn new(account: &str, name: &str, class: Class, appearance: Appearance) -> Self {
        let start = appearance.race.zone().graveyard();
        Self {
            account: account.to_string(),
            name: name.to_string(),
            class,
            appearance: appearance.clamped(),
            level: 1,
            xp: 0,
            money: 0,
            pos: start.to_array(),
            yaw: 0.0,
            bags: vec![None; BAG_SLOTS],
            gear: [None; 5],
            talents: [0; TALENTS],
        }
    }

    pub fn summary(&self) -> CharacterSummary {
        CharacterSummary {
            name: self.name.clone(),
            class: self.class,
            level: self.level,
            appearance: self.appearance,
            gear: self.gear,
        }
    }

    /// Repairs anything out of range, e.g. after hand-editing a save file.
    pub fn sanitize(&mut self) {
        self.level = self.level.clamp(1, MAX_LEVEL);
        self.talents = talents::sanitize(&self.talents, self.level);
        self.appearance = self.appearance.clamped();
        self.bags.resize(BAG_SLOTS, None);
        for slot in &mut self.bags {
            if let Some((id, n)) = *slot {
                if id.0 as usize >= ITEMS.len() || n == 0 {
                    *slot = None;
                } else {
                    *slot = Some((id, n.min(item(id).max_stack)));
                }
            }
        }
        for (i, g) in self.gear.iter_mut().enumerate() {
            let fits = g.is_some_and(|id| {
                (id.0 as usize) < ITEMS.len()
                    && matches!(item(id).kind, ItemKind::Armor { slot, .. } if slot.index() == i)
            });
            if !fits {
                *g = None;
            }
        }
        if !self.pos.iter().all(|v| v.is_finite()) {
            self.pos = self.appearance.race.zone().graveyard().to_array();
        }
    }
}

/// Checks a new character name and tidies its capitalization ("bob" -> "Bob").
pub fn validate_name(raw: &str) -> Result<String, &'static str> {
    let raw = raw.trim();
    if !raw.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("Names can only contain letters.");
    }
    if raw.len() < 2 || raw.len() > 12 {
        return Err("Names must be 2 to 12 letters long.");
    }
    let lower = raw.to_ascii_lowercase();
    Ok(lower[..1].to_ascii_uppercase() + &lower[1..])
}

/// Stat totals from worn armor.
pub fn gear_stats(gear: &[Option<ItemId>; 5]) -> Stats {
    let mut stats = Stats::default();
    for id in gear.iter().flatten() {
        if let ItemKind::Armor {
            armor,
            stamina,
            power,
            ..
        } = item(*id).kind
        {
            stats.armor += armor;
            stats.stamina += stamina;
            stats.power += power;
        }
    }
    stats
}

/// Puts items in bags, filling existing stacks first. Returns how many didn't fit.
pub fn add_item(bags: &mut [Option<Stack>], id: ItemId, mut count: u16) -> u16 {
    let max = item(id).max_stack;
    for slot in bags.iter_mut() {
        if let Some((sid, n)) = slot
            && *sid == id
            && *n < max
        {
            let take = count.min(max - *n);
            *n += take;
            count -= take;
        }
        if count == 0 {
            return 0;
        }
    }
    for slot in bags.iter_mut() {
        if slot.is_none() {
            let take = count.min(max);
            *slot = Some((id, take));
            count -= take;
        }
        if count == 0 {
            return 0;
        }
    }
    count
}

pub fn count_item(bags: &[Option<Stack>], id: ItemId) -> u32 {
    bags.iter()
        .flatten()
        .filter(|(sid, _)| *sid == id)
        .map(|(_, n)| *n as u32)
        .sum()
}

/// Takes items out of bags. Assumes there are enough.
pub fn remove_item(bags: &mut [Option<Stack>], id: ItemId, mut count: u16) {
    for slot in bags.iter_mut().rev() {
        if let Some((sid, n)) = slot
            && *sid == id
        {
            let take = count.min(*n);
            *n -= take;
            count -= take;
            if *n == 0 {
                *slot = None;
            }
        }
        if count == 0 {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::data::items::*;

    #[test]
    fn names() {
        assert_eq!(validate_name("bOB"), Ok("Bob".to_string()));
        assert!(validate_name("x").is_err());
        assert!(validate_name("Bob2").is_err());
        assert!(validate_name("Abcdefghijklm").is_err());
    }

    #[test]
    fn bags_stack_and_overflow() {
        let mut bags = vec![None; 2];
        assert_eq!(add_item(&mut bags, LIGHT_LEATHER, 25), 0);
        assert_eq!(bags[0], Some((LIGHT_LEATHER, 20)));
        assert_eq!(bags[1], Some((LIGHT_LEATHER, 5)));
        assert_eq!(add_item(&mut bags, LINEN_CLOTH, 1), 1);
        assert_eq!(add_item(&mut bags, LIGHT_LEATHER, 20), 5);
        assert_eq!(count_item(&bags, LIGHT_LEATHER), 40);
        remove_item(&mut bags, LIGHT_LEATHER, 30);
        assert_eq!(count_item(&bags, LIGHT_LEATHER), 10);
        assert_eq!(bags.iter().flatten().count(), 1);
    }

    #[test]
    fn sanitize_drops_bad_data() {
        let mut c = Character::new("a", "Bob", Class::Rogue, Appearance::default());
        c.level = 99;
        c.bags[0] = Some((ItemId(999), 1));
        c.gear[0] = Some(LEATHER_VEST); // a chest piece in the head slot
        c.gear[1] = Some(LEATHER_VEST);
        c.bags.truncate(3);
        c.sanitize();
        assert_eq!(c.level, MAX_LEVEL);
        assert_eq!(c.bags.len(), BAG_SLOTS);
        assert_eq!(c.bags[0], None);
        assert_eq!(c.gear[0], None);
        assert_eq!(c.gear[1], Some(LEATHER_VEST));
    }
}
