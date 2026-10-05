//! The sound engine: makes every sound effect once at startup (see
//! `synth.rs`) and plays them at the volumes picked in Settings.
//!
//! Interface sounds play at full volume. Sounds in the world get quieter
//! with distance from you and aren't played at all past `HEARING`.
//!
//! Music and ambience (see `music.rs`) are composed on a background thread
//! the first time they're wanted, then loop, fading over when you move to
//! a place with different music.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume, stop_sound,
};
use macroquad::prelude::*;
use shared::data::{HumanoidStyle, MobKind, MobModel, School};

use crate::music::{self, Ambience, Track};
use crate::settings::{self, Bus};
use crate::synth::{Rng, Shape, Wave, bell, noise, note, pluck, sweep, tone};

/// How far away (in yards) a sound in the world can be heard. Placeholder.
pub const HEARING: f32 = 40.0;
/// Closer than this, world sounds play at full volume. Placeholder.
const FULL_VOLUME: f32 = 8.0;
/// The same sound won't start again within this many seconds, so a
/// big fight doesn't stack dozens of copies.
const REPEAT_GAP: f32 = 0.05;
/// At most this many sounds start in one frame; the rest are dropped.
const MAX_PER_FRAME: usize = 8;
/// How many seconds music and ambience take to fade in or out. Placeholder.
const FADE: f32 = 2.5;

/// Every sound effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sfx {
    // Interface
    Click,
    WindowOpen,
    WindowClose,
    Pickup,
    Drop,
    Equip,
    Coins,
    RareDrop,
    Error,
    Message,
    Invite,
    QuestAccept,
    QuestComplete,
    LevelUp,
    SkillUp,
    // Combat
    Swing,
    Hit,
    Crit,
    Hurt,
    Death,
    Bow,
    Heal,
    Eat,
    Drink,
    Cast(School),
    Impact(School),
    // Monsters
    Mob(Voice, Cry),
    /// Marked ground about to be hit (the Sunken King's Tidal Crash).
    Warning,
    /// Marked ground being hit.
    Crash,
    /// A dungeon boss starting a spell.
    BossCast,
    /// A dungeon boss dying.
    BossDown,
    /// The way out opening after a boss.
    Portal,
    // World
    /// Travelling by waystone or teleporter.
    Teleport,
    /// A footstep; the flag picks one of two so steps don't repeat exactly.
    Step(Surface, bool),
}

/// The kinds of monster voice. Monsters built on the same body share one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Voice {
    /// Wolves and hounds.
    Beast,
    Boar,
    /// Spiders, scorpions and crabs.
    Bug,
    /// Bandits, cultists, trolls and other people.
    Person,
    /// Skeletons.
    Bones,
    /// Golems, colossi, yetis and treants.
    Giant,
}

/// What a monster's voice is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cry {
    /// Starting a fight.
    Aggro,
    Hurt,
    Death,
}

/// What you're walking on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    Grass,
    Sand,
    Snow,
    Stone,
    Water,
}

const VOICES: [Voice; 6] = [
    Voice::Beast,
    Voice::Boar,
    Voice::Bug,
    Voice::Person,
    Voice::Bones,
    Voice::Giant,
];
const SURFACES: [Surface; 5] = [
    Surface::Grass,
    Surface::Sand,
    Surface::Snow,
    Surface::Stone,
    Surface::Water,
];

/// The voice a monster has, from the body it's built on.
pub fn voice(kind: MobKind) -> Voice {
    match kind.template().model {
        MobModel::Wolf => Voice::Beast,
        MobModel::Boar => Voice::Boar,
        MobModel::Spider | MobModel::Scorpion => Voice::Bug,
        MobModel::Humanoid(HumanoidStyle::Skeleton) => Voice::Bones,
        MobModel::Humanoid(_) => Voice::Person,
        MobModel::Giant(_) => Voice::Giant,
    }
}

const SCHOOLS: [School; 7] = [
    School::Physical,
    School::Fire,
    School::Frost,
    School::Arcane,
    School::Holy,
    School::Shadow,
    School::Nature,
];

impl Sfx {
    fn all() -> Vec<Sfx> {
        use Sfx::*;
        let mut all = vec![
            Click,
            WindowOpen,
            WindowClose,
            Pickup,
            Drop,
            Equip,
            Coins,
            RareDrop,
            Error,
            Message,
            Invite,
            QuestAccept,
            QuestComplete,
            LevelUp,
            SkillUp,
            Swing,
            Hit,
            Crit,
            Hurt,
            Death,
            Bow,
            Heal,
            Eat,
            Drink,
        ];
        all.extend(SCHOOLS.iter().map(|&s| Cast(s)));
        all.extend(SCHOOLS.iter().map(|&s| Impact(s)));
        for v in VOICES {
            all.extend([Cry::Aggro, Cry::Hurt, Cry::Death].map(|c| Mob(v, c)));
        }
        all.extend([Warning, Crash, BossCast, BossDown, Portal, Teleport]);
        for surface in SURFACES {
            all.extend([Step(surface, false), Step(surface, true)]);
        }
        all
    }

    fn bus(self) -> Bus {
        use Sfx::*;
        match self {
            Click | WindowOpen | WindowClose | Pickup | Drop | Equip | Coins | RareDrop | Error
            | Message | Invite | QuestAccept | QuestComplete | LevelUp | SkillUp => Bus::Interface,
            _ => Bus::Effects,
        }
    }

    /// Builds the sound.
    fn make(self) -> Wave {
        self.build().fade_tail(0.015)
    }

    fn build(self) -> Wave {
        use Sfx::*;
        match self {
            Click => tone(Shape::Sine, 1900.0, 1300.0, 0.035, 0.001, 3.0)
                .mix(0.0, noise(0.015, 0.001, 2.0, 3).highpass(3000.0).gain(0.3))
                .normalize(0.5),
            WindowOpen => sweep(0.12, 900.0, 4000.0, 11)
                .gain(0.7)
                .mix(
                    0.03,
                    tone(Shape::Sine, note(3), note(10), 0.08, 0.005, 2.0).gain(0.25),
                )
                .normalize(0.45),
            WindowClose => sweep(0.1, 3500.0, 700.0, 12)
                .gain(0.7)
                .mix(
                    0.02,
                    tone(Shape::Sine, note(7), note(-2), 0.07, 0.005, 2.0).gain(0.25),
                )
                .normalize(0.4),
            Pickup => tone(Shape::Triangle, note(0), note(12), 0.09, 0.003, 1.5)
                .mix(
                    0.07,
                    tone(Shape::Sine, note(19), note(19), 0.12, 0.003, 2.0).gain(0.5),
                )
                .normalize(0.5),
            Drop => tone(Shape::Sine, 160.0, 60.0, 0.18, 0.002, 2.0)
                .mix(0.0, noise(0.08, 0.001, 2.0, 21).lowpass(900.0))
                .normalize(0.7),
            Equip => noise(0.07, 0.001, 2.0, 31)
                .lowpass(2500.0)
                .mix(0.03, bell(1250.0, 0.25).gain(0.35))
                .mix(0.07, bell(1650.0, 0.2).gain(0.25))
                .normalize(0.55),
            Coins => [0.0, 0.05, 0.11, 0.14, 0.2]
                .into_iter()
                .enumerate()
                .fold(Wave::new(), |w, (i, at)| {
                    w.mix(at, bell(2600.0 + i as f32 * 230.0, 0.22).gain(0.6))
                })
                .normalize(0.45),
            RareDrop => [12, 16, 19, 24, 28]
                .into_iter()
                .enumerate()
                .fold(Wave::new(), |w, (i, n)| {
                    w.mix(i as f32 * 0.07, bell(note(n), 0.7).gain(0.5))
                })
                .echo(0.11, 0.3)
                .normalize(0.55),
            Error => tone(Shape::Square, 190.0, 180.0, 0.09, 0.003, 1.0)
                .mix(0.12, tone(Shape::Square, 150.0, 140.0, 0.12, 0.003, 1.0))
                .lowpass(1600.0)
                .normalize(0.3),
            Message => tone(Shape::Sine, note(14), note(14), 0.12, 0.003, 2.0)
                .mix(
                    0.08,
                    tone(Shape::Sine, note(19), note(19), 0.18, 0.003, 2.0),
                )
                .normalize(0.35),
            Invite => bell(note(7), 0.6)
                .mix(0.16, bell(note(12), 0.8))
                .normalize(0.5),
            QuestAccept => [0, 4, 7]
                .into_iter()
                .enumerate()
                .fold(Wave::new(), |w, (i, n)| {
                    w.mix(
                        i as f32 * 0.09,
                        tone(Shape::Triangle, note(n), note(n), 0.3, 0.005, 2.0),
                    )
                })
                .echo(0.13, 0.25)
                .normalize(0.5),
            QuestComplete => fanfare(&[0, 4, 7, 12], 0.11, 1.1),
            LevelUp => fanfare(&[-5, 0, 4, 7, 12, 16], 0.08, 1.6)
                .mix(0.4, sweep(1.0, 2000.0, 9000.0, 41).gain(0.15))
                .normalize(0.6),
            SkillUp => tone(Shape::Triangle, note(12), note(12), 0.12, 0.004, 2.0)
                .mix(
                    0.08,
                    tone(Shape::Triangle, note(19), note(19), 0.25, 0.004, 2.0),
                )
                .normalize(0.45),
            Swing => sweep(0.2, 500.0, 2500.0, 51).normalize(0.5),
            Hit => thump(110.0, 61, 0.6),
            Crit => thump(90.0, 62, 0.75)
                .mix(0.0, bell(900.0, 0.25).gain(0.3))
                .normalize(0.8),
            Hurt => thump(80.0, 71, 0.6)
                .mix(
                    0.01,
                    tone(Shape::Saw, 150.0, 95.0, 0.16, 0.01, 1.5)
                        .lowpass(700.0)
                        .gain(0.6),
                )
                .normalize(0.65),
            Death => tone(Shape::Saw, 220.0, 70.0, 0.9, 0.01, 1.5)
                .lowpass(800.0)
                .mix(0.0, tone(Shape::Sine, 110.0, 40.0, 1.0, 0.01, 1.2))
                .normalize(0.6),
            Bow => pluck(196.0, 0.35, 0.995, 81)
                .mix(0.02, sweep(0.12, 2500.0, 6000.0, 82).gain(0.4))
                .normalize(0.55),
            Heal => [0, 7, 12, 16]
                .into_iter()
                .enumerate()
                .fold(Wave::new(), |w, (i, n)| {
                    w.mix(
                        i as f32 * 0.06,
                        tone(Shape::Sine, note(n + 3), note(n + 3), 0.5, 0.02, 2.0),
                    )
                })
                .normalize(0.4),
            Eat => [0.0, 0.12, 0.24]
                .into_iter()
                .enumerate()
                .fold(Wave::new(), |w, (i, at)| {
                    w.mix(at, noise(0.07, 0.003, 1.5, 91 + i as u32).lowpass(1800.0))
                })
                .normalize(0.45),
            Drink => [0.0, 0.07, 0.15, 0.21, 0.3]
                .into_iter()
                .enumerate()
                .fold(Wave::new(), |w, (i, at)| {
                    let f = 380.0 + (i as f32 * 1.7).sin().abs() * 300.0;
                    w.mix(at, tone(Shape::Sine, f, f * 1.8, 0.06, 0.003, 1.5))
                })
                .normalize(0.4),
            Cast(school) => cast(school),
            Impact(school) => impact(school),
            Mob(v, c) => mob_voice(v, c),
            Warning => tone(Shape::Saw, 98.0, 147.0, 0.9, 0.05, 0.6)
                .mix(0.0, tone(Shape::Saw, 99.0, 148.5, 0.9, 0.05, 0.6))
                .lowpass(900.0)
                .tremolo(9.0, 0.5)
                .normalize(0.55),
            Crash => noise(1.0, 0.002, 2.0, 201)
                .lowpass(700.0)
                .mix(0.0, tone(Shape::Sine, 70.0, 28.0, 0.9, 0.002, 1.5))
                .mix(0.05, sweep(0.6, 2500.0, 400.0, 202).gain(0.5))
                .normalize(0.8),
            BossCast => tone(Shape::Saw, 73.4, 73.4, 1.1, 0.25, 0.8)
                .mix(0.0, tone(Shape::Saw, 110.0, 110.0, 1.1, 0.25, 0.8))
                .mix(0.0, tone(Shape::Saw, 87.3, 87.3, 1.1, 0.25, 0.8).gain(0.7))
                .lowpass(650.0)
                .normalize(0.6),
            BossDown => fanfare(&[0, 4, 7, 12, 16, 19], 0.13, 2.2)
                .mix(
                    0.0,
                    tone(Shape::Sine, 65.0, 40.0, 1.0, 0.005, 1.5).gain(0.6),
                )
                .normalize(0.65),
            Portal => sweep(1.2, 400.0, 6000.0, 211)
                .gain(0.6)
                .mix(0.3, bell(note(19), 1.2).gain(0.5))
                .mix(0.5, bell(note(26), 1.0).gain(0.4))
                .normalize(0.5),
            Teleport => sweep(0.7, 300.0, 7000.0, 221)
                .mix(0.35, bell(note(24), 0.9).gain(0.6))
                .echo(0.12, 0.3)
                .normalize(0.55),
            Step(surface, alt) => step(surface, if alt { 2 } else { 1 }),
        }
    }
}

/// A monster's cry. Each voice is built differently: growls are buzzy
/// tones that wobble, bugs hiss and click, giants rumble.
fn mob_voice(v: Voice, c: Cry) -> Wave {
    let seed = 300 + VOICES.iter().position(|x| *x == v).unwrap_or(0) as u32 * 10;
    match (v, c) {
        (Voice::Beast, Cry::Aggro) => tone(Shape::Saw, 120.0, 95.0, 0.7, 0.08, 0.8)
            .lowpass(700.0)
            .tremolo(32.0, 0.6)
            .normalize(0.6),
        (Voice::Beast, Cry::Hurt) => tone(Shape::Triangle, 700.0, 1100.0, 0.08, 0.005, 1.0)
            .mix(0.08, tone(Shape::Triangle, 1000.0, 500.0, 0.12, 0.005, 1.5))
            .normalize(0.45),
        (Voice::Beast, Cry::Death) => tone(Shape::Triangle, 900.0, 300.0, 0.6, 0.01, 1.2)
            .mix(
                0.0,
                tone(Shape::Saw, 110.0, 70.0, 0.5, 0.01, 1.5)
                    .lowpass(500.0)
                    .gain(0.5),
            )
            .normalize(0.5),
        (Voice::Boar, Cry::Aggro) => [0.0, 0.18]
            .into_iter()
            .fold(Wave::new(), |w, at| {
                w.mix(
                    at,
                    tone(Shape::Square, 85.0, 65.0, 0.16, 0.01, 1.2)
                        .lowpass(500.0)
                        .tremolo(25.0, 0.5),
                )
            })
            .normalize(0.6),
        (Voice::Boar, Cry::Hurt) => tone(Shape::Saw, 900.0, 1500.0, 0.18, 0.01, 1.2)
            .lowpass(3000.0)
            .normalize(0.4),
        (Voice::Boar, Cry::Death) => tone(Shape::Saw, 1300.0, 500.0, 0.5, 0.01, 1.2)
            .lowpass(2500.0)
            .mix(
                0.3,
                tone(Shape::Square, 80.0, 50.0, 0.3, 0.01, 1.5).lowpass(400.0),
            )
            .normalize(0.5),
        (Voice::Bug, Cry::Aggro) => noise(0.5, 0.05, 1.2, seed)
            .highpass(3000.0)
            .tremolo(40.0, 0.7)
            .normalize(0.4),
        (Voice::Bug, Cry::Hurt) => clicks(5, 0.03, seed + 1).normalize(0.45),
        (Voice::Bug, Cry::Death) => clicks(12, 0.045, seed + 2)
            .mix(
                0.0,
                noise(0.6, 0.01, 2.0, seed + 3).highpass(2000.0).gain(0.4),
            )
            .normalize(0.45),
        (Voice::Person, Cry::Aggro) => tone(Shape::Saw, 190.0, 160.0, 0.35, 0.02, 0.8)
            .lowpass(1300.0)
            .mix(0.0, noise(0.3, 0.02, 1.0, seed).lowpass(2500.0).gain(0.25))
            .normalize(0.55),
        (Voice::Person, Cry::Hurt) => tone(Shape::Saw, 170.0, 120.0, 0.16, 0.01, 1.5)
            .lowpass(1000.0)
            .normalize(0.5),
        (Voice::Person, Cry::Death) => tone(Shape::Saw, 180.0, 75.0, 0.8, 0.01, 1.2)
            .lowpass(900.0)
            .normalize(0.5),
        (Voice::Bones, Cry::Aggro) => clicks(10, 0.04, seed)
            .mix(
                0.05,
                tone(Shape::Sine, 110.0, 100.0, 0.6, 0.1, 1.0)
                    .tremolo(6.0, 0.4)
                    .gain(0.6),
            )
            .normalize(0.5),
        (Voice::Bones, Cry::Hurt) => clicks(4, 0.03, seed + 1).normalize(0.5),
        (Voice::Bones, Cry::Death) => clicks(18, 0.035, seed + 2)
            .mix(
                0.15,
                noise(0.4, 0.005, 2.0, seed + 3).lowpass(1500.0).gain(0.5),
            )
            .normalize(0.55),
        (Voice::Giant, Cry::Aggro) => tone(Shape::Saw, 70.0, 55.0, 1.2, 0.1, 0.8)
            .mix(0.0, noise(1.2, 0.1, 0.8, seed).lowpass(400.0).gain(0.8))
            .lowpass(600.0)
            .tremolo(18.0, 0.5)
            .normalize(0.7),
        (Voice::Giant, Cry::Hurt) => thump(60.0, seed + 1, 0.6),
        (Voice::Giant, Cry::Death) => (0..8)
            .fold(tone(Shape::Sine, 60.0, 25.0, 1.4, 0.005, 1.2), |w, i| {
                w.mix(
                    0.1 + i as f32 * 0.12,
                    noise(0.25, 0.003, 2.0, seed + 10 + i).lowpass(900.0 - i as f32 * 60.0),
                )
            })
            .normalize(0.7),
    }
}

/// A run of quick clicks: legs, pincers, bones.
fn clicks(count: u32, gap: f32, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let mut w = Wave::new();
    let mut at = 0.0;
    for i in 0..count {
        w = w.mix(at, noise(0.012, 0.0005, 3.0, seed + i).highpass(1800.0));
        at += gap * (0.7 + rng.next().abs() * 0.6);
    }
    w
}

/// One footstep. `variant` changes the noise so steps don't repeat exactly.
fn step(surface: Surface, variant: u32) -> Wave {
    let seed = 400 + variant * 7;
    match surface {
        Surface::Grass => noise(0.07, 0.004, 2.0, seed)
            .lowpass(1600.0)
            .highpass(200.0)
            .normalize(0.3),
        Surface::Sand => noise(0.1, 0.01, 1.5, seed)
            .lowpass(2600.0)
            .highpass(500.0)
            .normalize(0.25),
        Surface::Snow => clicks(6, 0.012, seed)
            .lowpass(3500.0)
            .mix(
                0.0,
                noise(0.08, 0.005, 1.5, seed + 1).lowpass(1200.0).gain(0.6),
            )
            .normalize(0.3),
        Surface::Stone => noise(0.03, 0.001, 3.0, seed)
            .highpass(900.0)
            .mix(
                0.0,
                tone(Shape::Sine, 160.0, 110.0, 0.05, 0.001, 2.0).gain(0.7),
            )
            .normalize(0.35),
        Surface::Water => noise(0.16, 0.005, 1.5, seed)
            .lowpass(2200.0)
            .mix(
                0.02,
                tone(
                    Shape::Sine,
                    500.0 + variant as f32 * 60.0,
                    900.0,
                    0.06,
                    0.003,
                    1.5,
                )
                .gain(0.3),
            )
            .normalize(0.35),
    }
}

/// Rising notes ringing into a chord.
fn fanfare(notes: &[i32], step: f32, ring: f32) -> Wave {
    notes
        .iter()
        .enumerate()
        .fold(Wave::new(), |w, (i, &n)| {
            let len = ring - i as f32 * step;
            w.mix(
                i as f32 * step,
                tone(Shape::Triangle, note(n), note(n), len, 0.01, 1.8).mix(
                    0.0,
                    tone(Shape::Sine, note(n + 12), note(n + 12), len, 0.01, 2.5).gain(0.3),
                ),
            )
        })
        .echo(0.15, 0.25)
        .normalize(0.55)
}

/// A body blow: a low drop in pitch with a burst of noise on top.
fn thump(f: f32, seed: u32, peak: f32) -> Wave {
    tone(Shape::Sine, f * 1.6, f * 0.5, 0.2, 0.001, 2.0)
        .mix(0.0, noise(0.09, 0.001, 2.5, seed).lowpass(1800.0).gain(0.8))
        .normalize(peak)
}

/// The sound of starting a spell of each school.
fn cast(school: School) -> Wave {
    match school {
        School::Physical => sweep(0.18, 600.0, 2400.0, 101).normalize(0.45),
        School::Fire => sweep(0.35, 300.0, 1800.0, 102)
            .mix(0.0, noise(0.35, 0.05, 1.0, 103).lowpass(500.0).gain(0.6))
            .normalize(0.5),
        School::Frost => [24, 31, 28, 36]
            .into_iter()
            .enumerate()
            .fold(sweep(0.3, 3000.0, 7000.0, 104).gain(0.3), |w, (i, n)| {
                w.mix(i as f32 * 0.05, bell(note(n), 0.3).gain(0.4))
            })
            .normalize(0.4),
        School::Arcane => tone(Shape::Sine, note(0), note(24), 0.35, 0.02, 1.0)
            .mix(
                0.0,
                tone(Shape::Triangle, note(7), note(31), 0.35, 0.02, 1.0).gain(0.5),
            )
            .normalize(0.35),
        School::Holy => [0, 4, 7, 12]
            .into_iter()
            .fold(Wave::new(), |w, n| {
                w.mix(
                    0.0,
                    tone(Shape::Sine, note(n + 5), note(n + 5), 0.5, 0.15, 1.5),
                )
            })
            .normalize(0.35),
        School::Shadow => tone(Shape::Saw, 110.0, 82.0, 0.45, 0.1, 1.2)
            .mix(0.0, tone(Shape::Saw, 116.5, 87.0, 0.45, 0.1, 1.2))
            .lowpass(600.0)
            .normalize(0.45),
        School::Nature => [0.0, 0.08, 0.16]
            .into_iter()
            .enumerate()
            .fold(Wave::new(), |w, (i, at)| {
                w.mix(
                    at,
                    pluck(note(-5 + i as i32 * 5), 0.3, 0.99, 105 + i as u32),
                )
            })
            .normalize(0.4),
    }
}

/// The sound of a spell of each school landing.
fn impact(school: School) -> Wave {
    match school {
        School::Physical => thump(110.0, 111, 0.6),
        School::Fire => noise(0.45, 0.005, 1.6, 112)
            .lowpass(1400.0)
            .mix(0.0, tone(Shape::Sine, 140.0, 50.0, 0.3, 0.002, 2.0))
            .normalize(0.65),
        School::Frost => noise(0.25, 0.001, 2.0, 113)
            .highpass(2500.0)
            .mix(0.0, bell(note(29), 0.4).gain(0.5))
            .mix(0.02, bell(note(34), 0.3).gain(0.3))
            .normalize(0.5),
        School::Arcane => tone(Shape::Square, note(19), note(-5), 0.22, 0.002, 1.5)
            .lowpass(3000.0)
            .mix(
                0.0,
                tone(Shape::Sine, note(31), note(7), 0.2, 0.002, 2.0).gain(0.5),
            )
            .normalize(0.45),
        School::Holy => bell(note(12), 0.9)
            .mix(0.0, bell(note(19), 0.8).gain(0.6))
            .normalize(0.45),
        School::Shadow => tone(Shape::Saw, 90.0, 45.0, 0.4, 0.003, 1.5)
            .lowpass(500.0)
            .mix(0.0, noise(0.3, 0.003, 1.5, 116).lowpass(400.0))
            .normalize(0.6),
        School::Nature => noise(0.18, 0.002, 2.0, 117)
            .lowpass(2200.0)
            .mix(0.0, pluck(note(-17), 0.3, 0.98, 118).gain(0.7))
            .normalize(0.55),
    }
}

/// A looping piece of music or ambience.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Loop {
    Music(Track),
    Ambience(Ambience),
}

impl Loop {
    fn bus(self) -> Bus {
        match self {
            Loop::Music(_) => Bus::Music,
            Loop::Ambience(_) => Bus::Ambience,
        }
    }

    fn same_kind(self, other: Loop) -> bool {
        matches!(
            (self, other),
            (Loop::Music(_), Loop::Music(_)) | (Loop::Ambience(_), Loop::Ambience(_))
        )
    }
}

/// A loop that's playing, fading towards `target` (0 or 1).
struct Layer {
    what: Loop,
    sound: Sound,
    gain: f32,
    target: f32,
    /// The volume last sent to the mixer.
    volume: f32,
}

struct Engine {
    /// Every sound, and when it last started by the game clock.
    sounds: Vec<(Sfx, Sound, f64)>,
    minimized: bool,
    subscriber: usize,
    /// The music and ambience wanted now.
    want: [Option<Loop>; 2],
    /// Loops composed and loaded, ready to play.
    ready: HashMap<Loop, Sound>,
    /// Loops being composed on the background thread.
    composing: HashSet<Loop>,
    done_tx: Sender<(Loop, Vec<u8>)>,
    done_rx: Receiver<(Loop, Vec<u8>)>,
    layers: Vec<Layer>,
    /// Sounds started this frame.
    started: usize,
}

/// Loads a sound right away. Loading is only ever slow on the web, so on
/// desktop the future is done the first time it's asked.
fn load_now(bytes: &[u8]) -> Option<Sound> {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    let mut fut = std::pin::pin!(load_sound_from_bytes(bytes));
    match fut.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(Ok(sound)) => Some(sound),
        _ => None,
    }
}

impl Engine {
    /// Starts composing anything wanted that isn't ready, takes in finished
    /// loops, and fades loops in and out.
    fn update_loops(&mut self, dt: f32) {
        while let Ok((what, wav)) = self.done_rx.try_recv() {
            self.composing.remove(&what);
            if let Some(sound) = load_now(&wav) {
                self.ready.insert(what, sound);
            }
        }
        for what in self.want.into_iter().flatten() {
            if !self.ready.contains_key(&what) && self.composing.insert(what) {
                let tx = self.done_tx.clone();
                std::thread::spawn(move || {
                    let wave = match what {
                        Loop::Music(t) => music::compose(t),
                        Loop::Ambience(a) => music::ambience(a),
                    };
                    let _ = tx.send((what, wave.to_wav()));
                });
            }
        }

        // Fade in what's wanted once it's ready; fade out the rest.
        for layer in &mut self.layers {
            layer.target = if self.want.contains(&Some(layer.what)) {
                1.0
            } else {
                0.0
            };
        }
        for what in self.want.into_iter().flatten() {
            let playing = self.layers.iter().any(|l| l.what == what);
            if let (false, Some(sound)) = (playing, self.ready.get(&what)) {
                // Wait for the old one of its kind to fade out first, so
                // two tunes don't play over each other.
                let busy = self
                    .layers
                    .iter()
                    .any(|l| l.what.same_kind(what) && l.gain > 0.25);
                if !busy {
                    play_sound(
                        sound,
                        PlaySoundParams {
                            looped: true,
                            volume: 0.0,
                        },
                    );
                    self.layers.push(Layer {
                        what,
                        sound: sound.clone(),
                        gain: 0.0,
                        target: 1.0,
                        volume: 0.0,
                    });
                }
            }
        }

        let s = settings::current();
        let muted = s.mute_all || (s.mute_in_background && self.minimized);
        let step = dt / FADE;
        for layer in &mut self.layers {
            layer.gain = if layer.target > layer.gain {
                (layer.gain + step).min(layer.target)
            } else {
                (layer.gain - step).max(layer.target)
            };
            let volume = if muted {
                0.0
            } else {
                layer.gain * s.volume(Bus::Master) * s.volume(layer.what.bus())
            };
            if (volume - layer.volume).abs() > 0.001 {
                layer.volume = volume;
                set_sound_volume(&layer.sound, volume);
            }
        }
        // Faded out: stop, and forget it unless it's wanted again, so a
        // long trip doesn't keep every tune in memory.
        let want = self.want;
        let ready = &mut self.ready;
        self.layers.retain(|l| {
            let gone = l.gain <= 0.0 && l.target <= 0.0;
            if gone {
                stop_sound(&l.sound);
                if !want.contains(&Some(l.what)) {
                    ready.remove(&l.what);
                }
            }
            !gone
        });
    }
}

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
}

/// Makes and loads every sound. Call once before the first frame.
pub async fn init() {
    let mut sounds = Vec::new();
    for sfx in Sfx::all() {
        if let Ok(sound) = load_sound_from_bytes(&sfx.make().to_wav()).await {
            sounds.push((sfx, sound, f64::MIN));
        }
    }
    let subscriber = macroquad::input::utils::register_input_subscriber();
    let (done_tx, done_rx) = channel();
    ENGINE.with(|e| {
        *e.borrow_mut() = Some(Engine {
            sounds,
            minimized: false,
            subscriber,
            want: [None, None],
            ready: HashMap::new(),
            composing: HashSet::new(),
            done_tx,
            done_rx,
            layers: Vec::new(),
            started: 0,
        })
    });
}

/// Which music should play: `None` for silence. Call every frame; it fades
/// over when it changes.
pub fn set_music(track: Option<Track>) {
    ENGINE.with(|e| {
        if let Some(e) = e.borrow_mut().as_mut() {
            e.want[0] = track.map(Loop::Music);
        }
    });
}

/// Which ambience should play, like `set_music`.
pub fn set_ambience(amb: Option<Ambience>) {
    ENGINE.with(|e| {
        if let Some(e) = e.borrow_mut().as_mut() {
            e.want[1] = amb.map(Loop::Ambience);
        }
    });
}

/// Notices the window being minimized or restored, and keeps music and
/// ambience going. Call once a frame.
pub fn update() {
    struct Watch(bool);
    impl miniquad::EventHandler for Watch {
        fn update(&mut self) {}
        fn draw(&mut self) {}
        fn window_minimized_event(&mut self) {
            self.0 = true;
        }
        fn window_restored_event(&mut self) {
            self.0 = false;
        }
    }
    ENGINE.with(|e| {
        if let Some(e) = e.borrow_mut().as_mut() {
            let mut watch = Watch(e.minimized);
            macroquad::input::utils::repeat_all_miniquad_input(&mut watch, e.subscriber);
            e.minimized = watch.0;
            e.started = 0;
            e.update_loops(get_frame_time().min(0.1));
        }
    });
}

/// Plays an interface sound, or any sound at full volume.
pub fn play(sfx: Sfx) {
    play_scaled(sfx, 1.0);
}

/// Plays a sound that happens `at` a place in the world, heard from
/// `listener`: quieter further away, silent past `HEARING`.
pub fn play_at(sfx: Sfx, at: Vec3, listener: Vec3) {
    let falloff = falloff(at.distance(listener));
    if falloff > 0.0 {
        play_scaled(sfx, falloff);
    }
}

/// How loud a sound is at `distance` yards, 0 to 1.
pub fn falloff(distance: f32) -> f32 {
    if distance <= FULL_VOLUME {
        1.0
    } else {
        (1.0 - (distance - FULL_VOLUME) / (HEARING - FULL_VOLUME)).clamp(0.0, 1.0)
    }
}

fn play_scaled(sfx: Sfx, scale: f32) {
    let s = settings::current();
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Some(e) = e.as_mut() else {
            return;
        };
        if s.mute_all || (s.mute_in_background && e.minimized) {
            return;
        }
        let volume = s.volume(Bus::Master) * s.volume(sfx.bus()) * scale;
        if volume <= 0.001 {
            return;
        }
        if e.started >= MAX_PER_FRAME {
            return;
        }
        let now = get_time();
        if let Some((_, sound, last)) = e.sounds.iter_mut().find(|(s, ..)| *s == sfx)
            && now - *last >= REPEAT_GAP as f64
        {
            *last = now;
            e.started += 1;
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume,
                },
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_audible_and_short() {
        for sfx in Sfx::all() {
            let w = sfx.make();
            let peak = w.0.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak > 0.1 && peak <= 1.0, "{sfx:?} peak {peak}");
            assert!(
                w.secs() > 0.01 && w.secs() < 3.0,
                "{sfx:?} is {}s",
                w.secs()
            );
            assert!(w.0.iter().all(|s| s.is_finite()), "{sfx:?} has bad samples");
        }
    }

    #[test]
    fn quieter_with_distance() {
        assert_eq!(falloff(0.0), 1.0);
        assert_eq!(falloff(FULL_VOLUME), 1.0);
        assert!(falloff(20.0) < 1.0 && falloff(20.0) > 0.0);
        assert_eq!(falloff(HEARING), 0.0);
        assert_eq!(falloff(100.0), 0.0);
    }
}

/// Writes every sound to `sfx/` in the temp folder, to listen to them:
/// `cargo test -p client dump_wavs -- --ignored`.
#[cfg(test)]
mod dump {
    #[test]
    #[ignore]
    fn dump_wavs() {
        use super::{Ambience, Track, music};
        let dir = std::env::temp_dir().join("sfx");
        let dir = dir.as_path();
        std::fs::create_dir_all(dir).unwrap();
        for sfx in super::Sfx::all() {
            let name = format!("{sfx:?}")
                .replace(['(', ')'], "-")
                .trim_end_matches('-')
                .to_string();
            std::fs::write(dir.join(format!("{name}.wav")), sfx.make().to_wav()).unwrap();
        }
        let mut tracks = vec![Track::Menu, Track::Boss];
        tracks.extend(shared::world::Zone::ALL.map(Track::Zone));
        tracks.extend(shared::dungeon::DungeonId::ALL.map(Track::Dungeon));
        for t in tracks {
            let name = format!("music-{t:?}").replace(['(', ')'], "");
            std::fs::write(dir.join(format!("{name}.wav")), music::compose(t).to_wav()).unwrap();
        }
        let mut ambs: Vec<Ambience> = shared::world::Zone::ALL.map(Ambience::Zone).to_vec();
        ambs.extend(shared::dungeon::DungeonId::ALL.map(Ambience::Dungeon));
        for a in ambs {
            let name = format!("ambience-{a:?}").replace(['(', ')'], "");
            std::fs::write(dir.join(format!("{name}.wav")), music::ambience(a).to_wav()).unwrap();
        }
    }
}
