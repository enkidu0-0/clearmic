use clearmic_protocol::{FLAG_RAW16, FRAME_SAMPLES};
use opus_decoder::OpusDecoder;

/// Turns packet payloads into 480-sample frames, and conceals lost packets.
pub struct Decoder {
    last: [f32; FRAME_SAMPLES],
    losses: u32,
    opus: Option<OpusDecoder>,
    last_was_opus: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self { last: [0.0; FRAME_SAMPLES], losses: 0, opus: OpusDecoder::new(48_000, 1).ok(), last_was_opus: false }
    }

    /// `data` is `[flags, payload...]` as stored in the jitter buffer.
    /// Returns false when the packet could not be decoded (output is concealed).
    pub fn decode(&mut self, data: &[u8], out: &mut [f32; FRAME_SAMPLES]) -> bool {
        let (flags, payload) = match data.split_first() {
            Some((f, p)) => (*f, p),
            None => return self.conceal(out),
        };
        if flags & FLAG_RAW16 != 0 {
            if payload.len() != FRAME_SAMPLES * 2 {
                return self.conceal(out);
            }
            for (o, b) in out.iter_mut().zip(payload.chunks_exact(2)) {
                *o = i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0;
            }
            self.last = *out;
            self.losses = 0;
            self.last_was_opus = false;
            return true;
        }
        if let Some(dec) = self.opus.as_mut() {
            if matches!(dec.decode_float(payload, out, false), Ok(n) if n == FRAME_SAMPLES) {
                self.last = *out;
                self.losses = 0;
                self.last_was_opus = true;
                return true;
            }
        }
        self.conceal(out)
    }

    /// Repeat the last frame, fading out, so a lost packet is a short fade not a click.
    pub fn conceal(&mut self, out: &mut [f32; FRAME_SAMPLES]) -> bool {
        // Opus has its own concealment, which sounds better than repeating the last frame.
        if self.last_was_opus && self.losses < 5 {
            if let Some(dec) = self.opus.as_mut() {
                if matches!(dec.decode_float(&[], out, false), Ok(n) if n == FRAME_SAMPLES) {
                    self.last = *out;
                    self.losses += 1;
                    return false;
                }
            }
        }
        self.losses += 1;
        let g = 0.5f32.powi(self.losses as i32);
        for (o, l) in out.iter_mut().zip(self.last.iter()) {
            *o = l * g;
        }
        false
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}
