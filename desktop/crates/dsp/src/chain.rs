use crate::biquad::Biquad;
use crate::denoise::Denoiser;
use crate::dynamics::{AutoGain, Compressor, DeEsser, Gate, Limiter};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChainConfig {
    pub strength: f32,
    pub hpf: bool,
    pub gate: bool,
    pub eq: bool,
    pub comp: bool,
    pub deess: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Natural,
    Discord,
    Podcast,
}

impl ChainConfig {
    pub fn preset(p: Preset) -> Self {
        match p {
            Preset::Natural => Self { strength: 0.6, hpf: true, gate: false, eq: false, comp: false, deess: false },
            Preset::Discord => Self { strength: 0.8, hpf: true, gate: true, eq: true, comp: true, deess: false },
            Preset::Podcast => Self { strength: 0.9, hpf: true, gate: true, eq: true, comp: true, deess: true },
        }
    }
}

pub struct Chain {
    cfg: ChainConfig,
    bypass: bool,
    denoiser: Box<dyn Denoiser>,
    hpf: Biquad,
    leveler: AutoGain,
    gate: Gate,
    eq_mud: Biquad,
    eq_air: Biquad,
    comp: Compressor,
    deess: DeEsser,
    limiter: Limiter,
}

impl Chain {
    pub fn new(mut denoiser: Box<dyn Denoiser>, cfg: ChainConfig) -> Self {
        denoiser.set_strength(cfg.strength);
        Self {
            cfg,
            bypass: false,
            denoiser,
            hpf: Biquad::highpass(80.0, 0.707),
            leveler: AutoGain::new(),
            gate: Gate::new(-50.0, -58.0, -25.0),
            eq_mud: Biquad::peaking(250.0, 1.0, -2.0),
            eq_air: Biquad::high_shelf(4000.0, 3.0),
            comp: Compressor::new(-18.0, 3.0, 5.0, 80.0, 4.0),
            deess: DeEsser::new(-30.0),
            limiter: Limiter::new(-1.0),
        }
    }

    pub fn config(&self) -> ChainConfig {
        self.cfg
    }

    pub fn set_config(&mut self, cfg: ChainConfig) {
        if (cfg.strength - self.cfg.strength).abs() > f32::EPSILON {
            self.denoiser.set_strength(cfg.strength);
        }
        self.cfg = cfg;
    }

    pub fn set_preset(&mut self, p: Preset) {
        self.set_config(ChainConfig::preset(p));
    }

    /// Hold-to-hear-original: skips everything except the limiter.
    pub fn set_bypass(&mut self, on: bool) {
        self.bypass = on;
    }

    /// Swaps the noise remover while running (for example to a lighter one when the PC is busy).
    pub fn set_denoiser(&mut self, mut d: Box<dyn Denoiser>) {
        d.set_strength(self.cfg.strength);
        self.denoiser = d;
    }

    pub fn reset(&mut self) {
        self.denoiser.reset();
    }

    pub fn latency_ms(&self) -> f32 {
        self.denoiser.latency_ms()
    }

    pub fn process(&mut self, frame: &mut [f32]) {
        if self.bypass {
            // Same volume as the processed voice, so the comparison is about noise and tone, not loudness.
            self.leveler.apply(frame);
        } else {
            if self.cfg.hpf {
                self.hpf.run(frame);
            }
            self.leveler.apply(frame);
            self.denoiser.process(frame);
            self.leveler.update(frame);
            if self.cfg.gate {
                self.gate.run(frame);
            }
            if self.cfg.eq {
                self.eq_mud.run(frame);
                self.eq_air.run(frame);
            }
            if self.cfg.comp {
                self.comp.run(frame);
            }
            if self.cfg.deess {
                self.deess.run(frame);
            }
        }
        self.limiter.run(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::denoise::Passthrough;
    use crate::SAMPLE_RATE;
    use std::f32::consts::PI;

    fn sine(freq: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (2.0 * PI * freq * i as f32 / SAMPLE_RATE).sin()).collect()
    }
    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn leveler_lifts_quiet_voice_and_holds_in_silence() {
        let mut chain = Chain::new(Box::new(Passthrough), ChainConfig::preset(Preset::Natural));
        let quiet = sine(300.0, 0.004, 480); // about -53 dBFS rms, like an unprocessed phone mic
        let mut out = Vec::new();
        for _ in 0..400 {
            let mut f = quiet.clone();
            chain.process(&mut f);
            out = f;
        }
        let lvl = 20.0 * rms(&out).log10();
        assert!(lvl > -30.0 && lvl < -16.0, "quiet voice should reach about -23 dBFS, got {lvl}");
        for _ in 0..5 {
            chain.process(&mut vec![0.00001f32; 480]); // let the filter tail from the voice die out
        }
        let gain = chain.leveler.gain_db();
        for _ in 0..200 {
            let mut f = vec![0.00001f32; 480]; // silence
            chain.process(&mut f);
        }
        assert!((chain.leveler.gain_db() - gain).abs() < 0.01, "gain must hold in silence");
    }

    #[test]
    fn leveler_brings_a_loud_voice_down() {
        let mut chain = Chain::new(Box::new(Passthrough), ChainConfig::preset(Preset::Natural));
        let loud = sine(300.0, 0.7, 480);
        let mut out = Vec::new();
        for _ in 0..400 {
            let mut f = loud.clone();
            chain.process(&mut f);
            out = f;
        }
        assert!(chain.leveler.gain_db() < -3.0 && rms(&out) < 0.7 * 0.707);
    }

    #[test]
    fn highpass_cuts_rumble_keeps_voice() {
        let mut lo = sine(40.0, 0.5, 48_000);
        let mut mid = sine(1000.0, 0.5, 48_000);
        Biquad::highpass(80.0, 0.707).run(&mut lo);
        Biquad::highpass(80.0, 0.707).run(&mut mid);
        assert!(rms(&lo[4800..]) < 0.5 * 0.707 * 0.4, "40 Hz not cut: {}", rms(&lo[4800..]));
        assert!((rms(&mid[4800..]) - 0.5 * 0.707).abs() < 0.02);
    }

    #[test]
    fn limiter_never_exceeds_ceiling() {
        let mut l = Limiter::new(-1.0);
        let mut x = sine(300.0, 4.0, 4800);
        l.run(&mut x);
        let ceil = crate::db_to_lin(-1.0);
        assert!(x.iter().all(|v| v.abs() <= ceil + 1e-6));
    }

    #[test]
    fn compressor_reduces_loud_signal() {
        let mut c = Compressor::new(-18.0, 3.0, 5.0, 80.0, 0.0);
        let mut x = sine(500.0, 0.8, 24_000);
        c.run(&mut x);
        assert!(rms(&x[12_000..]) < 0.8 * 0.707 * 0.7);
    }

    #[test]
    fn gate_attenuates_quiet_noise_passes_loud() {
        let mut g = Gate::new(-50.0, -58.0, -25.0);
        let mut quiet = sine(2000.0, 0.0005, 24_000); // about -63 dBFS
        g.run(&mut quiet);
        assert!(rms(&quiet[20_000..]) < 0.0005 * 0.707 * 0.2);
        let mut g = Gate::new(-50.0, -58.0, -25.0);
        let mut loud = sine(200.0, 0.3, 24_000);
        g.run(&mut loud);
        assert!((rms(&loud[12_000..]) - 0.3 * 0.707).abs() < 0.02);
    }

    #[test]
    fn full_chain_is_finite_for_all_presets_and_bypass() {
        for p in [Preset::Natural, Preset::Discord, Preset::Podcast] {
            let mut ch = Chain::new(Box::new(Passthrough), ChainConfig::preset(p));
            for round in 0..2 {
                ch.set_bypass(round == 1);
                for _ in 0..100 {
                    let mut f = sine(220.0, 0.6, 480);
                    ch.process(&mut f);
                    assert!(f.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
                }
            }
        }
    }
}
