# Rusty MMO

A small 3D tab-targeting MMO written in Rust, in the spirit of World of Warcraft
and Final Fantasy XIV. Pick a Warrior, Mage or Cleric, then fight wolves, boars,
bandits and an elite golem across a hilly zone around a starter town. Play solo,
or run a server and adventure together.

Built with [macroquad](https://macroquad.rs) for the client and plain `std`
networking for the server.

## Running

```sh
cargo run --release
```

This opens the client. On the login screen, enter a name and pick a class, then:

- **Play Solo** starts a private server inside the game and connects to it.
- **Join Server** connects to the address in the box (`host` or `host:port`).

To host a game for friends, run the dedicated server:

```sh
cargo run --release -p server                # listens on 0.0.0.0:7878
cargo run --release -p server -- --port 9000
```

Then everyone uses **Join Server** with the host's address.

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
cargo build --release --target x86_64-pc-windows-gnu
```

The exes end up in `target/x86_64-pc-windows-gnu/release/` and run on their
own, with no installer or extra files needed.

## Versions

See [CHANGELOG.md](CHANGELOG.md). The current version is shown in the window
title and on the login screen.

## How to play

| Input | Action |
| --- | --- |
| `W` `A` `S` `D` / arrows | Run forward and back, turn left and right |
| `Q` `E` | Strafe |
| `Space` | Jump |
| Left mouse drag | Look around |
| Right mouse drag | Steer: your character turns with the camera (and `A` `D` strafe) |
| Both mouse buttons | Run forward |
| Mouse wheel | Zoom |
| `Tab` | Target the next enemy in view, nearest first |
| Left click | Target |
| Right click | Target and start attacking |
| `F1` | Target yourself |
| `1`-`6` or click the action bar | Use an ability |
| `T` | Start / stop auto attack |
| `Esc` | Clear target, or open the game menu |
| `Enter` | Chat (`/who` lists who's online) |
| `H` | Show / hide the controls |

### Combat

Combat works like a classic tab-target MMO:

- **Auto attack.** Your weapon (or a caster's wand) swings on a timer at your
  target while you're attacking.
- **Global cooldown.** Every ability triggers a 1.5 second GCD; some also have
  their own cooldown.
- **Cast times.** Spells like Fireball take time to cast, and moving or getting
  stunned interrupts them. Mobs cast too, and Shield Bash interrupts them.
- **Range and facing.** You have to be in range and facing your target. The
  action bar turns an ability's name red when you're out of range, and shades
  it blue when you can't afford it.
- **Threat.** Mobs attack whoever has made them angriest. Heroic Strike and
  Thunder Clap make extra threat, Taunt pulls a mob straight to you, and
  healing someone draws the attention of whatever is fighting them.
- **Buffs and debuffs.** Damage and heals over time, slows, roots, stuns and
  absorb shields, shown as icons with timers under the unit frames.
- **Leashing.** Chase a mob too far from home and it gives up, runs back and
  heals to full.

### Classes

| Class | Resource | Abilities |
| --- | --- | --- |
| Warrior | Rage: builds by hitting and being hit, drains out of combat | Heroic Strike, Rend, Shield Bash, Thunder Clap, Taunt, Second Wind |
| Mage | Mana | Fireball, Frostbolt, Fire Blast, Frost Nova, Ice Barrier, Evocation |
| Cleric | Mana | Smite, Shadow Word: Pain, Heal, Renew, Power Word: Shield, Holy Nova |

Mana regenerates quickly once you haven't spent any for 5 seconds. Health
regenerates out of combat.

### The world

The town in the middle (0, 0) is safe. Mob levels rise the further out you go.
Your coordinates are shown under the minimap.

| Near | Mobs | Levels |
| --- | --- | --- |
| 50, 25 | Gray Wolves | 1-2 |
| -45, 40 | Wild Boars (neutral: they only fight back) | 1-3 |
| 15, -60 | Gray Wolves | 2-3 |
| -85, -35 | Wild Boars | 3-5 |
| 90, -75 | Bandit camp: Thugs and Mystics (shadow casters) | 4-5 |
| 20, 110 | Gray Wolves | 5-6 |
| 120, 90 | Bandit camp | 6-8 |
| 155, -20 | Gray Wolves | 7-8 |
| -120, -120 | Bandit camp: Thugs | 8-9 |
| -150, -150 | Ruins of the **Ancient Golem** (elite, Ground Slam) | 10 |

Killing a mob gives experience. Mobs far below your level give none, and
everyone who fought a mob shares the kill. The level cap is 10. Mob level
numbers are colored by difficulty: grey (trivial), green, yellow, orange and
red (deadly). Bandits call nearby friends for help; animals fight alone.

When you die, **Release Spirit** brings you back at the graveyard in town with
half health.

## How it's built

```
shared/   Game data (classes, abilities, mobs, XP curve), the network protocol,
          message framing and the terrain height function.
server/   The authoritative world simulation (combat, AI, threat, XP) and the
          TCP server that runs it at 20 ticks per second.
client/   The macroquad client: login screen, movement, camera, targeting,
          3D rendering and the HUD.
```

- **Server authority.** The server decides every hit, heal, cooldown, cast and
  death. Clients send intentions ("use Fireball on target 42") and the server
  answers with events and a snapshot of everything within 110 yards.
- **Client-side movement.** Your own character moves locally, so it feels
  instant. The server checks each move against your run speed, roots and stuns,
  and snaps you back if a move isn't possible.
- **Protocol.** Length-prefixed [bincode](https://docs.rs/bincode) messages over
  TCP. Both sides check `PROTOCOL_VERSION` when a client logs in.
- **Data-driven abilities.** Each ability in `shared/src/data.rs` is a list of
  effects (damage, heal, aura, interrupt, taunt...), so adding a spell is
  mostly adding a table entry.
- **Rendering.** Everything is built from shaded boxes, spheres and cones in a
  custom batcher (`client/src/render.rs`). Static scenery is baked into meshes
  once at startup.

## Ideas for what's next

- Parties with shared XP and party frames
- Loot, inventory, equipment and gold
- Quests and NPCs in town
- Talents or a skill tree per class
- Saving characters between sessions
- Dungeon instances and more zones
- Swimming, mounts and a world map
- UDP networking, interest management and delta snapshots for many players
