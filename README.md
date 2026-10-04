# Rusty MMO

A small 3D tab-targeting MMO written in Rust, in the spirit of World of Warcraft
and Final Fantasy XIV. Pick one of six races and thirteen classes, and start in
your race's own starting area: autumn hills, a desert, an elven forest, a
goblin cave, snowy peaks or a dying forest, each with its own town, merchant,
creatures and elite. Loot leather, cloth and iron, craft armor and weapons, buy potions, and level
up to unlock your abilities. Play solo, or run a server and adventure together.
Characters are saved between sessions.

Built with [macroquad](https://macroquad.rs) for the client and plain `std`
networking for the server.

## Running

```sh
cargo run --release
```

This opens the client. On the login screen, enter an account name, then:

- **Play Solo** starts a private server inside the game. Solo characters are
  saved on your computer (`%APPDATA%\RustyMMO` on Windows,
  `~/.local/share/rusty-mmo` on Linux, `~/Library/Application Support/RustyMMO`
  on macOS).
- **Join Server** connects to the address in the box (`host` or `host:port`).
  Your characters are saved on that server, under your account name.
- **Sandbox** starts a private world with cheats, for trying things out. Press
  `P` in game for the sandbox panel: set your level, add money, give yourself
  any item, teleport to any starting area, summon any of the area's mobs, turn
  on god mode (no damage, free abilities), or refresh your cooldowns, health
  and power. Sandbox characters are kept apart from your solo ones.

Then pick a character or create one: choose a name, race, class, build, skin,
hair style and hair color, and press **Enter World**. You start in your race's
starting area.

To host a game for friends, run the dedicated server:

```sh
cargo run --release -p server                       # listens on 0.0.0.0:7878
cargo run --release -p server -- --port 9000 --save my_world.json
cargo run --release -p server -- --sandbox          # everyone may use the cheats
```

Characters are saved to `characters.json` (or the `--save` file) every few
seconds and whenever someone logs out. Accounts have no passwords yet, so only
share a server with people you trust.

On Linux you may need the system libraries macroquad links against:

```sh
sudo apt install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev
```

Run the tests with `cargo test --workspace`.

### Building a Windows .exe

On Windows, `cargo build --release` produces `target/release/rusty_mmo.exe`
(the game) and `target/release/server.exe` (the dedicated server).
To cross-compile from Linux:

```sh
rustup target add x86_64-pc-windows-gnu
sudo apt install gcc-mingw-w64-x86-64
cargo build --release --workspace --target x86_64-pc-windows-gnu
```

The exes end up in `target/x86_64-pc-windows-gnu/release/` and run on their
own, with no installer or extra files needed.

## Versions

See [CHANGELOG.md](CHANGELOG.md). The current version is shown in the window
title and on the login screen.

## How to play

| Input | Action |
| --- | --- |
| `W` `S` / up and down arrows | Run forward and back |
| `A` `D` | Strafe left and right |
| Left and right arrows | Turn |
| `Space` | Jump |
| Left mouse drag | Look around |
| Hold right mouse | Steer with the mouse; the cursor locks and hides while held |
| Both mouse buttons | Run forward |
| Mouse wheel | Zoom |
| `Tab` | Target the next enemy in view, nearest first |
| Left click | Target |
| Right click | Attack an enemy, loot a sparkling corpse, trade with a merchant, or talk to a quest giver |
| `F1` | Target yourself |
| `1`-`6`, `E`, or click the action bar | Use an ability |
| `T` | Start / stop auto attack |
| `B` / `C` / `K` | Backpack / character / crafting |
| `N` | Talents |
| `L` | Quest log |
| `M` | World map |
| `P` | Sandbox panel (sandbox mode only) |
| `Esc` | Close windows, clear target, or open the game menu |
| `Enter` | Chat (`/help` lists the chat commands, `/who` lists who's online) |
| `H` | Show / hide the controls |

Your character turns to face where the camera looks whenever you move, and
turns to face your target when you attack while standing still.

### Parties

Up to 8 players can group up. Target a player and click **Invite** (or type
`/invite NAME`); they accept or decline from a popup. Party frames down the
left show everyone's health and power; click one to target that member. When
anyone in the party kills a mob, every member alive and within 60 yards gets
full experience for their level and quest credit, and members take turns
looting (10 seconds each, then it's open to the party). `/p MESSAGE` is party
chat; `/leave`, `/kick NAME` and `/promote NAME` do what they say.

### Combat

Combat works like a classic tab-target MMO:

- **Auto attack.** Your weapon (or a caster's wand) swings on a timer at your
  target while you're attacking.
- **Global cooldown.** Every ability triggers a 1.5 second GCD; some also have
  their own cooldown.
- **Cast times.** Spells like Fireball take time to cast, and moving or getting
  stunned interrupts them. Mobs cast too, and Skull Bash, Shield Bash, Gouge
  and Kick interrupt them.
- **Range and facing.** You have to be in range and facing your target. The
  action bar turns an ability's name red when you're out of range, and shades
  it blue when you can't use it yet.
- **Threat.** Attacking a mob turns it on you. Mobs attack whoever has made
  them angriest. Some attacks make extra threat, Taunt pulls
  a mob straight to you, and healing someone draws the attention of whatever is
  fighting them.
- **Buffs and debuffs.** Damage and heals over time (Rend stacks up to 3
  times), slows, roots, stuns, absorb shields and damage reduction, shown as
  icons with timers under the unit frames.
- **Leashing.** Chase a mob too far from home and it gives up, runs back and
  heals to full.

### Races

Each race starts in its own starting area. The areas share a basic shape (a
town in the middle, farms, camps and an elite's ruins further out) but each
has its own look, buildings, creatures and lighting.

| Race | Starting area | Town | Creatures |
| --- | --- | --- | --- |
| Human | Amberfall Vale: autumn hills at dusk | Hearthmere | Gray Wolves, Wild Boars, Bandits, the Ancient Golem |
| Orc | Scorchsand Wastes: desert dunes and mesas | Kragmaw Hold | Dune Scorpions, Dust Hyenas, Sand Raiders and Shamans, the Sandstone Colossus |
| Elf | Silverbough Glade: a twilight forest of silver trees | Aelthas | Shadowfang Wolves, Thornback Boars, Satyrs, the Ancient Treant |
| Goblin | Grubdeep Caverns: a cave lit by crystals and mushrooms | Rustpocket | Cave Spiders, Stonehide Boars, Troggs, the Crystal Golem |
| Gnome | Frostcog Peaks: bright snowy mountains | Gearhaven | Snow Wolves, Frost Boars, Frost Trolls, the Yeti |
| Undead | Witherwood: a dying forest under a purple sky | Gravenhold | Ghoul Hounds, Plague Boars, Skeletons and Necromancers, the Bone Colossus |

### Classes

Every class starts with one ability and learns the rest at levels 2, 4, 6, 8
and 10. The seventh ability, on `E`, is learned at level 3 and is usually a
way to move: Charge, Blink, Disengage and so on.

| Class | Resource | Abilities (keys 1-6, then E) |
| --- | --- | --- |
| Barbarian | Rage | Savage Strike, Rend, Whirlwind, Skull Bash, Taunt, Recklessness, Charge |
| Fighter | Rage | Power Strike, Thunder Clap, Shield Bash, Taunt, Shield Wall, Second Wind, Shield Block |
| Paladin | Mana | Crusader Strike, Holy Light, Judgment, Hammer of Justice, Divine Protection, Consecration, Lay on Hands |
| Monk | Energy | Tiger Palm, Blackout Kick, Rising Sun Kick, Leg Sweep, Fortifying Brew, Spinning Crane Kick, Roll |
| Rogue | Energy | Sinister Strike, Eviscerate, Backstab, Gouge, Evasion, Kick, Sprint |
| Ranger | Energy | Steady Shot, Serpent Sting, Concussive Shot, Multi-Shot, Hawk Eye, Kill Shot, Disengage |
| Artificer | Mana | Arcane Rifle, Acid Flask, Shock Net, Thunder Grenade, Arcane Armor, Infused Tonic, Rocket Boots |
| Bard | Mana | Vicious Mockery, Healing Word, Thunderwave, Song of Rest, Inspire, Hypnotic Pattern, Dissonant Whispers |
| Cleric | Mana | Smite, Heal, Shadow Word: Pain, Renew, Power Word: Shield, Holy Nova, Flash Heal |
| Druid | Mana | Wrath, Rejuvenation, Moonfire, Entangling Roots, Healing Touch, Starfall, Dash |
| Mage | Mana | Fireball, Frostbolt, Fire Blast, Frost Nova, Ice Barrier, Evocation, Blink |
| Sorcerer | Mana | Chaos Bolt, Arcane Barrage, Hold Person, Meteor, Mana Shield, Wild Surge, Misty Step |
| Warlock | Mana | Eldritch Blast, Corruption, Drain Life, Curse of Weakness, Shadow Ward, Soul Fire, Siphon Soul |

Rage builds by hitting and being hit and drains out of combat. Energy comes
back quickly. Mana regenerates quickly once you haven't spent any for 5
seconds. Health regenerates out of combat. Rogues and Monks build combo points
and spend them on Eviscerate or Blackout Kick. Backstab only works from behind.
The Warrior is now the Barbarian; old Warrior characters load as Barbarians.

### Quests

Every town has a quest giver with a golden **!** over their head (a **?**
when you have a quest to hand in). Right-click them to see their three
quests:

- **A hunt**: kill eight of the area's hunters (wolves, scorpions, spiders...).
- **A crafting job**: gather materials, craft a piece of armor with `K`, and
  hand it over.
- **The elite**: slay the area's elite in its ruins (level 8 and up).

Each pays out money, experience and a piece of green gear. Your quests and
their progress show on the right of the screen, and their hunting grounds are
circled on the world map (`M`). Press `L` for your quest log, where you can
abandon a quest. Quests are saved with your character, and each can be done
once.

### Talents

From level 2 you earn a talent point every level (9 at level 10). Press `N`
to open your class's talent tree: three branches (a Mage has Fire, Frost and
Arcane, a Rogue Assassination, Combat and Subtlety, and so on) of three
talents each. The first talent in a branch takes up to 3 points, the second
2 and the last 1; the second opens once you've spent 2 points in that branch
and the last once you've spent 4. Talents make your abilities hit harder,
cast faster or come back sooner, or give you more health, critical strikes,
speed, toughness, regeneration or life drain. **Reset talents** gives back
every point.

### Loot and crafting

Mobs drop money and items. When a corpse sparkles, right-click it to take its
loot (everyone who fought it may loot it; the first to do so gets it).

- Boars (and some wolves) drop **Light Leather**
- Bandits (and the other humanoids) drop **Linen Cloth**
- Bandits, raiders and the other fighters also drop **Iron Scrap**
- Each starting area's elite drops an **Ancient Core**
- Any mob may drop a piece of **green** gear (rarely; elites usually do):
  helms, chests, gloves, legs and boots, each sturdy (armor and stamina) or
  arcane (power). Greens are better than crafted leather and linen, but not
  as good as the rare (blue) Heartstone Chestguard. Quests reward greens too
- Any mob may also drop a **green weapon** (rarely; elites often do): the
  Wolfbite Axe, Ironwood Mace, Shadowfang Dagger, Ashwood Longbow, Emberwand
  and Moonwhisper Staff

Open crafting with `K` to turn materials into gear: a leather cap, vest,
gloves, pants and boots; a linen hood, robe, gloves, pants and sandals; an
Iron Sword, Hunting Bow and Apprentice Staff; and the rare Heartstone
Chestguard and Heartstone Greatsword. Click gear in your backpack to wear or
hold it, and click it in the character window to take it off. Armor reduces
physical damage, a weapon adds damage to every auto attack, stamina adds
health, and power adds to the damage and healing you do. What you wear shows on
your character (weapons don't yet; your class's own weapon is drawn instead).
Any class can use any weapon.

### Merchants

Every town has a merchant in its square (Tobin Hale in Hearthmere, Joe in
Kragmaw Hold, Elarion in Aelthas, Migwick in Rustpocket, Nimble Cogsworth in
Gearhaven and Mortimer Graves in Gravenhold). Right-click them to open their wares:
Healing and Mana Potions (50 copper each), Light Leather and Linen Cloth. While
their window is open, right-click items in your bags to sell them for a quarter
of their price. Right-click a potion in your bags to drink it (potions share a
30 second cooldown).

### The world

Buildings, trees, rocks, fences and other scenery are solid to players. The town
in the middle of each area is safe, with a well, a cart, banners and signposts
on the roads out. Every area has its own map: its own hills, valleys and lakes,
its own town layout, and its own places for fields, camps and the elite's
ruins. Mob levels rise the further you go from town: hunters and grazers
(levels 1-8) roam the open, two camps of fighters and casters (4-5 and 6-8) and
a fighters' camp (8-9) sit further out, and the area's **elite** (10) waits in
its ruins near the edge.

Press `M` for the world map of your area, inked on parchment: the town, every
camp and its levels (tents for camps, paw prints for beasts, a skull for the
elite), the quest giver's **!**, your quests' hunting grounds circled in
yellow, and everyone nearby. The minimap in the corner shows the land itself. North is up on the map; the minimap in
the corner turns with your camera and shows N, E, S and W around its edge. Your
coordinates (relative to the town) are shown under the minimap.

Killing a mob gives experience. Mobs far below your level give none, and
everyone who fought a mob shares the kill. The level cap is 10. Mob level
numbers are colored by difficulty: grey (trivial), green, yellow, orange and
red (deadly). Bandits call nearby friends for help; animals fight alone.

When you die, **Release Spirit** brings you back at the graveyard in town with
half health.

The starting areas sit side by side, far apart; for now there are no roads
between them.

## How it's built

```
shared/   Game data (classes, abilities, mobs, items, recipes, talents, XP
          curve), the network protocol, message framing, each area's layout
          and terrain, and where all the scenery stands.
server/   The authoritative world simulation (combat, AI, threat, loot, XP),
          character saving, and the TCP server that runs it at 20 ticks per
          second.
client/   The macroquad client: login, character select and creation,
          movement, camera, targeting, 3D rendering and the HUD.
```

- **Server authority.** The server decides every hit, heal, cooldown, cast,
  drop and death. Clients send intentions ("use Fireball on target 42") and the
  server answers with events and a snapshot of everything within 110 yards.
- **Client-side movement.** Your own character moves locally, so it feels
  instant. The server checks each move against your run speed, roots and stuns,
  and snaps you back if a move isn't possible.
- **Saving.** Characters (level, experience, money, bags, gear and position)
  live in a JSON file on the server, written atomically every few seconds.
- **Protocol.** Length-prefixed [bincode](https://docs.rs/bincode) messages over
  TCP. Both sides check `PROTOCOL_VERSION` when a client logs in.
- **Data-driven content.** Abilities are lists of effects, and items, recipes
  and loot tables are plain tables in `shared/src/data.rs`, so adding a spell,
  an item or a drop is mostly adding a row.
- **Layouts.** Each area's terrain, town, fields and camp sites come from
  `shared/src/layout.rs`; camps are placed by a search for dry, gentle ground.
  Scenery is placed by a seeded generator in `shared/src/props.rs`, so what the
  client draws is exactly what players bump into and where the server spawns
  mobs. (Mobs don't collide with scenery yet.)
- **Rendering.** People are rigged 3D models (below); everything else is
  built from boxes, ellipsoids and cones in a custom batcher, and all of it is lit per pixel on the GPU: a warm sun, a cool sky fill and a
  warm bounce from the ground, a soft rim light, a faint painted grain and
  distance fog (`client/src/gfx/shader.rs`). Boxes shade like slightly rounded
  blocks and cones like round ones, and everything standing on the ground casts
  a soft contact shadow. Each area has its own light, sky, clouds, weather
  (falling leaves, blowing sand, fireflies, spores, snow and wisps) and fog.
  Each area's scenery is baked into meshes the first time you see it.
- **Client layout.** Graphics code is split so different people can work on it
  without editing the same files:
  - `client/src/gfx/`: the engine (batcher, model-building frames, shader and
    lighting, textures, `.glb` model loading, colors). The rest only calls
    what `gfx` exports.
  - `client/src/world/`: the scenery (terrain, sky and weather, water, trees
    and props, buildings, each zone's colors and light).
  - `client/src/models/`: characters and creatures, their gear and animation.
  - `client/src/vfx.rs`: spell effects.
  - `client/src/render.rs` only gathers the names the game and menus use.
  - Art files go in `client/assets/world/`, `client/assets/characters/` and
    `client/assets/fx/`, and get built into the exe.
- **Spell effects** (`client/src/vfx.rs`). Missiles look like their school
  (fireballs trailing flame, spinning frost shards, shadow bolts, arcane
  orbs, arrows and bullets) and burst where they land; melee abilities sweep
  a slash, area spells send out a shockwave, heals raise spirals and pillars
  of light, drains pour a beam back to the caster, casters stand in a
  glowing rune circle, and auras show on whoever has them: shield bubbles,
  stun stars, roots, frost, flames, poison, bleeding and more.
- **Models and animation.** Players, townsfolk and humanoid mobs are rigged,
  skinned models with skeletal animation, from the free (CC0) KayKit
  Adventurers and Skeletons packs (`client/assets/characters/`). All bodies
  share one skeleton and one set of animation clips; the `.glb` loader
  (`client/src/gfx/model_file.rs`) reads skeletons, skins and clips, and
  `client/src/models/rigged.rs` reshapes the packs' cartoon bodies toward
  heroic proportions (longer limbs and torso, smaller head, hands and feet,
  with the meshes stretched to fit), picks a body and props for each class or mob,
  repaints its texture for race, class and armor colors, and blends clips from
  what someone is doing: running, idling, casting, attacking (with the upper
  body only while running), flinching, jumping and dying. Every class fights
  in its own way: barbarians chop with a great axe, fighters slash behind a
  shield, monks punch and kick, rogues stab with two knives, rangers and
  artificers shoot crossbows, and casters channel and hurl spells. Creatures
  and giants are still built from shapes and posed in code: wolves sniff about
  and snap their jaws, boars root around, spiders rear up and scorpions sway
  their tails.

## Ideas for what's next

- Parties with shared XP and party frames
- Quests from NPCs in each town
- Weapons as loot, and more crafting recipes
- Dungeon instances, and roads or portals between the starting areas
- Account passwords
- Swimming and mounts
- UDP networking, interest management and delta snapshots for many players
