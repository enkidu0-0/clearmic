/// A neural noise remover working on 480-sample (10 ms) frames at 48 kHz.
/// Not `Send`: tract models hold `Rc`s, so a denoiser is created on the thread that uses it.
pub trait Denoiser {
    fn process(&mut self, frame: &mut [f32]);
    /// 0.0 = leave the noise, 1.0 = remove as much as the model can.
    fn set_strength(&mut self, _strength: f32) {}
    /// Extra delay the model adds, for the latency readout.
    fn latency_ms(&self) -> f32 {
        0.0
    }
    /// Forget the audio heard so far (the phone went quiet and came back).
    fn reset(&mut self) {}
}

pub struct Passthrough;

impl Denoiser for Passthrough {
    fn process(&mut self, _frame: &mut [f32]) {}
}

#[cfg(feature = "dfn3")]
pub use dfn3::DeepFilter;

#[cfg(feature = "dfn3")]
mod dfn3 {
    use super::Denoiser;
    use deep_filter::tract::{DfParams, DfTract, RuntimeParams};
    use ndarray::Array2;

    /// DeepFilterNet3 through tract (pure Rust, no ONNX runtime needed).
    pub struct DeepFilter {
        model: DfTract,
        inp: Array2<f32>,
        out: Array2<f32>,
    }

    impl DeepFilter {
        pub fn new() -> Result<Self, String> {
            let rp = RuntimeParams::default_with_ch(1);
            let model = DfTract::new(DfParams::default(), &rp).map_err(|e| e.to_string())?;
            let hop = model.hop_size;
            Ok(Self { model, inp: Array2::zeros((1, hop)), out: Array2::zeros((1, hop)) })
        }
    }

    impl Denoiser for DeepFilter {
        fn process(&mut self, frame: &mut [f32]) {
            debug_assert_eq!(frame.len(), self.inp.ncols());
            for (d, s) in self.inp.row_mut(0).iter_mut().zip(frame.iter()) {
                *d = *s;
            }
            if self.model.process(self.inp.view(), self.out.view_mut()).is_ok() {
                for (d, s) in frame.iter_mut().zip(self.out.row(0).iter()) {
                    *d = *s;
                }
            }
        }

        fn set_strength(&mut self, strength: f32) {
            // 0 -> 6 dB of reduction, 1 -> 100 dB (effectively unlimited)
            self.model.set_atten_lim(6.0 + 94.0 * strength.clamp(0.0, 1.0));
        }

        fn latency_ms(&self) -> f32 {
            40.0
        }
    }
}

#[cfg(feature = "dpdfnet")]
pub use dpdfnet::DpdfNet;

#[cfg(feature = "dpdfnet")]
mod dpdfnet {
    use super::Denoiser;
    use crate::resample::{Decimator, Interpolator};
    use realfft::num_complex::Complex;
    use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
    use std::collections::VecDeque;
    use std::sync::Arc;
    use tract_onnx::prelude::*;

    const FRAME: usize = 480; // what the pipeline hands us: 10 ms at 48 kHz
    /// The model's output trails its input by this many hops (see dpdfnet `apply_attn_limit`).
    const MODEL_DELAY_HOPS: usize = 4;

    type Plan = TypedSimplePlan<TypedModel>;

    /// The sample-rate conversion around a 16 kHz model.
    struct Rate {
        down: Decimator,
        up: Interpolator,
        lo_in: Vec<f32>,
        lo_out: Vec<f32>,
        hi_out: Vec<f32>,
    }

    /// DPDFNet in streaming form: one window every hop, the recurrent state carried in one vector.
    /// 48 kHz models run directly; 16 kHz ones (about a sixth of the work) get resampled around them.
    pub struct DpdfNet {
        plan: Plan,
        init_state: Vec<f32>,
        state: Vec<f32>,
        win: usize,
        hop: usize,
        bins: usize,
        hop_ms: f32,
        window: Vec<f32>,
        fft: Arc<dyn RealToComplex<f32>>,
        ifft: Arc<dyn ComplexToReal<f32>>,
        in_buf: Vec<f32>,
        out_buf: Vec<f32>,
        noisy_hist: VecDeque<Vec<Complex<f32>>>,
        alpha: f32, // 0 = full removal, 1 = nothing removed
        rate: Option<Rate>,
    }

    fn meta<'a>(proto: &'a tract_onnx::pb::ModelProto, key: &str) -> Result<&'a str, String> {
        proto
            .metadata_props
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.value.as_str())
            .ok_or_else(|| format!("model metadata is missing '{key}'"))
    }

    fn floats(s: &str) -> Result<Vec<f32>, String> {
        s.split(',').map(|v| v.trim().parse::<f32>().map_err(|e| e.to_string())).collect()
    }

    impl DpdfNet {
        /// Loads a DPDFNet streaming ONNX file (`dpdfnet2_48khz_hr`, `dpdfnet2`, `baseline`...).
        pub fn from_file(path: &str) -> Result<Self, String> {
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            Self::from_bytes(&bytes)
        }

        pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
            let onnx = tract_onnx::onnx();
            let proto = onnx.proto_model_for_read(&mut std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;

            let win: usize = meta(&proto, "window_length")?.parse().map_err(|_| "bad window_length")?;
            let sr: usize = meta(&proto, "sample_rate")?.parse().map_err(|_| "bad sample_rate")?;
            let hop = win / 2;
            let bins = win / 2 + 1;
            let rate = match sr {
                48_000 if hop == FRAME => None,
                16_000 if hop * 3 == FRAME => Some(Rate {
                    down: Decimator::new(),
                    up: Interpolator::new(),
                    lo_in: Vec::with_capacity(hop),
                    lo_out: Vec::with_capacity(hop),
                    hi_out: Vec::with_capacity(FRAME),
                }),
                _ => return Err(format!("unsupported model: {sr} Hz with a {win}-sample window")),
            };
            let state_size: usize = meta(&proto, "state_size")?.parse().map_err(|_| "bad state_size")?;
            let erb_n: usize = meta(&proto, "erb_norm_state_size")?.parse().map_err(|_| "bad erb size")?;
            let spec_n: usize = meta(&proto, "spec_norm_state_size")?.parse().map_err(|_| "bad spec size")?;
            let erb_init = floats(meta(&proto, "erb_norm_init")?)?;
            let spec_init = floats(meta(&proto, "spec_norm_init")?)?;
            if erb_init.len() != erb_n || spec_init.len() != spec_n || erb_n + spec_n > state_size {
                return Err("model metadata sizes do not agree".into());
            }
            let mut init_state = vec![0f32; state_size];
            init_state[..erb_n].copy_from_slice(&erb_init);
            init_state[erb_n..erb_n + spec_n].copy_from_slice(&spec_init);

            let plan = onnx
                .model_for_proto_model(&proto)
                .and_then(|m| m.into_optimized())
                .and_then(|m| m.into_runnable())
                .map_err(|e| e.to_string())?;

            // Vorbis window
            let window: Vec<f32> = (0..win)
                .map(|i| {
                    let s = (0.5 * std::f32::consts::PI * (i as f32 + 0.5) / (win as f32 / 2.0)).sin();
                    (0.5 * std::f32::consts::PI * s * s).sin()
                })
                .collect();

            let mut planner = RealFftPlanner::<f32>::new();
            Ok(Self {
                plan,
                state: init_state.clone(),
                init_state,
                win,
                hop,
                bins,
                hop_ms: hop as f32 * 1000.0 / sr as f32,
                window,
                fft: planner.plan_fft_forward(win),
                ifft: planner.plan_fft_inverse(win),
                in_buf: vec![0.0; win],
                out_buf: vec![0.0; win],
                noisy_hist: VecDeque::new(),
                alpha: 0.0,
                rate,
            })
        }

        pub fn reset(&mut self) {
            self.state.copy_from_slice(&self.init_state);
            self.in_buf.fill(0.0);
            self.out_buf.fill(0.0);
            self.noisy_hist.clear();
        }

        /// One hop through the network, at the model's own rate. `io` is read, then overwritten.
        fn run_hop(&mut self, io: &mut [f32]) {
            let (win, hop, bins) = (self.win, self.hop, self.bins);
            self.in_buf.copy_within(hop.., 0);
            self.in_buf[win - hop..].copy_from_slice(io);

            // analysis
            let mut windowed: Vec<f32> = self.in_buf.iter().zip(&self.window).map(|(a, b)| a * b).collect();
            let mut spec = vec![Complex::new(0.0, 0.0); bins];
            if self.fft.process(&mut windowed, &mut spec).is_err() {
                return;
            }
            let mut ri = Vec::with_capacity(bins * 2);
            for c in &spec {
                ri.push(c.re);
                ri.push(c.im);
            }

            // network
            let Ok(spec_t) = Tensor::from_shape(&[1, 1, bins, 2], &ri) else { return };
            let Ok(state_t) = Tensor::from_shape(&[self.state.len()], &self.state) else { return };
            let Ok(out) = self.plan.run(tvec!(spec_t.into_tvalue(), state_t.into_tvalue())) else { return };
            let (Ok(enh), Ok(new_state)) = (out[0].as_slice::<f32>(), out[1].as_slice::<f32>()) else { return };
            self.state.copy_from_slice(new_state);
            let mut enh_c: Vec<Complex<f32>> = (0..bins).map(|k| Complex::new(enh[2 * k], enh[2 * k + 1])).collect();

            // strength: blend the noisy spectrum (delayed to match the model) back in
            self.noisy_hist.push_back(spec);
            if self.noisy_hist.len() > MODEL_DELAY_HOPS + 1 {
                self.noisy_hist.pop_front();
            }
            if self.alpha > 0.0 && self.noisy_hist.len() == MODEL_DELAY_HOPS + 1 {
                let noisy = &self.noisy_hist[0];
                for (e, n) in enh_c.iter_mut().zip(noisy) {
                    *e = *e * (1.0 - self.alpha) + *n * self.alpha;
                }
            }

            // synthesis and overlap-add
            let mut time = vec![0f32; win];
            enh_c[0].im = 0.0; // realfft wants real DC and Nyquist bins
            enh_c[bins - 1].im = 0.0;
            if self.ifft.process(&mut enh_c, &mut time).is_err() {
                return;
            }
            let norm = 1.0 / win as f32;
            for (o, (t, w)) in self.out_buf.iter_mut().zip(time.iter().zip(&self.window)) {
                *o += t * norm * w;
            }
            io.copy_from_slice(&self.out_buf[..hop]);
            self.out_buf.copy_within(hop.., 0);
            self.out_buf[win - hop..].fill(0.0);
        }
    }

    impl Denoiser for DpdfNet {
        fn process(&mut self, frame: &mut [f32]) {
            debug_assert_eq!(frame.len(), FRAME);
            let Some(mut r) = self.rate.take() else {
                return self.run_hop(frame);
            };
            r.lo_in.clear();
            r.down.process(frame, &mut r.lo_in);
            self.run_hop(&mut r.lo_in);
            r.hi_out.clear();
            r.up.process(&r.lo_in, &mut r.hi_out);
            frame.copy_from_slice(&r.hi_out);
            self.rate = Some(r);
        }

        fn set_strength(&mut self, strength: f32) {
            // 1.0 = full removal. Below that, mix back up to 94% of the original (about -6 dB at 0).
            let s = strength.clamp(0.0, 1.0);
            self.alpha = if s >= 0.999 { 0.0 } else { 10f32.powf(-(6.0 + 94.0 * s) / 20.0).max(0.0) };
        }

        fn latency_ms(&self) -> f32 {
            (MODEL_DELAY_HOPS + 2) as f32 * self.hop_ms
        }

        fn reset(&mut self) {
            DpdfNet::reset(self);
        }
    }
}
