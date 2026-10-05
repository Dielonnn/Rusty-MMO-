//! Quests: every starting area's quest giver offers three. Hunt a number of
//! the area's beasts, craft a piece of armor and hand it over, and slay the
//! area's elite. Each also sends their own race's heroes into the Sunken
//! Vault after its king.

use serde::{Deserialize, Serialize};

use crate::data::{ItemId, MobKind, Race, items};
use crate::world::Zone;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuestId(pub u16);

/// What a quest asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Goal {
    /// Kill this many of a kind of mob.
    Kill { kind: MobKind, count: u16 },
    /// Bring the quest giver one of these (which you craft).
    TurnIn { item: ItemId },
}

#[derive(Clone, Copy, Debug)]
pub struct Quest {
    pub id: QuestId,
    pub zone: Zone,
    pub name: &'static str,
    /// What the quest giver says when offering it.
    pub text: &'static str,
    /// What they say when you come back done.
    pub done_text: &'static str,
    pub goal: Goal,
    /// You can take it from this level on.
    pub min_level: u8,
    pub xp: u32,
    /// In copper.
    pub money: u32,
    pub reward: Option<ItemId>,
    /// Only offered to the race whose home town this is.
    pub race_only: bool,
}

/// Quests you can have at once.
pub const MAX_ACTIVE: usize = 6;

/// Your quests: the ones in progress (with progress so far) and the ones
/// turned in.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct QuestLog {
    pub active: Vec<(QuestId, u16)>,
    pub done: Vec<QuestId>,
}

impl QuestLog {
    pub fn progress(&self, id: QuestId) -> Option<u16> {
        self.active.iter().find(|(q, _)| *q == id).map(|(_, p)| *p)
    }

    pub fn is_done(&self, id: QuestId) -> bool {
        self.done.contains(&id)
    }

    /// What a quest giver can offer: not taken, not done, and your level.
    pub fn can_accept(&self, q: &Quest, level: u8) -> Result<(), &'static str> {
        if self.is_done(q.id) {
            Err("You've already done that quest.")
        } else if self.progress(q.id).is_some() {
            Err("You're already on that quest.")
        } else if level < q.min_level {
            Err("You need a higher level for that quest.")
        } else if self.active.len() >= MAX_ACTIVE {
            Err("Your quest log is full.")
        } else {
            Ok(())
        }
    }

    /// Drops unknown quests and duplicates (for old or edited saves).
    pub fn sanitize(&mut self) {
        let mut seen = Vec::new();
        self.active.retain(|(id, _)| {
            let ok = (id.0 as usize) < QUESTS.len() && !seen.contains(id);
            seen.push(*id);
            ok
        });
        self.done.retain(|id| (id.0 as usize) < QUESTS.len());
        self.done.dedup();
        self.active.truncate(MAX_ACTIVE);
    }
}

impl Quest {
    /// How many of something the quest needs.
    pub fn needed(&self) -> u16 {
        match self.goal {
            Goal::Kill { count, .. } => count,
            Goal::TurnIn { .. } => 1,
        }
    }
}

pub fn quest(id: QuestId) -> &'static Quest {
    &QUESTS[id.0 as usize]
}

/// A zone's three quests, in the order they're offered.
pub fn zone_quests(zone: Zone) -> &'static [Quest] {
    let i = zone.index() * 3;
    &QUESTS[i..i + 3]
}

/// The Sunken Vault quest a zone's quest giver offers its own race.
pub fn vault_quest(zone: Zone) -> &'static Quest {
    &QUESTS[18 + zone.index()]
}

/// Every quest a zone's quest giver offers a player of `race`: the zone's
/// three, and the vault quest if it's their home town.
pub fn offered(zone: Zone, race: Race) -> Vec<&'static Quest> {
    let mut all: Vec<&'static Quest> = zone_quests(zone).iter().collect();
    if race.zone() == zone {
        all.push(vault_quest(zone));
    }
    all
}

/// The quest giver in each town.
pub fn quest_giver_name(zone: Zone) -> &'static str {
    match zone {
        Zone::Amberfall => "Marshal Edda Vane",
        Zone::Scorchsand => "Warchief Ruk",
        Zone::Silverbough => "Ranger Lyssara",
        Zone::Grubdeep => "Foreman Skizzle",
        Zone::Frostcog => "Tinker Pip Gearwhistle",
        Zone::Witherwood => "Deathguard Morrow",
    }
}

#[allow(clippy::too_many_arguments)]
const fn hunt(
    id: u16,
    zone: Zone,
    name: &'static str,
    kind: MobKind,
    count: u16,
    text: &'static str,
    done_text: &'static str,
    reward: ItemId,
) -> Quest {
    Quest {
        id: QuestId(id),
        zone,
        name,
        text,
        done_text,
        goal: Goal::Kill { kind, count },
        min_level: 1,
        xp: 250,
        money: 150,
        reward: Some(reward),
        race_only: false,
    }
}

const fn craft(
    id: u16,
    zone: Zone,
    name: &'static str,
    item: ItemId,
    text: &'static str,
    done_text: &'static str,
    reward: ItemId,
) -> Quest {
    Quest {
        id: QuestId(id),
        zone,
        name,
        text,
        done_text,
        goal: Goal::TurnIn { item },
        min_level: 2,
        xp: 350,
        money: 300,
        reward: Some(reward),
        race_only: false,
    }
}

const fn elite(
    id: u16,
    zone: Zone,
    name: &'static str,
    kind: MobKind,
    text: &'static str,
    done_text: &'static str,
    reward: ItemId,
) -> Quest {
    Quest {
        id: QuestId(id),
        zone,
        name,
        text,
        done_text,
        goal: Goal::Kill { kind, count: 1 },
        min_level: 8,
        xp: 600,
        money: 2000,
        reward: Some(reward),
        race_only: false,
    }
}

const fn vault(id: u16, zone: Zone, text: &'static str, done_text: &'static str) -> Quest {
    Quest {
        id: QuestId(id),
        zone,
        name: "The Sunken King",
        text,
        done_text,
        goal: Goal::Kill {
            kind: MobKind::SunkenKing,
            count: 1,
        },
        min_level: 8,
        xp: 1200,
        money: 5000,
        reward: Some(TIDEBREAKER_TRIDENT),
        race_only: true,
    }
}

use items::*;

pub static QUESTS: [Quest; 24] = [
    // Amberfall Vale
    hunt(
        0,
        Zone::Amberfall,
        "Wolves at the Door",
        MobKind::Wolf,
        8,
        "The gray wolves grow bolder every night. They've taken two of our sheep and nearly a child. Thin their numbers: eight should teach them to keep to the hills.",
        "The night watch says the howling's quieter already. Hearthmere thanks you.",
        TRAILBLAZER_BOOTS,
    ),
    craft(
        1,
        Zone::Amberfall,
        "A Vest for the Watch",
        LEATHER_VEST,
        "Our watchmen patrol in their shirtsleeves. Take hides from the wild boars, work them into a Leather Vest (press K to craft) and bring it to me.",
        "Fine stitching! The watch will be glad of it. Take this for your trouble.",
        IRONBARK_JERKIN,
    ),
    elite(
        2,
        Zone::Amberfall,
        "The Ancient Golem",
        MobKind::Golem,
        "Far to the south-west, in the old ruins, an Ancient Golem has woken. It is no ordinary foe; gather your strength, and friends if you have them, and put it back to sleep.",
        "The golem has fallen? Then the vale is safe once more. You're a hero of Hearthmere.",
        MOONTHREAD_VESTMENT,
    ),
    // Scorchsand Wastes
    hunt(
        3,
        Zone::Scorchsand,
        "Stingers in the Sand",
        MobKind::Scorpion,
        8,
        "Dune scorpions have stung three of my warriors this week. Crush eight of them, and bring honor to Kragmaw Hold.",
        "Strong! The sands are safer for your blade.",
        TRAILBLAZER_BOOTS,
    ),
    craft(
        4,
        Zone::Scorchsand,
        "Boots for the Long March",
        LEATHER_BOOTS,
        "Our scouts' feet burn on the hot sand. Skin the dust hyenas, make a pair of Leather Boots (K to craft), and bring them here.",
        "These will carry a scout across the whole waste. Lok'tar!",
        RIDGERUNNER_LEGGINGS,
    ),
    elite(
        5,
        Zone::Scorchsand,
        "The Sandstone Colossus",
        MobKind::SandstoneColossus,
        "A living mountain of sandstone walks the ruins at the edge of the waste. Topple the Sandstone Colossus and every orc will know your name.",
        "The colossus is rubble? Ha! Your name will be sung at every fire.",
        IRONBARK_JERKIN,
    ),
    // Silverbough Glade
    hunt(
        6,
        Zone::Silverbough,
        "Shadows in the Glade",
        MobKind::ShadowfangWolf,
        8,
        "Shadowfang wolves stalk beneath the silver boughs, corrupted by something old. Lay eight of them to rest.",
        "The glade breathes easier. Elune's light upon you.",
        WHISPERSTEP_SLIPPERS,
    ),
    craft(
        7,
        Zone::Silverbough,
        "Robes for the Moonwell",
        LINEN_ROBE,
        "The moonwell's keepers need new robes. The satyrs carry fine cloth; take it from them, weave a Linen Robe (K to craft), and bring it to me.",
        "Beautifully woven. The keepers will wear it with pride.",
        STARWEAVE_TROUSERS,
    ),
    elite(
        8,
        Zone::Silverbough,
        "The Ancient Treant",
        MobKind::Treant,
        "An ancient treant, maddened by the corruption, tears at the old ruins. It was a guardian once. End its suffering.",
        "Its spirit is at peace. Thank you, friend of the forest.",
        MOONTHREAD_VESTMENT,
    ),
    // Grubdeep Caverns
    hunt(
        9,
        Zone::Grubdeep,
        "Spider Problem",
        MobKind::CaveSpider,
        8,
        "Spiders! In the mine! Webbing up my carts! Squash eight of 'em and I'll make it worth your while. Probably.",
        "Ha! The carts are rolling again. Here's your cut, as promised. Mostly.",
        BRAWLERS_GRIPS,
    ),
    craft(
        10,
        Zone::Grubdeep,
        "Safety Gloves",
        LEATHER_GLOVES,
        "Workplace safety is very important to me, for insurance reasons. Get hides off the stonehide boars, make some Leather Gloves (K to craft), and hand 'em over.",
        "Perfect fit! Now nobody can sue me. Pleasure doing business.",
        SPELLWEAVER_GLOVES,
    ),
    elite(
        11,
        Zone::Grubdeep,
        "The Crystal Golem",
        MobKind::CrystalGolem,
        "There's a golem made of solid crystal down in the deep ruins. Do you know what crystal is worth? Smash it, and the shiny bits are ours. I mean, yours. Some of them.",
        "You smashed it! I'm rich! I mean, we're rich! Here, take this, quick, before I change my mind.",
        IRONBARK_JERKIN,
    ),
    // Frostcog Peaks
    hunt(
        12,
        Zone::Frostcog,
        "Wolves on the Ridge",
        MobKind::SnowWolf,
        8,
        "The snow wolves keep chewing on my gear-sleds! Statistically, eight fewer wolves means roughly eight fewer chewed sleds.",
        "My calculations were correct! Splendid. Here's a little something.",
        WOLFHIDE_HELM,
    ),
    craft(
        13,
        Zone::Frostcog,
        "A Cap for the Cold",
        LEATHER_CAP,
        "Brr! My ears are positively frozen. Could you skin a frost boar, stitch a Leather Cap (K to craft), and bring it to me?",
        "Toasty! Ingenious! Take this, with my gratitude.",
        SILKWEAVE_CIRCLET,
    ),
    elite(
        14,
        Zone::Frostcog,
        "The Yeti",
        MobKind::Yeti,
        "A great yeti has made a lair in the ruins on the high peak, and it's been eating my test subjects. Er, my mountain goats. Please deal with it.",
        "The yeti is no more! Science, and the goats, thank you.",
        MOONTHREAD_VESTMENT,
    ),
    // Witherwood
    hunt(
        15,
        Zone::Witherwood,
        "Hounds of the Grave",
        MobKind::GhoulHound,
        8,
        "Ghoul hounds dig at our crypts and drag off the bones of our kin. Put down eight of the beasts.",
        "The crypts are quiet. The Forsaken remember those who help them.",
        WHISPERSTEP_SLIPPERS,
    ),
    craft(
        16,
        Zone::Witherwood,
        "A Hood Against the Light",
        LINEN_HOOD,
        "The sun stings our dead eyes. The necromancers carry cloth; take it from them, sew a Linen Hood (K to craft) and bring it to me.",
        "Ah, shade. Thank you. Take this; I have no further use for it.",
        SILKWEAVE_CIRCLET,
    ),
    elite(
        17,
        Zone::Witherwood,
        "The Bone Colossus",
        MobKind::BoneColossus,
        "In the ruins at the forest's edge, the necromancers have raised a colossus of bones. Break it apart before it marches on Gravenhold.",
        "The colossus is dust. Gravenhold stands because of you.",
        IRONBARK_JERKIN,
    ),
    // The Sunken Vault, one for each race, in zone order.
    vault(
        18,
        Zone::Amberfall,
        "Beyond the waystone lies the Sunken Vault, where Morvane the Sunken King gathers drowned men to his throne. Take the waystone, gather friends, and end his reign before the tide reaches Hearthmere.",
        "Morvane is dead? Then the tide turns at last. Hearthmere's smiths pulled this trident from the vault's spoils; it's yours.",
    ),
    vault(
        19,
        Zone::Scorchsand,
        "There's a drowned king beneath the world who thinks he can take orc lands for his sea. Go through the waystone into his Sunken Vault and bring the Horde his crown's last breath.",
        "The Sunken King is fish food! Take his trident, champion. Every warrior in Kragmaw will want to hear how you won it.",
    ),
    vault(
        20,
        Zone::Silverbough,
        "The moonwell's waters grow brackish. The cause lies deep in the Sunken Vault: Morvane, a king who would drown the whole world. The waystone will take you there. Elune guide your blade.",
        "The well runs clear again. This trident was his; may it serve the glade better than it served him.",
    ),
    vault(
        21,
        Zone::Grubdeep,
        "Somebody's flooding my lower tunnels, and the somebody is a dead king in a vault full of treasure. Take the waystone, sink the Sunken King, and we split the profits. Mostly in your favor! Probably.",
        "You did it! Here, the trident's yours, fair and square. I only took the gems off it. Some of the gems.",
    ),
    vault(
        22,
        Zone::Frostcog,
        "My tide gauges show something enormous rising in the Sunken Vault. Hypothesis: a very angry dead king. Please use the waystone and test that hypothesis by defeating him.",
        "Hypothesis confirmed, and also defeated! I've recalibrated his trident for you. It's ever so slightly more pointy now.",
    ),
    vault(
        23,
        Zone::Witherwood,
        "Morvane the Sunken King calls the drowned to serve him, and some of them were ours. Go through the waystone into his vault and silence him. No one commands the Forsaken but the Forsaken.",
        "His call is silent. Our drowned kin rest. Take his trident; Gravenhold has no use for a dead king's toys.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{ItemKind, MobKind, RECIPES, item};

    #[test]
    fn every_zone_has_a_hunt_a_craft_and_an_elite() {
        for (i, q) in QUESTS.iter().enumerate() {
            assert_eq!(q.id.0 as usize, i);
        }
        for zone in Zone::ALL {
            let qs = zone_quests(zone);
            let mobs = MobKind::for_zone(zone);
            assert!(qs.iter().all(|q| q.zone == zone));
            assert!(
                matches!(qs[0].goal, Goal::Kill { kind, count } if kind == mobs[0] && count > 1)
            );
            match qs[1].goal {
                Goal::TurnIn { item: it } => {
                    assert!(matches!(item(it).kind, ItemKind::Armor { .. }));
                    assert!(
                        RECIPES.iter().any(|r| r.result == it),
                        "{zone:?} turn-in isn't craftable"
                    );
                }
                _ => panic!("{zone:?} has no crafting quest"),
            }
            assert!(matches!(qs[2].goal, Goal::Kill { kind, count: 1 } if kind == mobs[4]));
            assert!(mobs[4].template().elite);
        }
    }

    #[test]
    fn every_race_gets_the_vault_quest_at_home_only() {
        for race in Race::ALL {
            for zone in Zone::ALL {
                let offered = offered(zone, race);
                let has = offered.iter().any(|q| q.race_only);
                assert_eq!(has, race.zone() == zone, "{race:?} in {zone:?}");
                assert_eq!(offered.len(), if has { 4 } else { 3 });
            }
            let q = vault_quest(race.zone());
            assert_eq!(q.zone, race.zone());
            assert!(matches!(
                q.goal,
                Goal::Kill {
                    kind: MobKind::SunkenKing,
                    count: 1
                }
            ));
            let reward = item(q.reward.unwrap());
            assert!(matches!(reward.kind, ItemKind::Weapon { .. }));
            assert_eq!(reward.quality, crate::data::Quality::Rare);
        }
    }

    #[test]
    fn quest_logs_check_what_can_be_taken() {
        let q = &QUESTS[0];
        let mut log = QuestLog::default();
        assert!(log.can_accept(q, 1).is_ok());
        assert!(
            log.can_accept(&QUESTS[2], 1).is_err(),
            "elite quest needs a level"
        );
        log.active.push((q.id, 3));
        assert!(log.can_accept(q, 1).is_err());
        assert_eq!(log.progress(q.id), Some(3));
        log.active.push((QuestId(999), 0));
        log.active.push((q.id, 1));
        log.sanitize();
        assert_eq!(log.active, vec![(q.id, 3)]);
    }
}
