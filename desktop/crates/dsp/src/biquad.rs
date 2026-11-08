//! RBJ cookbook biquads, transposed direct form II.

use crate::SAMPLE_RATE;
use std::f32::consts::PI;

#[derive(Clone)]
pub struct Biquad {
    b0: f32, b1: f32, b2: f32, a1: f32, a2: f32,
    z1: f32, z2: f32,
}

impl Biquad {
    fn from(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, z1: 0.0, z2: 0.0 }
    }

    pub fn highpass(freq: f32, q: f32) -> Self {
        let w = 2.0 * PI * freq / SAMPLE_RATE;
        let (s, c) = w.sin_cos();
        let al = s / (2.0 * q);
        Self::from((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0, 1.0 + al, -2.0 * c, 1.0 - al)
    }

    pub fn bandpass(freq: f32, q: f32) -> Self {
        let w = 2.0 * PI * freq / SAMPLE_RATE;
        let (s, c) = w.sin_cos();
        let al = s / (2.0 * q);
        Self::from(al, 0.0, -al, 1.0 + al, -2.0 * c, 1.0 - al)
    }

    pub fn peaking(freq: f32, q: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * freq / SAMPLE_RATE;
        let (s, c) = w.sin_cos();
        let al = s / (2.0 * q);
        Self::from(1.0 + al * a, -2.0 * c, 1.0 - al * a, 1.0 + al / a, -2.0 * c, 1.0 - al / a)
    }

    pub fn high_shelf(freq: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * freq / SAMPLE_RATE;
        let (s, c) = w.sin_cos();
        let al = s / 2.0 * std::f32::consts::SQRT_2; // shelf slope 1
        let sq = 2.0 * a.sqrt() * al;
        Self::from(
            a * ((a + 1.0) + (a - 1.0) * c + sq),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
            a * ((a + 1.0) + (a - 1.0) * c - sq),
            (a + 1.0) - (a - 1.0) * c + sq,
            2.0 * ((a - 1.0) - (a + 1.0) * c),
            (a + 1.0) - (a - 1.0) * c - sq,
        )
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn run(&mut self, buf: &mut [f32]) {
        for s in buf {
            *s = self.tick(*s);
        }
    }
}
