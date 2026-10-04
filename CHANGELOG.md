# Changelog

Each release's number is set in the workspace `Cargo.toml` and shown in the
window title, on the login screen and in the server's startup message. The
commit for each release is listed under its heading, and a Windows .exe is
built from that commit.

## v6.0

Commit: the one titled "v6.0: GPU lighting, soft shadows and a split renderer".

- **Smooth lighting**: light is worked out per pixel on the graphics card
  instead of once per face. A warm sun, a cool fill from the sky and a dim
  warm bounce from the ground; light wraps softly around shapes; a gentle rim
  light catches edges. Spheres and limbs no longer look faceted, cones and
  cylinders (tree trunks, barrels, wells, posts) shade as round, and boxes
  shade in soft gradients like slightly rounded blocks
- **Soft shadows**: every character and creature, and trees, rocks, shrubs,
  barrels, crates, carts, wells and stalls, cast a soft dark patch on the
  ground, so they sit on it instead of floating. A jumping character's shadow
  stays on the ground and fades as they rise
- **Painted grain**: a faint, brush-like grain over every lit surface up
  close, so large flat colors aren't perfectly flat
- Under the hood, for what's next: the renderer can draw textured surfaces
  and load 3D models (`.glb`), and the client's graphics code is split into
  an engine, the world, the models and the effects so they can be worked on
  separately

## v5.0

Commit: the one titled "v5.0: quests, green drops, spell effects, map and model detail".

- **Quests**: a quest giver in every town with three quests each: a hunt
  (kill eight of the area's hunters), a crafting job (craft a piece of armor
  and hand it in) and an elite kill. They pay money, experience and green
  gear. "!" and "?" markers over the quest giver, on the minimap and on the
  world map; a quest tracker on screen; a quest log (`L`); quests are saved
- **Green gear**: ten new uncommon pieces (a sturdy and an arcane one for
  each slot), better than crafted leather and linen but weaker than the
  blue Heartstone Chestguard. Any mob may drop one; elites usually do
- **Goblin and gnome areas aren't grid-like any more**: their hills had
  ridges laid out in a regular grid, and the ground's colors in every area
  made a checkerboard; both now come from smooth, irregular noise. Their
  towns were rebuilt as an untidy goblin sprawl and huddled gnome hamlets,
  and the trees clump into irregular woods everywhere
- **Spell effects**: missiles shaped by their school, impact bursts, melee
  slashes, shockwaves, pillars of light, healing spirals, drain beams,
  casting circles, and visible auras (shields, stuns, roots, frost, flames,
  poison, bleeding, buffs and curses)
- **More detail, in the classic MMO style**: big pauldrons for most classes
  and many enemies, chunkier hands and boots, armored giants; the world map
  is now inked on parchment with icons for camps, beasts and elites, and the
  minimap shows the land itself in a gold frame
- Protocol version 5: v4 clients and servers can't connect to v5 ones

## v4.0

Commit: the one titled "v4.0: talents, sandbox mode, world map, new layouts and animations".

- **Talent trees** for all 13 classes (`N`): three branches of three talents
  each, a point per level from 2, deeper talents opening as you spend points
  in a branch, and a reset button. Talents are saved with your character
- **Sandbox mode** from the login screen (or `server --sandbox`): a private
  world with a cheat panel (`P`) to set your level, add money, give items,
  teleport between starting areas, summon mobs, turn on god mode and refresh
  cooldowns. Sandbox characters are saved separately
- **World map** (`M`) of your starting area, with its town, camps, levels, the
  elite and everyone nearby
- **Compass** on the minimap: N, E, S and W turn with the camera
- **Every starting area has its own layout**: its own hills and lakes, town
  arrangement (rings of huts, a crescent of towers, a goblin street, a tight
  gnome village, a crooked undead lane), and places for fields, camps and the
  elite's ruins. Amberfall Vale keeps its original map
- **More detail**: wells, carts, signposts, waving banners, flower patches and
  fallen logs; clouds; weather in every area (falling leaves, blowing sand,
  fireflies, glowing spores, snow, ghostly wisps); belts, pouches, bracers,
  boot cuffs, capes with clasps, plate knee and elbow guards and more on
  characters; manes, claws, hooves, jaws and bristles on creatures
- **Animations, unique to each class**: their own idle stance, attack and
  casting pose (two-handed overhead chops, shield-and-sword slashes,
  alternating monk jabs and kicks, twin dagger stabs, drawing a bow, aiming a
  rifle, strumming a lute, raised arms, spread arms, fel claws and more), spell
  light in the hands, flinching when hit, jumping poses and blinking. Creatures
  sniff, root, twitch, bite, rear up and strike
- The goblin merchant Fizzwick is now **Migwick**, and the orc merchant Grukka
  is now **Joe**
- Protocol version 4: v3 clients and servers can't connect to v4 ones

## v3.0

Commit: the one titled "v3.0: races, six starting areas, nine new classes, merchants, collision".

- **Races**: Human, Orc, Elf, Goblin, Gnome and Undead, chosen at character
  creation. Each has its own build and features (orc tusks, elf ears, small
  goblins and gnomes with big heads, undead with glowing eyes) and its own skin
  tones
- **Six starting areas**, one per race, and you start in yours: Amberfall Vale
  (human, autumn dusk), Scorchsand Wastes (orc, desert), Silverbough Glade
  (elf, twilight forest), Grubdeep Caverns (goblin, glowing cave), Frostcog
  Peaks (gnome, snow) and Witherwood (undead, dying forest). Each has its own
  town, buildings, scenery, lighting, five kinds of mobs and an elite
- **Nine new classes**: Ranger, Sorcerer, Paladin, Druid, Artificer, Warlock,
  Fighter, Monk and Bard, each with its own gear on the character model
- The Warrior is now the **Barbarian**, with Charge on `E`; existing Warrior
  characters load as Barbarians
- Every class has a **seventh ability on `E`**, learned at level 3 (Charge,
  Blink, Disengage, Roll, Sprint, Misty Step, Rocket Boots...). `Q` and `E` no
  longer strafe; use `A` and `D`
- **Merchants** in every town square: buy Healing and Mana Potions and crafting
  materials, sell your loot, and drink potions from your bags
- **Collision**: buildings, trees, rocks, fences, tents and other scenery are
  solid
- Fixed the mage hat and the boar's body turning the wrong way when facing
  different directions: rounded parts now turn with the model
- The Golem Core is now the Ancient Core (every elite drops one) and the
  Golemheart Chestguard is the Heartstone Chestguard

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
