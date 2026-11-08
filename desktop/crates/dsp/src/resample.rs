//! 3:1 polyphase resamplers between 48 kHz and 16 kHz (windowed-sinc low-pass, 48 taps).
//! Used so the light 16 kHz noise models can sit inside the 48 kHz pipeline.

use std::f32::consts::PI;

const TAPS: usize = 48; // multiple of 3
const RATIO: usize = 3;

fn kernel() -> Vec<f32> {
    let fc = 0.5 / RATIO as f32 * 0.93; // a little under 8 kHz, in cycles per 48 kHz sample
    let m = (TAPS - 1) as f32 / 2.0;
    let mut h: Vec<f32> = (0..TAPS)
        .map(|i| {
            let x = i as f32 - m;
            let sinc = if x == 0.0 { 2.0 * fc } else { (2.0 * PI * fc * x).sin() / (PI * x) };
            let w = 0.42 - 0.5 * (2.0 * PI * i as f32 / (TAPS - 1) as f32).cos() + 0.08 * (4.0 * PI * i as f32 / (TAPS - 1) as f32).cos();
            sinc * w
        })
        .collect();
    let sum: f32 = h.iter().sum();
    h.iter_mut().for_each(|v| *v /= sum); // unity gain at DC
    h
}

/// 48 kHz -> 16 kHz. Feed a multiple of 3 samples per call.
pub struct Decimator {
    h: Vec<f32>,
    hist: Vec<f32>, // the last TAPS-1 inputs
}

impl Decimator {
    pub fn new() -> Self {
        Self { h: kernel(), hist: vec![0.0; TAPS - 1] }
    }

    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        debug_assert_eq!(input.len() % RATIO, 0);
        let mut buf = Vec::with_capacity(self.hist.len() + input.len());
        buf.extend_from_slice(&self.hist);
        buf.extend_from_slice(input);
        let mut n = 0;
        while n + TAPS <= buf.len() {
            let acc: f32 = self.h.iter().zip(&buf[n..n + TAPS]).map(|(a, b)| a * b).sum();
            out.push(acc);
            n += RATIO;
        }
        self.hist.copy_from_slice(&buf[buf.len() - (TAPS - 1)..]);
    }
}

/// 16 kHz -> 48 kHz.
pub struct Interpolator {
    h: Vec<f32>,
    hist: Vec<f32>, // the last TAPS/3 - 1 inputs
}

impl Interpolator {
    pub fn new() -> Self {
        Self { h: kernel(), hist: vec![0.0; TAPS / RATIO - 1] }
    }

    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let per = TAPS / RATIO;
        let mut buf = Vec::with_capacity(self.hist.len() + input.len());
        buf.extend_from_slice(&self.hist);
        buf.extend_from_slice(input);
        for n in per - 1..buf.len() {
            for phase in 0..RATIO {
                // output sample = sum over j of h[3j + phase] * x[n - j], scaled by the ratio
                let acc: f32 = (0..per).map(|j| self.h[RATIO * j + phase] * buf[n - j]).sum();
                out.push(acc * RATIO as f32);
            }
        }
        self.hist.copy_from_slice(&buf[buf.len() - (per - 1)..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(f: f32, n: usize, sr: f32) -> Vec<f32> {
        (0..n).map(|i| (2.0 * PI * f * i as f32 / sr).sin()).collect()
    }
    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn round_trip_keeps_speech_band_and_drops_above_8k() {
        for (f, keep) in [(500.0, true), (3000.0, true), (11_000.0, false)] {
            let x = tone(f, 48_000, 48_000.0);
            let (mut d, mut u) = (Decimator::new(), Interpolator::new());
            let (mut lo, mut y) = (Vec::new(), Vec::new());
            for c in x.chunks(480) {
                d.process(c, &mut lo);
            }
            assert_eq!(lo.len(), 16_000, "decimated length");
            for c in lo.chunks(160) {
                u.process(c, &mut y);
            }
            assert_eq!(y.len(), 48_000, "interpolated length");
            let g = rms(&y[9_600..]) / rms(&x[9_600..]);
            if keep {
                assert!((g - 1.0).abs() < 0.08, "{f} Hz gain {g}");
            } else {
                assert!(g < 0.05, "{f} Hz should be removed, gain {g}");
            }
        }
    }
}
