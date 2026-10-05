//! Sound effects made from scratch in code: tones, noise and plucked
//! strings shaped by envelopes and filters. Nothing is downloaded or copied,
//! so there's no license to track, and the exe only grows by this code.
//! Each sound comes out as a WAV file in memory for the audio engine.

use std::f32::consts::TAU;

pub const RATE: u32 = 44_100;

/// A sound being built: mono samples at `RATE`.
#[derive(Clone, Default)]
pub struct Wave(pub Vec<f32>);

/// Tone shapes.
#[derive(Clone, Copy)]
pub enum Shape {
    Sine,
    Triangle,
    Square,
    Saw,
}

/// A small deterministic random source, so every run sounds the same.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    /// -1 to 1.
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

pub fn samples(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

/// How loud a sound is at `t` of `len` seconds: a quick rise over `attack`,
/// then a fall shaped by `curve` (1 straight, higher drops away faster).
fn envelope(t: f32, len: f32, attack: f32, curve: f32) -> f32 {
    if t < attack {
        t / attack
    } else {
        let left = 1.0 - (t - attack) / (len - attack).max(1e-4);
        left.max(0.0).powf(curve)
    }
}

fn osc(shape: Shape, phase: f32) -> f32 {
    let p = phase.fract();
    match shape {
        Shape::Sine => (p * TAU).sin(),
        Shape::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
        Shape::Square => {
            if p < 0.5 {
                0.6
            } else {
                -0.6
            }
        }
        Shape::Saw => (2.0 * p - 1.0) * 0.7,
    }
}

impl Wave {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn secs(&self) -> f32 {
        self.0.len() as f32 / RATE as f32
    }

    /// Adds `other` starting `at` seconds in, growing to fit.
    pub fn mix(mut self, at: f32, other: Wave) -> Self {
        let start = samples(at);
        if self.0.len() < start + other.0.len() {
            self.0.resize(start + other.0.len(), 0.0);
        }
        for (i, s) in other.0.into_iter().enumerate() {
            self.0[start + i] += s;
        }
        self
    }

    /// Wobbles the loudness `rate` times a second, `depth` 0 to 1: growls
    /// and rattles.
    pub fn tremolo(mut self, rate: f32, depth: f32) -> Self {
        for (i, s) in self.0.iter_mut().enumerate() {
            let t = i as f32 / RATE as f32;
            *s *= 1.0 - depth * (0.5 + 0.5 * (t * rate * TAU).sin());
        }
        self
    }

    pub fn gain(mut self, g: f32) -> Self {
        self.0.iter_mut().for_each(|s| *s *= g);
        self
    }

    /// A one-pole low-pass: softens everything above about `cutoff` Hz.
    pub fn lowpass(mut self, cutoff: f32) -> Self {
        let a = 1.0 - (-TAU * cutoff / RATE as f32).exp();
        let mut y = 0.0;
        for s in &mut self.0 {
            y += a * (*s - y);
            *s = y;
        }
        self
    }

    /// The high part only: whatever a low-pass at `cutoff` would remove.
    pub fn highpass(self, cutoff: f32) -> Self {
        let low = self.clone().lowpass(cutoff);
        Wave(self.0.iter().zip(low.0).map(|(s, l)| s - l).collect())
    }

    /// A short echo, for a sense of space.
    pub fn echo(mut self, delay: f32, feedback: f32) -> Self {
        let d = samples(delay);
        let len = self.0.len();
        self.0.resize(len + d * 3, 0.0);
        for i in d..self.0.len() {
            self.0[i] += self.0[i - d] * feedback;
        }
        self
    }

    /// Makes the sound loop smoothly every `len` seconds: whatever rings on
    /// past the end (echoes, long notes) is laid over the start, which is
    /// what you hear when it repeats.
    pub fn fold_loop(self, len: f32) -> Self {
        let n = samples(len);
        let mut out = vec![0.0; n];
        for (i, s) in self.0.into_iter().enumerate() {
            out[i % n] += s;
        }
        Wave(out)
    }

    /// Makes a steady sound (wind, water) loop every `len` seconds without a
    /// jump: the extra `xfade` seconds past the end are blended into the
    /// start. Needs at least `len + xfade` seconds of sound.
    pub fn crossfade_loop(self, len: f32, xfade: f32) -> Self {
        let (n, x) = (samples(len), samples(xfade));
        let mut w = self.0;
        w.resize(w.len().max(n + x), 0.0);
        for i in 0..x {
            let t = i as f32 / x as f32;
            // Equal power, so the blend doesn't dip in loudness.
            w[i] = w[i] * t.sqrt() + w[n + i] * (1.0 - t).sqrt();
        }
        w.truncate(n);
        Wave(w)
    }

    /// Fades the last `secs` out, so a sound cut short doesn't click.
    pub fn fade_tail(mut self, secs: f32) -> Self {
        let n = samples(secs).min(self.0.len());
        let len = self.0.len();
        for i in 0..n {
            self.0[len - n + i] *= 1.0 - i as f32 / n as f32;
        }
        self
    }

    /// Scales the loudest sample to `peak`.
    pub fn normalize(self, peak: f32) -> Self {
        let max = self.0.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        if max > 0.0 {
            self.gain(peak / max)
        } else {
            self
        }
    }

    /// 16-bit mono WAV file bytes.
    pub fn to_wav(&self) -> Vec<u8> {
        let data_len = self.0.len() as u32 * 2;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in &self.0 {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }
}

/// A tone gliding from `f0` to `f1` Hz over `len` seconds.
pub fn tone(shape: Shape, f0: f32, f1: f32, len: f32, attack: f32, curve: f32) -> Wave {
    let n = samples(len);
    // Exponential glide sounds even to the ear: the same step every sample.
    let step = (f1 / f0).powf(1.0 / n.max(1) as f32);
    let mut f = f0;
    let mut phase = 0.0f32;
    Wave(
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                phase = (phase + f / RATE as f32).fract();
                f *= step;
                osc(shape, phase) * envelope(t, len, attack, curve)
            })
            .collect(),
    )
}

/// Wind: noise whose brightness and loudness drift slowly up and down.
/// `base` is the usual brightness in Hz; `gust` how much it swings.
pub fn wind(len: f32, base: f32, gust: f32, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let n = samples(len);
    let (mut y, mut y2) = (0.0f32, 0.0f32);
    // Two slow waves at odd rates, so the gusts don't repeat evenly.
    let (r1, r2) = (
        0.07 + rng.next().abs() * 0.05,
        0.17 + rng.next().abs() * 0.08,
    );
    Wave(
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let swell = 0.6 + 0.25 * (t * r1 * TAU).sin() + 0.15 * (t * r2 * TAU + 1.3).sin();
                let c = base * (1.0 + gust * (swell - 0.5) * 2.0).max(0.1);
                let a = 1.0 - (-TAU * c / RATE as f32).exp();
                y += a * (rng.next() - y);
                y2 += a * (y - y2);
                y2 * swell
            })
            .collect(),
    )
}

/// A bell or metal ping: a few inharmonic partials that ring and fade, the
/// higher ones faster.
pub fn bell(f: f32, len: f32) -> Wave {
    [(1.0, 1.0), (2.76, 0.5), (5.4, 0.25), (8.93, 0.12)]
        .into_iter()
        .fold(Wave::new(), |w, (ratio, amp)| {
            w.mix(
                0.0,
                tone(
                    Shape::Sine,
                    f * ratio,
                    f * ratio,
                    len / ratio.sqrt(),
                    0.002,
                    2.5,
                )
                .gain(amp),
            )
        })
}

/// Noise shaped by an envelope; filter it to taste.
pub fn noise(len: f32, attack: f32, curve: f32, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let n = samples(len);
    Wave(
        (0..n)
            .map(|i| rng.next() * envelope(i as f32 / RATE as f32, len, attack, curve))
            .collect(),
    )
}

/// Noise whose brightness sweeps from `c0` to `c1` Hz: a whoosh.
pub fn sweep(len: f32, c0: f32, c1: f32, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let n = samples(len);
    let mut y = 0.0;
    let mut y2 = 0.0;
    Wave(
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let c = c0 * (c1 / c0).powf(t / len);
                let a = 1.0 - (-TAU * c / RATE as f32).exp();
                y += a * (rng.next() - y);
                y2 += a * (y - y2);
                // Rises and falls smoothly, loudest in the middle.
                y2 * (t / len * std::f32::consts::PI).sin()
            })
            .collect(),
    )
}

/// A plucked string (Karplus-Strong): a burst of noise looping through a
/// short delay that softens a little each pass.
pub fn pluck(f: f32, len: f32, damping: f32, seed: u32) -> Wave {
    let mut rng = Rng::new(seed);
    let period = (RATE as f32 / f) as usize;
    let mut buf: Vec<f32> = (0..period).map(|_| rng.next()).collect();
    let n = samples(len);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let j = i % period;
        let next = buf[(j + 1) % period];
        let s = buf[j];
        buf[j] = (s + next) * 0.5 * damping;
        out.push(s);
    }
    Wave(out)
}

/// Notes in Hz by semitones from A4.
pub fn note(semitones: i32) -> f32 {
    440.0 * 2f32.powf(semitones as f32 / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_matches_length() {
        let w = tone(Shape::Sine, 440.0, 440.0, 0.1, 0.01, 1.0);
        let bytes = w.to_wav();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(bytes.len(), 44 + w.0.len() * 2);
        let data_len = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        assert_eq!(data_len as usize, w.0.len() * 2);
    }

    #[test]
    fn mix_grows_to_fit() {
        let a = tone(Shape::Sine, 440.0, 440.0, 0.1, 0.01, 1.0);
        let b = tone(Shape::Sine, 660.0, 660.0, 0.1, 0.01, 1.0);
        let m = a.mix(0.2, b);
        assert!((m.secs() - 0.3).abs() < 0.01);
    }

    #[test]
    fn normalize_hits_the_peak() {
        let w = noise(0.2, 0.01, 1.0, 7).normalize(0.5);
        let max = w.0.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((max - 0.5).abs() < 1e-4);
    }
}
