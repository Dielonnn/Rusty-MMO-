# Changelog

Each release's number is set in the workspace `Cargo.toml` and shown in the
window title, on the login screen and in the server's startup message. The
commit for each release is listed under its heading, and a Windows .exe is
built from that commit.

## v7.7

Commit: the one titled "v7.7: casters drop potions, elites and the Sunken King drop materials".

- **Casters drop potions**: mystics, shamans, tricksters, necromancers and
  the Drowned Adept drop a Healing Potion 35% of the time
- **Elites drop Iron Scrap**: every zone elite and the Stone Warden always
  drop 5 Iron Scrap, on top of their Ancient Core and Linen Cloth
- **The Sunken King drops materials**: Morvane always drops 3 to 5 Iron
  Scrap and 3 to 5 Light Leather, on top of the rest of his loot

## v7.6

Commit: the one titled "v7.6: Waystones and the Sunken Vault".

- **Waystones**: every town has a rune stone beside its square. Stand next to
  it and press `F` to travel to any other starting area's town, so all six
  areas are now linked. Travel is free, but not while you're in combat
- **The Sunken Vault**, the first dungeon, for level 8 and up: flooded stone
  halls with Vault Hounds, Vault Crawlers, Drowned Enforcers and Drowned
  Adepts (levels 9 and 10), the Stone Warden halfway through, and Morvane the
  Sunken King at the end. Enter from any town's waystone
- **Your own copy**: your party shares one copy of the vault, and everyone else
  (another party, or a player on their own) gets their own. Mobs you kill stay
  dead, and the copy resets once nobody has been inside for 5 minutes.
  Leaving the party sends you back out of its vault; when a party breaks up,
  whoever's left keeps it
- Dying in the vault brings you back at its entrance, whose waystone takes you
  back to the town you came from. Logging out inside puts you by that town's
  waystone next time
- **Crown of the Sunken King**: a new blue helm (14 armor, +6 stamina, +5
  power) that Morvane drops a quarter of the time
- **Loot**: Morvane also always drops a green armor piece and 2 Ancient Cores,
  and half the time a green weapon. The Stone Warden drops like other elites
- The minimap and the world map (`M`) show the vault's halls and room names
- Older games can't join a v7.6 server (protocol 10)

## v7.5

Commit: the one titled "v7.5: Textured spell effects".

- **Glowing spell effects**: spells are now drawn with soft, textured light
  that brightens whatever is behind it, instead of flat colored balls. Each
  school keeps its colors
- **Missiles**: fireballs trail licking flames and smoke, frostbolts carry an
  ice crystal and shed snowflakes, shadow bolts swirl with dark mist, arcane
  missiles spin with stars, holy bolts flare gold, nature bolts trail green
  mist, and arrows and bullets leave faint tracer streaks
- **Impacts**: hits flash with a star-shaped burst and a puff of light, with
  sparks streaking out; area spells send a glowing ring across the ground
- **Heals and beams**: healing pillars are columns of light with motes
  drifting down, healing spirals rise in sparkles, and drain spells pull a
  flickering ribbon of light from the target
- **Casting**: a turning circle of runes glows under anyone casting, with a
  ring closing in as the cast fills
- **Auras**: burning targets have real flames, poison and curses wreathe them
  in mist, stuns spin stars over the head, slows spread frost underfoot,
  shields glint, and empowered characters have red flames at their feet

## v7.3

Commit: the one titled "v7.3: Height and weight sliders".

- **Height and weight sliders** in character creation: drag them to make a
  character shorter or taller, thinner or heavier. The choice is saved with
  the character and sent to everyone who sees them. Characters don't change
  shape yet: the models catch up in a later release
- Characters made before v7.3 start with both sliders in the middle
- Other players and the character list now know which weapon each character
  holds, ready for the models to draw it
- Protocol version 9: clients and servers must both be v7.3

## v7.2

Commit: the one titled "v7.2: Account passwords".

- **Account passwords**: joining a server now asks for a password as well as
  an account name. The first time you log in to an account, the password you
  type becomes its password; after that, only that password gets in. Passwords
  are 6 to 64 characters
- **Existing accounts**: accounts saved before v7.2 have no password yet, and
  take the password used on their next login, keeping all their characters.
  Server owners who open their server to strangers should have their players
  log in once first, so nobody else claims their accounts
- Passwords are never saved: the server keeps only a salted Argon2 hash of
  each one, in the same save file as the characters
- Solo and sandbox play don't need a password
- Older games can't join a v7.2 server (protocol 8)

## v7.1

Commit: the one titled "v7.1: weapons as loot and more crafting recipes".

- **Weapons**: a new weapon slot on the character sheet (`C`). A weapon adds
  damage to every auto attack, and some add stamina or power. Click one in
  your backpack to hold it, and click it on the character sheet to put it
  away. Any class can use any weapon. Weapons aren't drawn in hands yet
- **Weapon drops**: any mob may drop a green weapon (3%; elites 30%): the
  Wolfbite Axe, Ironwood Mace and Shadowfang Dagger (+4 damage, +2 stamina),
  the Ashwood Longbow (+4 damage, +1 stamina, +1 power), and the Emberwand
  and Moonwhisper Staff (+2 damage, +4 power)
- **Iron Scrap**: a new material that fighters (bandits, raiders, satyrs,
  troggs, trolls and skeletons) drop 35% of the time
- **New recipes**: Iron Sword (5 Iron Scrap, 2 Light Leather; +2 damage),
  Hunting Bow (2 Iron Scrap, 4 Light Leather; +2 damage), Apprentice Staff
  (2 Iron Scrap, 4 Linen Cloth; +1 damage, +2 power), the rare Heartstone
  Greatsword (1 Ancient Core, 8 Iron Scrap; +7 damage, +6 stamina, +3 power),
  and Linen Gloves and Linen Sandals to finish the linen set
- The character sheet shows your weapon damage next to stamina and power
- Saved characters load as before, with an empty weapon slot. Old clients
  can't join a v7.1 server (protocol 7)

## v7.0

Commit: the one titled "v7.0: Painted world: ground, foliage, rocks, water and sky".

- **Painted ground**: the ground is painted instead of flat color. Grass has
  soft clumps and brush strokes, with warm highlights and cool shadows;
  roads, fields, shores and town squares show packed dirt and pebbles;
  cliffs and high ground show cracked, layered rock. Each zone has its own
  top layer: grass, wind-rippled sand in Scorchsand, soft snow in
  Frostcog and gritty earth in Grubdeep. The edges between them are ragged,
  like brushwork, instead of smooth fades
- **Leafy trees**: tree canopies and bushes are clumps of painted leaves
  instead of smooth balls, shaded as one round shape. Pines have drooping,
  feathery boughs, snow-dusted in Frostcog
- **Grass and ferns**: thick drifts of grass tufts across the meadows of
  Amberfall, Silverbough and Witherwood, which take their color from the
  ground they grow from; ferns in the undergrowth; flowers in Amberfall;
  dry grass in the desert and frosted grass poking through the snow
- **Rounded rocks**: boulders are lumpy, rounded stones, darker in the
  hollows and underneath, with moss, lichen or snow on top. Pebbles are
  little stones too, mesas are round and have fallen boulders around them,
  and hills shade as smoother slopes
- **Water**: it ripples, is lighter in the shallows and darker in the deep,
  reflects the sky at low angles, catches the sun, and has foam lapping at
  the shore. Frostcog's water is still, glossy ice
- **Skies**: billowing painted clouds, lit on top and shaded underneath; a
  wide, soft glow around the sun; and hazy faraway mountains along the
  horizon, paler the further away they are
- All the new textures are generated by the game itself, so there are no
  outside art files and nothing to license
## v6.5

Commit: the one titled "v6.5: parties".

- **Parties** of up to 8 players. Target a player and click **Invite** (or
  type `/invite NAME`); they get a popup to **Accept** or **Decline**, and the
  invite runs out after 60 seconds. Only the leader invites
- **Party frames** down the left of the screen show each member's name,
  class, level, health and mana/rage/energy, with a gold mark on the leader.
  Members too far away to share kills are dimmed. Click a frame to target
  that member (handy for heals), click **Leave party** to leave
- The leader can remove a member with the **x** on their frame (or
  `/kick NAME`) and hand over the lead with `/promote NAME`. If the leader
  leaves, the next member takes over; a party of one breaks up. Logging out
  leaves your party
- **Shared kills**: when anyone in the party kills a mob, every member who
  is alive and within 60 yards gets the same experience they'd get solo
  (nothing is split) and quest kill credit, and can loot the corpse
- **Loot turns**: party members take turns at the loot. The member whose
  turn it is has the corpse to themselves for 10 seconds, then anyone in the
  party nearby can take what's left
- **Party chat**: `/p MESSAGE` talks only to your party, in blue.
  `/help` lists all the chat commands

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
