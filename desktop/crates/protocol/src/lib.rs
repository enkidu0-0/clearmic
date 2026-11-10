//! Sonivane wire format (see ARCHITECTURE.md section 4).
//!
//! Datagram = 18-byte header + ChaCha20-Poly1305 ciphertext (payload + 16-byte tag).
//! The header is authenticated as associated data; the nonce is `seq | ts_us`, so it
//! never repeats for a given key as long as the phone's sequence number does not wrap.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

pub const MAGIC: [u8; 2] = [0xC1, 0xEA];
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 18;
pub const TAG_LEN: usize = 16;
/// Samples per packet: 10 ms at 48 kHz.
pub const FRAME_SAMPLES: usize = 480;
pub const SAMPLE_RATE: u32 = 48_000;

pub const FLAG_MUTED: u8 = 1;
pub const FLAG_PHONE_DENOISE: u8 = 2;
pub const FLAG_FEC: u8 = 4;
/// Payload is raw little-endian i16 PCM instead of Opus. For tests and the simulator.
pub const FLAG_RAW16: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    TooShort,
    BadMagic,
    BadVersion,
    BadLength,
    Auth,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub flags: u8,
    pub seq: u32,
    /// Phone capture time in microseconds (phone clock).
    pub ts_us: u64,
    /// Length of the encrypted payload, tag included.
    pub len: u16,
}

impl Header {
    pub fn write(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        b[0..2].copy_from_slice(&MAGIC);
        b[2] = VERSION;
        b[3] = self.flags;
        b[4..8].copy_from_slice(&self.seq.to_le_bytes());
        b[8..16].copy_from_slice(&self.ts_us.to_le_bytes());
        b[16..18].copy_from_slice(&self.len.to_le_bytes());
        b
    }

    pub fn read(buf: &[u8]) -> Result<Header, Error> {
        if buf.len() < HEADER_LEN {
            return Err(Error::TooShort);
        }
        if buf[0..2] != MAGIC {
            return Err(Error::BadMagic);
        }
        if buf[2] != VERSION {
            return Err(Error::BadVersion);
        }
        let len = u16::from_le_bytes([buf[16], buf[17]]);
        if buf.len() != HEADER_LEN + len as usize || (len as usize) < TAG_LEN {
            return Err(Error::BadLength);
        }
        Ok(Header {
            flags: buf[3],
            seq: u32::from_le_bytes(buf[4..8].try_into().unwrap()),
            ts_us: u64::from_le_bytes(buf[8..16].try_into().unwrap()),
            len,
        })
    }
}

pub struct Cipher(ChaCha20Poly1305);

impl Cipher {
    pub fn new(key: &[u8; 32]) -> Self {
        Cipher(ChaCha20Poly1305::new(Key::from_slice(key)))
    }

    fn nonce(seq: u32, ts_us: u64) -> [u8; 12] {
        let mut n = [0u8; 12];
        n[0..4].copy_from_slice(&seq.to_le_bytes());
        n[4..12].copy_from_slice(&ts_us.to_le_bytes());
        n
    }

    /// Build one datagram.
    pub fn seal(&self, flags: u8, seq: u32, ts_us: u64, payload: &[u8]) -> Vec<u8> {
        let header = Header { flags, seq, ts_us, len: (payload.len() + TAG_LEN) as u16 };
        let hb = header.write();
        let ct = self
            .0
            .encrypt(Nonce::from_slice(&Self::nonce(seq, ts_us)), Payload { msg: payload, aad: &hb })
            .expect("encrypt");
        let mut out = Vec::with_capacity(HEADER_LEN + ct.len());
        out.extend_from_slice(&hb);
        out.extend_from_slice(&ct);
        out
    }

    /// Check and decrypt one datagram.
    pub fn open(&self, datagram: &[u8]) -> Result<(Header, Vec<u8>), Error> {
        let h = Header::read(datagram)?;
        let pt = self
            .0
            .decrypt(
                Nonce::from_slice(&Self::nonce(h.seq, h.ts_us)),
                Payload { msg: &datagram[HEADER_LEN..], aad: &datagram[..HEADER_LEN] },
            )
            .map_err(|_| Error::Auth)?;
        Ok((h, pt))
    }
}

/// Parse a 64-character hex key (what the pairing QR carries, hex for now).
pub fn parse_key_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let c = Cipher::new(&[7u8; 32]);
        let d = c.seal(FLAG_RAW16, 42, 123_456, b"hello opus");
        let (h, p) = c.open(&d).unwrap();
        assert_eq!((h.seq, h.ts_us, h.flags), (42, 123_456, FLAG_RAW16));
        assert_eq!(p, b"hello opus");
    }

    #[test]
    fn tamper_header_or_body_fails() {
        let c = Cipher::new(&[7u8; 32]);
        let d = c.seal(0, 1, 2, b"abc");
        let mut a = d.clone();
        a[3] ^= 1; // flags are authenticated
        assert_eq!(c.open(&a).unwrap_err(), Error::Auth);
        let mut b = d.clone();
        *b.last_mut().unwrap() ^= 1;
        assert_eq!(c.open(&b).unwrap_err(), Error::Auth);
    }

    #[test]
    fn wrong_key_and_garbage() {
        let d = Cipher::new(&[1u8; 32]).seal(0, 1, 2, b"abc");
        assert_eq!(Cipher::new(&[2u8; 32]).open(&d).unwrap_err(), Error::Auth);
        let c = Cipher::new(&[1u8; 32]);
        assert_eq!(c.open(&[0u8; 5]).unwrap_err(), Error::TooShort);
        assert_eq!(c.open(&[0u8; 40]).unwrap_err(), Error::BadMagic);
    }

    #[test]
    fn key_hex() {
        assert_eq!(parse_key_hex(&"ab".repeat(32)), Some([0xab; 32]));
        assert_eq!(parse_key_hex("zz"), None);
    }
}
