//! Adaptive jitter buffer. Packets go in with `push`, and `pop` is called once per
//! playout tick (every 10 ms). It reorders, reports lost packets, and grows its
//! cushion when packets arrive late.

use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub enum Slot {
    /// The next packet in order.
    Packet(Vec<u8>),
    /// The next packet never arrived; later ones did. Conceal it.
    Lost,
    /// Nothing buffered (start-up, or ran dry). Output silence or conceal.
    Waiting,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    pub received: u64,
    pub lost: u64,
    pub late: u64,
    pub underruns: u64,
    pub skipped: u64,
}

pub struct JitterBuffer {
    q: BTreeMap<u32, Vec<u8>>,
    next: u32,
    started: bool,
    target: usize,
    min_target: usize,
    max_target: usize,
    pub stats: Stats,
}

/// A sequence jump this large (in packets, 5 s of audio) means a new stream, not reordering.
const RESET_DISTANCE: u32 = 500;

/// Signed distance a - b, correct across u32 wraparound.
fn diff(a: u32, b: u32) -> i32 {
    a.wrapping_sub(b) as i32
}

impl JitterBuffer {
    pub fn new(min_target: usize, max_target: usize) -> Self {
        Self {
            q: BTreeMap::new(),
            next: 0,
            started: false,
            target: min_target,
            min_target,
            max_target,
            stats: Stats::default(),
        }
    }

    /// Current cushion in packets.
    pub fn target(&self) -> usize {
        self.target
    }

    pub fn push(&mut self, seq: u32, data: Vec<u8>) {
        self.stats.received += 1;
        if self.started && diff(seq, self.next).unsigned_abs() > RESET_DISTANCE {
            // The sender restarted with a new sequence range. Drop the old stream.
            self.q.clear();
            self.started = false;
        }
        if self.started && diff(seq, self.next) < 0 {
            // Arrived after its turn: too late to play. Grow the cushion.
            self.stats.late += 1;
            self.target = (self.target + 1).min(self.max_target);
            return;
        }
        self.q.insert(seq, data);
    }

    fn first_key(&self) -> Option<u32> {
        // BTreeMap orders by u32, which breaks across wraparound; find the oldest by distance.
        let anchor = if self.started { self.next } else { self.q.keys().next().copied()? };
        self.q.keys().copied().min_by_key(|k| diff(*k, anchor))
    }

    pub fn pop(&mut self) -> Slot {
        if !self.started {
            if self.q.len() < self.target {
                return Slot::Waiting;
            }
            self.next = self.first_key().unwrap();
            self.started = true;
        }
        let n = self.next;
        if let Some(p) = self.q.remove(&n) {
            self.next = n.wrapping_add(1);
            // Reclaim latency if the queue grew past the cushion.
            while self.q.len() > self.target + 2 {
                let k = self.first_key().unwrap();
                self.q.remove(&k);
                self.stats.skipped += 1;
                self.next = k.wrapping_add(1);
            }
            return Slot::Packet(p);
        }
        if self.q.keys().any(|k| diff(*k, n) > 0) {
            self.next = n.wrapping_add(1);
            self.stats.lost += 1;
            return Slot::Lost;
        }
        // Ran dry: refill to the cushion before playing again.
        self.started = false;
        self.stats.underruns += 1;
        Slot::Waiting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(n: u8) -> Vec<u8> {
        vec![n]
    }

    #[test]
    fn in_order_after_cushion() {
        let mut j = JitterBuffer::new(3, 8);
        j.push(10, p(0));
        j.push(11, p(1));
        assert_eq!(j.pop(), Slot::Waiting);
        j.push(12, p(2));
        assert_eq!(j.pop(), Slot::Packet(p(0)));
        assert_eq!(j.pop(), Slot::Packet(p(1)));
    }

    #[test]
    fn reorders() {
        let mut j = JitterBuffer::new(3, 8);
        for s in [1u32, 3, 2, 4] {
            j.push(s, p(s as u8));
        }
        let got: Vec<_> = (0..4).map(|_| j.pop()).collect();
        assert_eq!(got, vec![Slot::Packet(p(1)), Slot::Packet(p(2)), Slot::Packet(p(3)), Slot::Packet(p(4))]);
    }

    #[test]
    fn reports_loss() {
        let mut j = JitterBuffer::new(3, 8);
        for s in [1u32, 2, 4, 5] {
            j.push(s, p(s as u8));
        }
        assert_eq!(j.pop(), Slot::Packet(p(1)));
        assert_eq!(j.pop(), Slot::Packet(p(2)));
        assert_eq!(j.pop(), Slot::Lost);
        assert_eq!(j.pop(), Slot::Packet(p(4)));
        assert_eq!(j.stats.lost, 1);
    }

    #[test]
    fn late_packet_grows_cushion() {
        let mut j = JitterBuffer::new(2, 8);
        for s in [1u32, 2, 4] {
            j.push(s, p(s as u8));
        }
        j.pop();
        j.pop();
        assert_eq!(j.pop(), Slot::Lost); // 3 missing
        j.push(3, p(3)); // arrives after its turn
        assert_eq!(j.stats.late, 1);
        assert_eq!(j.target(), 3);
    }

    #[test]
    fn underrun_rebuffers() {
        let mut j = JitterBuffer::new(2, 8);
        j.push(1, p(1));
        j.push(2, p(2));
        j.pop();
        j.pop();
        assert_eq!(j.pop(), Slot::Waiting);
        assert_eq!(j.stats.underruns, 1);
        j.push(3, p(3));
        assert_eq!(j.pop(), Slot::Waiting); // below cushion again
    }

    #[test]
    fn sequence_wraparound() {
        let mut j = JitterBuffer::new(2, 8);
        for s in [u32::MAX - 1, u32::MAX, 0, 1] {
            j.push(s, p(s as u8));
        }
        let got: Vec<_> = (0..4).map(|_| j.pop()).collect();
        assert!(got.iter().all(|s| matches!(s, Slot::Packet(_))), "{got:?}");
    }

    #[test]
    fn restarted_sender_is_a_new_stream() {
        let mut j = JitterBuffer::new(2, 8);
        for s in 0..5u32 {
            j.push(s, p(s as u8));
        }
        j.pop();
        j.pop();
        for s in [1_000_000u32, 1_000_001, 1_000_002] {
            j.push(s, p(0));
        }
        assert_eq!(j.stats.late, 0);
        assert_eq!(j.pop(), Slot::Packet(p(0)));
    }

    #[test]
    fn trims_excess_latency() {
        let mut j = JitterBuffer::new(2, 8);
        for s in 0..10u32 {
            j.push(s, p(s as u8));
        }
        j.pop();
        assert!(j.stats.skipped > 0);
    }
}
