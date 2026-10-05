//! The sound engine: makes every sound effect once at startup (see
//! `synth.rs`) and plays them at the volumes picked in Settings.
//!
//! Interface sounds play at full volume. Sounds in the world get quieter
//! with distance from you and aren't played at all past `HEARING`.

use std::cell::RefCell;

use macroquad::audio::{PlaySoundParams, Sound, load_sound_from_bytes, play_sound};
use macroquad::prelude::*;
use shared::data::School;

use crate::settings::{self, Bus};
use crate::synth::{Shape, Wave, bell, noise, note, pluck, sweep, tone};

/// How far away (in yards) a sound in the world can be heard. Placeholder.
pub const HEARING: f32 = 40.0;
/// Closer than this, world sounds play at full volume. Placeholder.
const FULL_VOLUME: f32 = 8.0;
/// The same sound won't start again within this many seconds, so a
/// big fight doesn't stack dozens of copies.
const REPEAT_GAP: f32 = 0.05;

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
        }
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

struct Engine {
    /// Every sound, and when it last started by the game clock.
    sounds: Vec<(Sfx, Sound, f64)>,
    minimized: bool,
    subscriber: usize,
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
    ENGINE.with(|e| {
        *e.borrow_mut() = Some(Engine {
            sounds,
            minimized: false,
            subscriber,
        })
    });
}

/// Notices the window being minimized or restored. Call once a frame.
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
        let now = get_time();
        if let Some((_, sound, last)) = e.sounds.iter_mut().find(|(s, ..)| *s == sfx)
            && now - *last >= REPEAT_GAP as f64
        {
            *last = now;
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
    }
}
