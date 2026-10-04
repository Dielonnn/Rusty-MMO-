# Changelog

Each release's number is set in the workspace `Cargo.toml` and shown in the
window title, on the login screen and in the server's startup message. The
commit for each release is listed under its heading, and a Windows .exe is
built from that commit.

## v2.0

Commit: the one titled "v2.0: Rogue, unlocks, saves, loot and crafting, autumn dusk".

- New class: **Rogue**, running on energy, with combo points (Sinister Strike,
  Eviscerate, Backstab from behind, Gouge, Evasion, Kick)
- Every class starts with one ability and learns the rest at levels 2, 4, 6, 8
  and 10; locked slots show the level they unlock at
- Character creation: name, class, broad or slender build, skin tone, five hair
  styles and six hair colors, with a live 3D preview
- Character select screen, and logging out goes back to it
- Characters are saved between sessions: level, experience, money, bags, worn
  armor and position (solo saves on your computer, servers save to a JSON file)
- Accounts: log in with an account name; each account has its own characters
- Loot: mobs drop money and items; right-click a sparkling corpse to loot it
  (boars drop Light Leather, bandits Linen Cloth, the golem a Golem Core)
- Backpack (B), character window (C) and crafting (K): craft leather and linen
  armor and the Golemheart Chestguard, and wear it; armor reduces physical
  damage, stamina adds health, power adds damage and healing, and worn armor
  shows on your character
- Rend stacks up to 3 times; Fireball leaves a burn
- Attacking a mob always turns it on you, from any range
- Controls: A and D strafe, your character turns to face the camera as you
  move, holding the right mouse button locks and hides the cursor, and you turn
  to face your target when attacking while standing still
- The zone is now **Amberfall Vale**, the human starting area, at dusk in
  autumn: a setting sun, gradient sky, fog, warm light, red, orange and gold
  trees, fallen leaves, grass, pumpkin and wheat fields with scarecrows,
  fences, and the town of Hearthmere with market stalls, lit windows,
  lanterns and chimney smoke
- Much more detailed characters (faces, hair, bending knees and elbows, capes,
  belts, class gear) and creatures (wolves with ruffs, glowing eyes and bushy
  tails; bristly boars with tusks; hooded bandits; golems with glowing runes)
- Chat lines wrap instead of being cut off
- Protocol version 2: v1 clients can't join v2 servers

## v1.0

Commit: the one titled "v1.0: version labels, changelog, Windows exe build".

The first playable version.

- 3D world: hilly zone with lakes, forests, a safe starter town with a
  graveyard, bandit camps and golem ruins
- Three classes with six abilities each: Warrior (rage), Mage and Cleric (mana)
- Tab-target combat: auto attack, global cooldown, cast times, interrupts,
  range and facing checks, threat, taunts, buffs and debuffs, absorb shields
- Mobs: Gray Wolves, neutral Wild Boars, Bandit Thugs, Bandit Mystics
  (shadow casters) and the elite Ancient Golem; bandits call nearby friends
  for help, and mobs give up and run home if pulled too far
- Levels 1-10 with experience from kills, shared by everyone who fought
- Death and Release Spirit at the graveyard
- HUD: player, target and target-of-target frames, action bar with
  cooldowns, cast bars, nameplates, floating combat text, minimap, chat
- Play Solo (built-in server) or join a dedicated server for multiplayer
