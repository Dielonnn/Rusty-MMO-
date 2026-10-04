# Rusty MMO

A small 3D tab-targeting MMO written in Rust, in the spirit of World of Warcraft
and Final Fantasy XIV. Pick one of six races and thirteen classes, and start in
your race's own starting area: autumn hills, a desert, an elven forest, a
goblin cave, snowy peaks or a dying forest, each with its own town, merchant,
creatures and elite. Loot leather and cloth, craft armor, buy potions, and level
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

Then pick a character or create one: choose a name, race, class, build, skin,
hair style and hair color, and press **Enter World**. You start in your race's
starting area.

To host a game for friends, run the dedicated server:

```sh
cargo run --release -p server                       # listens on 0.0.0.0:7878
cargo run --release -p server -- --port 9000 --save my_world.json
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
| Right click | Attack an enemy, loot a sparkling corpse, or trade with a merchant |
| `F1` | Target yourself |
| `1`-`6`, `E`, or click the action bar | Use an ability |
| `T` | Start / stop auto attack |
| `B` / `C` / `K` | Backpack / character / crafting |
| `Esc` | Close windows, clear target, or open the game menu |
| `Enter` | Chat (`/who` lists who's online) |
| `H` | Show / hide the controls |

Your character turns to face where the camera looks whenever you move, and
turns to face your target when you attack while standing still.

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

### Loot and crafting

Mobs drop money and items. When a corpse sparkles, right-click it to take its
loot (everyone who fought it may loot it; the first to do so gets it).

- Boars (and some wolves) drop **Light Leather**
- Bandits (and the other humanoids) drop **Linen Cloth**
- Each starting area's elite drops an **Ancient Core**

Open crafting with `K` to turn materials into armor: a leather cap, vest,
gloves, pants and boots; a linen hood, robe and pants; and the Heartstone
Chestguard. Click armor in your backpack to wear it, and click it in the
character window to take it off. Armor reduces physical damage, stamina adds
health, and power adds to the damage and healing you do. What you wear shows on
your character.

### Merchants

Every town has a merchant in its square. Right-click them to open their wares:
Healing and Mana Potions (50 copper each), Light Leather and Linen Cloth. While
their window is open, right-click items in your bags to sell them for a quarter
of their price. Right-click a potion in your bags to drink it (potions share a
30 second cooldown).

### The world

Buildings, trees, rocks, fences and other scenery are solid to players. The town in the
middle of each area is safe. Mob levels rise the further you go from town. Your
coordinates (relative to the town) are shown under the minimap.

| Near | Mobs | Levels |
| --- | --- | --- |
| 50, 25 | The area's hunters (wolves, scorpions, spiders...) | 1-2 |
| -45, 40 | The area's grazers (boars, hyenas: neutral, they only fight back) | 1-3 |
| 15, -60 | Hunters | 2-3 |
| -85, -35 | Grazers | 3-5 |
| 90, -75 | A camp of fighters and casters | 4-5 |
| 20, 110 | Hunters | 5-6 |
| 120, 90 | A camp | 6-8 |
| 155, -20 | Hunters | 7-8 |
| -120, -120 | A camp of fighters | 8-9 |
| -150, -150 | The ruins of the area's **elite** | 10 |

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
shared/   Game data (classes, abilities, mobs, items, recipes, XP curve), the
          network protocol, message framing and the terrain height function.
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
- **Shared layout.** Every starting area uses the same layout, turned and
  mirrored differently (`shared/src/world.rs`). Scenery is placed by a seeded
  generator in `shared/src/props.rs`, so what the client draws is exactly what
  players bump into. (Mobs don't collide with scenery yet.)
- **Rendering.** Everything is built from shaded boxes, ellipsoids and cones in
  a custom batcher (`client/src/render.rs`). Each area has its own light, sky
  and distance fog (a small GLSL shader). Each area's scenery is baked into
  meshes the first time you see it.

## Ideas for what's next

- Parties with shared XP and party frames
- Quests from NPCs in each town
- Weapons as loot, and more crafting recipes
- Talents or a skill tree per class
- Dungeon instances, and roads or portals between the starting areas
- Account passwords
- Swimming, mounts and a world map
- UDP networking, interest management and delta snapshots for many players
