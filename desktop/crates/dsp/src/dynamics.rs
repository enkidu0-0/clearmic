use crate::biquad::Biquad;
use crate::{db_to_lin, lin_to_db, time_coef};

/// Slow input leveler. A phone mic set to "unprocessed" can arrive 20 dB quieter than a headset, so the noise remover and
/// the gate would treat normal speech as noise. This lifts the voice toward a healthy level before them.
/// The gain is applied to the raw frame; the level is measured on the cleaned frame, so noise never counts as speech.
pub struct AutoGain {
    gain_db: f32,
    applied: f32,
    speech_db: f32,
}

impl AutoGain {
    const TARGET_DB: f32 = -23.0;
    /// Below this the cleaned frame is treated as silence and the gain holds.
    const SPEECH_FLOOR_DB: f32 = -62.0;
    const MIN_DB: f32 = -12.0;
    const MAX_DB: f32 = 30.0;

    pub fn new() -> Self {
        Self { gain_db: 0.0, applied: 1.0, speech_db: Self::TARGET_DB }
    }

    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    /// Scales the raw frame, ramping from the previous gain so there is no click.
    pub fn apply(&mut self, frame: &mut [f32]) {
        let target = db_to_lin(self.gain_db);
        let n = frame.len().max(1) as f32;
        for (i, s) in frame.iter_mut().enumerate() {
            let g = self.applied + (target - self.applied) * ((i + 1) as f32 / n);
            *s *= g;
        }
        self.applied = target;
    }

    /// Looks at the cleaned frame (after the noise remover) and nudges the gain.
    pub fn update(&mut self, cleaned: &[f32]) {
        let rms = (cleaned.iter().map(|v| v * v).sum::<f32>() / cleaned.len().max(1) as f32).sqrt();
        let db = lin_to_db(rms);
        if db < Self::SPEECH_FLOOR_DB {
            return;
        }
        self.speech_db += (db - self.speech_db) * 0.05;
        let err = Self::TARGET_DB - self.speech_db;
        // too loud: come down quickly; too quiet: rise slowly, about 40 dB a second at most
        let step = if err < 0.0 { err * 0.05 } else { (err * 0.02).min(0.4) };
        self.gain_db = (self.gain_db + step).clamp(Self::MIN_DB, Self::MAX_DB);
    }
}

/// Soft gate. Opens fast on speech, closes slowly, never fully mutes (keeps room tone natural).
pub struct Gate {
    open_db: f32,
    close_db: f32,
    floor: f32,
    env: f32,
    gain: f32,
    is_open: bool,
    env_up: f32,
    env_down: f32,
    g_up: f32,
    g_down: f32,
}

impl Gate {
    pub fn new(open_db: f32, close_db: f32, floor_db: f32) -> Self {
        Self {
            open_db, close_db, floor: db_to_lin(floor_db), env: 0.0, gain: 1.0, is_open: true,
            env_up: time_coef(2.0), env_down: time_coef(60.0),
            g_up: time_coef(3.0), g_down: time_coef(120.0),
        }
    }

    pub fn run(&mut self, buf: &mut [f32]) {
        for s in buf {
            let a = s.abs();
            let c = if a > self.env { self.env_up } else { self.env_down };
            self.env = c * self.env + (1.0 - c) * a;
            let db = lin_to_db(self.env);
            if self.is_open && db < self.close_db {
                self.is_open = false;
            } else if !self.is_open && db > self.open_db {
                self.is_open = true;
            }
            let target = if self.is_open { 1.0 } else { self.floor };
            let c = if target > self.gain { self.g_up } else { self.g_down };
            self.gain = c * self.gain + (1.0 - c) * target;
            *s *= self.gain;
        }
    }
}

/// Feed-forward compressor with a hard knee and fixed make-up gain.
pub struct Compressor {
    threshold_db: f32,
    ratio: f32,
    makeup: f32,
    env: f32,
    attack: f32,
    release: f32,
}

impl Compressor {
    pub fn new(threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32, makeup_db: f32) -> Self {
        Self {
            threshold_db, ratio, makeup: db_to_lin(makeup_db), env: 0.0,
            attack: time_coef(attack_ms), release: time_coef(release_ms),
        }
    }

    pub fn run(&mut self, buf: &mut [f32]) {
        for s in buf {
            let a = s.abs();
            let c = if a > self.env { self.attack } else { self.release };
            self.env = c * self.env + (1.0 - c) * a;
            let over = lin_to_db(self.env) - self.threshold_db;
            let gr_db = if over > 0.0 { -over * (1.0 - 1.0 / self.ratio) } else { 0.0 };
            *s *= db_to_lin(gr_db) * self.makeup;
        }
    }
}

/// Split-band de-esser: the 5 kHz and up band is turned down when it gets loud.
pub struct DeEsser {
    band: Biquad,
    high: Biquad,
    threshold: f32,
    env: f32,
    attack: f32,
    release: f32,
}

impl DeEsser {
    pub fn new(threshold_db: f32) -> Self {
        Self {
            band: Biquad::bandpass(6500.0, 1.0),
            high: Biquad::highpass(5000.0, 0.707),
            threshold: db_to_lin(threshold_db),
            env: 0.0,
            attack: time_coef(1.0),
            release: time_coef(40.0),
        }
    }

    pub fn run(&mut self, buf: &mut [f32]) {
        for s in buf {
            let x = *s;
            let det = self.band.tick(x).abs();
            let c = if det > self.env { self.attack } else { self.release };
            self.env = c * self.env + (1.0 - c) * det;
            let gain = if self.env > self.threshold { self.threshold / self.env } else { 1.0 };
            let hp = self.high.tick(x);
            *s = x - hp + hp * gain;
        }
    }
}

/// Peak limiter: instant attack, smooth release. Output never exceeds the ceiling.
pub struct Limiter {
    ceiling: f32,
    gain: f32,
    release: f32,
}

impl Limiter {
    pub fn new(ceiling_db: f32) -> Self {
        Self { ceiling: db_to_lin(ceiling_db), gain: 1.0, release: time_coef(60.0) }
    }

    pub fn run(&mut self, buf: &mut [f32]) {
        for s in buf {
            let a = s.abs() * self.gain;
            if a > self.ceiling {
                self.gain *= self.ceiling / a;
            } else {
                self.gain = self.release * self.gain + (1.0 - self.release) * 1.0;
            }
            *s = (*s * self.gain).clamp(-self.ceiling, self.ceiling);
        }
    }
}
