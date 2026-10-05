//! Music and ambience, composed by the game itself.
//!
//! Each track is a loop built from a style: a key and scale, a tempo, a
//! chord progression and which instruments play. A seeded random melody
//! runs over the chords, so every track is different but the same each
//! time. Ambience loops are layered wind, birds, water drips, lava and so
//! on. Both are built on a background thread the first time they're
//! needed (see `audio.rs`).

use shared::dungeon::DungeonId;
use shared::world::Zone;

use crate::synth::{Rng, Shape, Wave, bell, noise, note, pluck, tone, wind};

/// A piece of music.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Track {
    /// Login and character screens.
    Menu,
    Zone(Zone),
    Dungeon(DungeonId),
    /// Fighting a dungeon boss.
    Boss,
}

/// A background sound loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ambience {
    Zone(Zone),
    Dungeon(DungeonId),
}

const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const MINOR: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
const DORIAN: [i32; 7] = [0, 2, 3, 5, 7, 9, 10];
const PHRYGIAN: [i32; 7] = [0, 1, 3, 5, 7, 8, 10];
const LYDIAN: [i32; 7] = [0, 2, 4, 6, 7, 9, 11];
const HARMONIC: [i32; 7] = [0, 2, 3, 5, 7, 8, 11];
const MIXOLYDIAN: [i32; 7] = [0, 2, 4, 5, 7, 9, 10];

#[derive(Clone, Copy, PartialEq)]
enum Lead {
    Bell,
    Flute,
    Pluck,
    Reed,
}

#[derive(Clone, Copy, PartialEq)]
enum Drums {
    None,
    /// A hand drum.
    Soft,
    /// Big low drums.
    War,
}

struct Style {
    /// The key's home note, in semitones from A4.
    root: i32,
    scale: [i32; 7],
    bpm: f32,
    /// Scale degrees (0 is the home chord), two bars each.
    chords: [usize; 4],
    pad: Shape,
    /// How bright the pad is, in Hz.
    pad_tone: f32,
    lead: Lead,
    /// Notes rippling up the chord.
    arp: Option<Lead>,
    drums: Drums,
    /// How busy the melody is, 0 to 1.
    busy: f32,
    seed: u32,
}

fn style(track: Track) -> Style {
    let s = |root, scale, bpm, chords, pad, pad_tone, lead, arp, drums, busy, seed| Style {
        root,
        scale,
        bpm,
        chords,
        pad,
        pad_tone,
        lead,
        arp,
        drums,
        busy,
        seed,
    };
    use Lead::*;
    use Shape::*;
    match track {
        Track::Menu => s(
            -7,
            DORIAN,
            70.0,
            [0, 6, 5, 4],
            Triangle,
            900.0,
            Bell,
            Some(Pluck),
            Drums::None,
            0.4,
            1,
        ),
        Track::Zone(z) => match z {
            // Autumn hills at dusk: warm and homely.
            Zone::Amberfall => s(
                -4,
                MAJOR,
                76.0,
                [0, 4, 5, 3],
                Triangle,
                1200.0,
                Flute,
                Some(Pluck),
                Drums::None,
                0.55,
                2,
            ),
            // Desert: an exotic scale over a hand drum.
            Zone::Scorchsand => s(
                -7,
                PHRYGIAN,
                84.0,
                [0, 1, 0, 6],
                Saw,
                700.0,
                Reed,
                None,
                Drums::Soft,
                0.6,
                3,
            ),
            // Twilight elf forest: dreamy and bright.
            Zone::Silverbough => s(
                -2,
                LYDIAN,
                66.0,
                [0, 1, 4, 0],
                Triangle,
                1500.0,
                Bell,
                Some(Bell),
                Drums::None,
                0.35,
                4,
            ),
            // Goblin cave: bouncy and a little sly.
            Zone::Grubdeep => s(
                -9,
                MINOR,
                96.0,
                [0, 3, 4, 0],
                Square,
                600.0,
                Pluck,
                Some(Pluck),
                Drums::Soft,
                0.7,
                5,
            ),
            // Gnome peaks: clear and crisp.
            Zone::Frostcog => s(
                -5,
                MAJOR,
                72.0,
                [0, 3, 0, 4],
                Triangle,
                1800.0,
                Bell,
                Some(Bell),
                Drums::None,
                0.45,
                6,
            ),
            // Dying forest: slow and uneasy.
            Zone::Witherwood => s(
                -11,
                HARMONIC,
                60.0,
                [0, 3, 5, 4],
                Saw,
                500.0,
                Flute,
                None,
                Drums::None,
                0.3,
                7,
            ),
            // Open highlands: a broad, hopeful road song.
            Zone::Sunfold => s(
                -3,
                MIXOLYDIAN,
                80.0,
                [0, 6, 3, 4],
                Triangle,
                1400.0,
                Flute,
                Some(Pluck),
                Drums::Soft,
                0.5,
                40,
            ),
            // Badlands: grim and driving.
            Zone::Blightscar => s(
                -8,
                PHRYGIAN,
                88.0,
                [0, 1, 5, 4],
                Saw,
                650.0,
                Reed,
                Some(Pluck),
                Drums::Soft,
                0.55,
                41,
            ),
        },
        Track::Dungeon(d) => match d {
            DungeonId::SunkenVault => s(
                -10,
                DORIAN,
                64.0,
                [0, 6, 3, 0],
                Triangle,
                600.0,
                Bell,
                None,
                Drums::None,
                0.25,
                8,
            ),
            DungeonId::Cinderforge => s(
                -8,
                PHRYGIAN,
                96.0,
                [0, 1, 6, 0],
                Saw,
                650.0,
                Reed,
                None,
                Drums::War,
                0.5,
                9,
            ),
            DungeonId::Frosthowl => s(
                -6,
                HARMONIC,
                70.0,
                [0, 5, 6, 4],
                Triangle,
                1300.0,
                Bell,
                Some(Bell),
                Drums::None,
                0.35,
                10,
            ),
        },
        Track::Boss => s(
            -9,
            HARMONIC,
            124.0,
            [0, 5, 4, 4],
            Saw,
            900.0,
            Reed,
            Some(Pluck),
            Drums::War,
            0.75,
            11,
        ),
    }
}

/// The note `degree` steps up the scale from the key's home note (more
/// than 7 goes up an octave).
fn degree(st: &Style, d: i32) -> i32 {
    let (oct, i) = (d.div_euclid(7), d.rem_euclid(7));
    st.root + oct * 12 + st.scale[i as usize]
}

fn instrument(lead: Lead, f: f32, len: f32, seed: u32) -> Wave {
    match lead {
        Lead::Bell => bell(f, len.max(0.8) * 1.5).gain(0.7),
        Lead::Flute => tone(Shape::Triangle, f, f, len, 0.06, 0.8).mix(
            0.0,
            tone(Shape::Sine, f * 2.0, f * 2.0, len, 0.06, 1.0).gain(0.15),
        ),
        Lead::Pluck => pluck(f, len.max(0.5) + 0.3, 0.996, seed).gain(0.8),
        Lead::Reed => tone(Shape::Saw, f, f, len, 0.03, 0.7)
            .lowpass(f * 3.0)
            .gain(1.4),
    }
}

/// Builds a whole track, ready to loop.
pub fn compose(track: Track) -> Wave {
    let st = style(track);
    let beat = 60.0 / st.bpm;
    let bar = beat * 4.0;
    let bars = 16;
    let len = bar * bars as f32;
    let mut rng = Rng::new(st.seed * 7919);
    let mut pad = Wave::new();
    let mut bass = Wave::new();
    let mut arp = Wave::new();
    let mut lead = Wave::new();
    let mut drums = Wave::new();

    for b in 0..bars {
        let chord = st.chords[(b / 2) % 4] as i32;
        let t0 = b as f32 * bar;
        // Pad: the chord held, swelling in and out over two bars.
        if b % 2 == 0 {
            for (i, step) in [0, 2, 4].into_iter().enumerate() {
                let f = note(degree(&st, chord + step) - 12);
                let voice = tone(st.pad, f, f, bar * 2.0, bar * 0.6, 0.7)
                    .mix(
                        0.0,
                        tone(st.pad, f * 1.004, f * 1.004, bar * 2.0, bar * 0.6, 0.7),
                    )
                    .lowpass(st.pad_tone)
                    .gain(if i == 0 { 0.35 } else { 0.25 });
                pad = pad.mix(t0, voice);
            }
        }
        // Bass on the first and third beats.
        for beat_at in [0.0, 2.0] {
            let f = note(degree(&st, chord) - 24);
            bass = bass.mix(
                t0 + beat_at * beat,
                tone(Shape::Sine, f, f, beat * 1.8, 0.01, 1.2),
            );
        }
        // Arpeggio: up and down the chord in eighth notes.
        if let Some(kind) = st.arp
            && b >= 2
        {
            for (i, step) in [0, 2, 4, 7, 4, 2, 0, 2].into_iter().enumerate() {
                let f = note(degree(&st, chord + step));
                let n = instrument(kind, f, beat * 0.5, st.seed + (b * 8 + i) as u32);
                arp = arp.mix(t0 + i as f32 * beat * 0.5, n.gain(0.22));
            }
        }
        // Melody: rests in the first bars of each half, then a seeded walk
        // that lands on chord notes on the strong beats.
        if b % 8 >= 2 {
            let mut at: f32 = 0.0;
            let mut d = chord + 7;
            while at < 4.0 {
                let long = rng.next().abs() > st.busy;
                let dur = if long { 1.0 } else { 0.5 };
                if rng.next().abs() < 0.85 {
                    if at.fract() == 0.0 && at % 2.0 == 0.0 {
                        // Strong beat: a note of the chord.
                        let tones = [chord + 7, chord + 9, chord + 11];
                        d = tones[(rng.next().abs() * 2.99) as usize];
                    } else {
                        d += (rng.next() * 2.5).round() as i32;
                        d = d.clamp(chord + 4, chord + 13);
                    }
                    let f = note(degree(&st, d));
                    let n = instrument(st.lead, f, dur * beat * 0.95, st.seed + b as u32 * 31);
                    lead = lead.mix(t0 + at * beat, n.gain(0.45));
                }
                at += dur;
            }
        }
        // Drums.
        match st.drums {
            Drums::None => {}
            Drums::Soft => {
                for (at, f) in [
                    (0.0, 180.0),
                    (1.5, 240.0),
                    (2.0, 180.0),
                    (3.0, 240.0),
                    (3.5, 240.0),
                ] {
                    let hit = tone(Shape::Sine, f * 1.5, f, 0.18, 0.002, 2.5).mix(
                        0.0,
                        noise(0.03, 0.001, 2.0, b as u32).lowpass(3000.0).gain(0.3),
                    );
                    drums = drums.mix(t0 + at * beat, hit.gain(0.5));
                }
            }
            Drums::War => {
                for at in [0.0, 1.0, 2.0, 2.5, 3.0] {
                    let big = at == 0.0 || at == 2.0;
                    let hit = tone(
                        Shape::Sine,
                        110.0,
                        42.0,
                        if big { 0.5 } else { 0.3 },
                        0.002,
                        1.8,
                    )
                    .mix(
                        0.0,
                        noise(0.06, 0.001, 2.0, b as u32 * 5)
                            .lowpass(1200.0)
                            .gain(0.5),
                    );
                    drums = drums.mix(t0 + at * beat, hit.gain(if big { 0.9 } else { 0.55 }));
                }
                if b % 4 == 3 {
                    // A roll into the next phrase.
                    for i in 0..4 {
                        let hit = tone(Shape::Sine, 160.0, 90.0, 0.2, 0.002, 2.0);
                        drums = drums.mix(t0 + (3.0 + i as f32 * 0.25) * beat, hit.gain(0.45));
                    }
                }
            }
        }
    }

    pad.lowpass(4000.0)
        .mix(0.0, bass.gain(0.5))
        .mix(0.0, arp.echo(beat * 0.75, 0.3))
        .mix(0.0, lead.echo(beat * 0.75, 0.3))
        .mix(0.0, drums)
        .fold_loop(len)
        .normalize(0.6)
}

/// Chirps from a few birds, scattered over `len` seconds.
fn birds(len: f32, count: usize, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let mut w = Wave::new();
    for _ in 0..count {
        let at = rng.next().abs() * len;
        let base = 2600.0 + rng.next().abs() * 2200.0;
        let notes = 2 + (rng.next().abs() * 4.0) as usize;
        for i in 0..notes {
            let (f0, f1) = if i % 2 == 0 {
                (base, base * 1.3)
            } else {
                (base * 1.2, base * 0.9)
            };
            let chirp = tone(Shape::Sine, f0, f1, 0.07, 0.005, 1.5).gain(0.25);
            w = w.mix(at + i as f32 * 0.09, chirp);
        }
    }
    w
}

/// Water drips in a cave, with a little echo.
fn drips(len: f32, count: usize, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let mut w = Wave::new();
    for _ in 0..count {
        let at = rng.next().abs() * len;
        let f = 1100.0 + rng.next().abs() * 900.0;
        w = w.mix(
            at,
            tone(Shape::Sine, f * 1.6, f, 0.06, 0.001, 2.0).gain(0.3),
        );
    }
    w.echo(0.23, 0.35)
}

/// Lapping water: soft noise that swells like small waves.
fn waves(len: f32, seed: u32) -> Wave {
    let n = wind(len, 500.0, 0.6, seed);
    let period = 5.5;
    Wave(
        n.0.iter()
            .enumerate()
            .map(|(i, s)| {
                let t = i as f32 / crate::synth::RATE as f32;
                s * (0.35 + 0.65 * ((t / period * std::f32::consts::TAU).sin() * 0.5 + 0.5))
            })
            .collect(),
    )
}

/// Pops and crackles, like embers.
fn crackles(len: f32, count: usize, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let mut w = Wave::new();
    for i in 0..count {
        let at = rng.next().abs() * len;
        let pop = noise(0.012, 0.0005, 3.0, seed + i as u32).highpass(1500.0);
        w = w.mix(at, pop.gain(0.2 + rng.next().abs() * 0.4));
    }
    w
}

/// Branches creaking, and now and then a crow.
fn creaks(len: f32, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let mut w = Wave::new();
    for _ in 0..5 {
        let at = rng.next().abs() * len;
        let f = 70.0 + rng.next().abs() * 40.0;
        let creak = tone(Shape::Saw, f, f * 1.4, 0.7, 0.2, 1.0)
            .lowpass(900.0)
            .gain(0.12);
        w = w.mix(at, creak);
    }
    for _ in 0..2 {
        let at = rng.next().abs() * len;
        for i in 0..2 {
            let caw = tone(Shape::Saw, 620.0, 430.0, 0.22, 0.01, 1.2)
                .lowpass(2200.0)
                .gain(0.12);
            w = w.mix(at + i as f32 * 0.32, caw);
        }
    }
    w
}

/// Crickets chirping steadily, for dusk.
fn crickets(len: f32, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let mut w = Wave::new();
    let mut at = 0.0;
    while at < len {
        for i in 0..3 {
            w = w.mix(
                at + i as f32 * 0.035,
                tone(Shape::Sine, 4400.0, 4400.0, 0.025, 0.003, 1.0).gain(0.06),
            );
        }
        at += 0.5 + rng.next().abs() * 0.4;
    }
    w
}

/// Builds an ambience loop.
pub fn ambience(amb: Ambience) -> Wave {
    const LOOP: f32 = 30.0;
    const XFADE: f32 = 3.0;
    // Steady layers run past the loop's end to blend into its start;
    // one-off sounds (birds, drips) stay within it.
    let len = LOOP + XFADE;
    let ev = LOOP - 1.0;
    let w = match amb {
        Ambience::Zone(z) => match z {
            Zone::Amberfall => wind(len, 350.0, 0.5, 21)
                .gain(0.6)
                .mix(0.0, birds(ev, 5, 22))
                .mix(0.0, crickets(ev, 23)),
            Zone::Scorchsand => wind(len, 700.0, 0.8, 24),
            Zone::Silverbough => wind(len, 300.0, 0.4, 25)
                .gain(0.5)
                .mix(0.0, birds(ev, 12, 26)),
            Zone::Grubdeep => tone(Shape::Sine, 55.0, 55.0, len, 0.01, 0.0)
                .gain(0.08)
                .mix(0.0, wind(len, 150.0, 0.3, 27).gain(0.5))
                .mix(0.0, drips(ev, 14, 28)),
            Zone::Frostcog => {
                wind(len, 900.0, 0.9, 29).mix(0.0, wind(len, 1600.0, 0.7, 30).gain(0.3))
            }
            Zone::Witherwood => wind(len, 250.0, 0.6, 31).gain(0.7).mix(0.0, creaks(ev, 32)),
            Zone::Sunfold => wind(len, 500.0, 0.6, 140)
                .gain(0.6)
                .mix(0.0, birds(ev, 8, 141)),
            Zone::Blightscar => wind(len, 600.0, 0.8, 142)
                .gain(0.8)
                .mix(0.0, creaks(ev, 143).gain(0.5)),
        },
        Ambience::Dungeon(d) => match d {
            DungeonId::SunkenVault => waves(len, 33).mix(0.0, drips(ev, 18, 34)),
            DungeonId::Cinderforge => wind(len, 120.0, 0.5, 35)
                .mix(0.0, crackles(ev, 90, 36))
                .mix(0.0, tone(Shape::Sine, 41.0, 41.0, len, 0.01, 0.0).gain(0.2)),
            DungeonId::Frosthowl => wind(len, 600.0, 0.8, 37).mix(0.0, drips(ev, 8, 38)),
        },
    };
    w.crossfade_loop(LOOP, XFADE).normalize(0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_track() -> Vec<Track> {
        let mut all = vec![Track::Menu, Track::Boss];
        all.extend(Zone::ALL.map(Track::Zone));
        all.extend(DungeonId::ALL.map(Track::Dungeon));
        all
    }

    #[test]
    fn tracks_are_loops_of_sensible_length() {
        for t in every_track() {
            let w = compose(t);
            assert!(w.0.iter().all(|s| s.is_finite()), "{t:?}");
            let secs = w.0.len() as f32 / crate::synth::RATE as f32;
            assert!((25.0..=70.0).contains(&secs), "{t:?} is {secs}s");
            let peak = w.0.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!((peak - 0.6).abs() < 1e-3, "{t:?} peak {peak}");
        }
    }

    #[test]
    fn ambience_loops() {
        let mut all: Vec<Ambience> = Zone::ALL.map(Ambience::Zone).to_vec();
        all.extend(DungeonId::ALL.map(Ambience::Dungeon));
        for a in all {
            let w = ambience(a);
            assert!(w.0.iter().all(|s| s.is_finite()), "{a:?}");
            assert_eq!(w.0.len(), crate::synth::samples(30.0), "{a:?}");
        }
    }
}
