//! Fixed-size output history for one background job, addressed by absolute
//! byte offsets so a reader can tell how much it missed after overflow.

use std::collections::VecDeque;

#[derive(Debug)]
pub(super) struct OutputRing {
    buf: VecDeque<u8>,
    /// Absolute offset of `buf[0]`.
    start: u64,
    cap: usize,
}

impl OutputRing {
    pub(super) fn new(cap: usize) -> Self {
        Self {
            buf: VecDeque::new(),
            start: 0,
            cap,
        }
    }

    /// Absolute offset one past the newest byte.
    pub(super) fn end(&self) -> u64 {
        self.start + self.buf.len() as u64
    }

    pub(super) fn push(&mut self, bytes: &[u8]) {
        self.buf.extend(bytes);
        let excess = self.buf.len().saturating_sub(self.cap);
        if excess > 0 {
            self.buf.drain(..excess);
            self.start += excess as u64;
        }
    }

    /// Text from absolute offset `from` to the end, and how many bytes at
    /// or after `from` were already overwritten.
    pub(super) fn read_from(&self, from: u64) -> (String, u64) {
        let begin = from.clamp(self.start, self.end());
        let dropped = begin.saturating_sub(from);
        let skip = usize::try_from(begin - self.start).unwrap_or(usize::MAX);
        let bytes: Vec<u8> = self.buf.range(skip..).copied().collect();
        (decode(&bytes), dropped)
    }

    /// Up to the last `max` bytes as text.
    pub(super) fn tail(&self, max: usize) -> String {
        self.read_from(self.end().saturating_sub(max as u64)).0
    }
}

/// Lossy UTF-8 that skips a partial character left by the overflow cut.
fn decode(bytes: &[u8]) -> String {
    let skip = bytes
        .iter()
        .take(3)
        .take_while(|&&b| (b & 0xC0) == 0x80)
        .count();
    String::from_utf8_lossy(&bytes[skip..]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_advances_start_and_reports_dropped_bytes() {
        let mut ring = OutputRing::new(8);
        ring.push(b"0123456789");
        assert_eq!(ring.end(), 10);
        assert_eq!(ring.read_from(0), ("23456789".to_string(), 2));
        assert_eq!(ring.read_from(5), ("56789".to_string(), 0));
        assert_eq!(ring.read_from(10), (String::new(), 0));
        assert_eq!(ring.tail(3), "789");
    }

    #[test]
    fn partial_characters_at_the_cut_are_skipped() {
        let mut ring = OutputRing::new(3);
        ring.push("éé".as_bytes());
        assert_eq!(ring.read_from(0).0, "é");
    }
}
