//! Voice processing chain: high-pass, denoise, gate, EQ, compressor, de-esser, limiter.
//! All processing is per 10 ms frame, mono f32, 48 kHz, with no allocation in `process`.

pub mod biquad;
pub mod chain;
pub mod denoise;
pub mod dynamics;
pub mod resample;

pub use chain::{Chain, ChainConfig, Preset};
pub use denoise::{Denoiser, Passthrough};

pub const SAMPLE_RATE: f32 = 48_000.0;

#[inline]
pub fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[inline]
pub fn lin_to_db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

/// One-pole smoothing coefficient for a time constant in milliseconds.
#[inline]
pub fn time_coef(ms: f32) -> f32 {
    (-1.0 / (ms * 0.001 * SAMPLE_RATE)).exp()
}
