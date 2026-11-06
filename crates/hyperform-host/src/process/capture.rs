//! Bounded capture of command output that keeps both ends: the first half
//! of the budget holds the head, the second half a rolling tail, so a noisy
//! command still shows how it started and how it finished.

use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub(crate) struct Capture {
    stdout: CappedText,
    stderr: CappedText,
    combined: CappedText,
}

/// Finished capture: stdout, stderr, combined, and whether anything was cut.
#[derive(Debug)]
pub(crate) struct Captured {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) combined: String,
    pub(crate) truncated: bool,
}

impl Capture {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            stdout: CappedText::new(cap),
            stderr: CappedText::new(cap),
            combined: CappedText::new(cap),
        }
    }

    pub(crate) fn push(&mut self, stream: Stream, text: &str) {
        self.combined.push(text);
        match stream {
            Stream::Stdout => self.stdout.push(text),
            Stream::Stderr => self.stderr.push(text),
        }
    }

    pub(crate) fn finish(self) -> Captured {
        let (stdout, a) = self.stdout.finish();
        let (stderr, b) = self.stderr.finish();
        let (combined, c) = self.combined.finish();
        Captured {
            stdout,
            stderr,
            combined,
            truncated: a || b || c,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CappedText {
    head: String,
    head_cap: usize,
    tail: VecDeque<String>,
    tail_len: usize,
    tail_cap: usize,
    dropped: u64,
}

impl CappedText {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            head: String::new(),
            head_cap: cap / 2,
            tail: VecDeque::new(),
            tail_len: 0,
            tail_cap: cap - cap / 2,
            dropped: 0,
        }
    }

    pub(crate) fn push(&mut self, text: &str) {
        let mut rest = text;
        if self.tail.is_empty() && self.head.len() < self.head_cap {
            let take = floor_boundary(rest, self.head_cap - self.head.len());
            self.head.push_str(&rest[..take]);
            rest = &rest[take..];
        }
        if rest.is_empty() {
            return;
        }
        self.tail.push_back(rest.to_string());
        self.tail_len += rest.len();
        while self.tail_len > self.tail_cap {
            let excess = self.tail_len - self.tail_cap;
            let Some(front) = self.tail.front_mut() else {
                break;
            };
            let cut = if front.len() <= excess {
                front.len()
            } else {
                ceil_boundary(front, excess)
            };
            if cut == front.len() {
                self.tail.pop_front();
            } else {
                front.drain(..cut);
            }
            self.tail_len -= cut;
            self.dropped += cut as u64;
        }
    }

    /// The kept text (with an omission marker when bytes were dropped) and
    /// whether anything was dropped.
    pub(crate) fn finish(self) -> (String, bool) {
        let mut out = self.head;
        if self.dropped > 0 {
            out.push_str(&format!(
                "\n[... {} bytes of output omitted ...]\n",
                self.dropped
            ));
        }
        out.extend(self.tail);
        (out, self.dropped > 0)
    }
}

fn floor_boundary(s: &str, index: usize) -> usize {
    let mut i = index.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_boundary(s: &str, index: usize) -> usize {
    let mut i = index.min(s.len());
    while !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_output_passes_through() {
        let mut text = CappedText::new(100);
        text.push("hello\n");
        text.push("world\n");
        assert_eq!(text.finish(), ("hello\nworld\n".to_string(), false));
    }

    #[test]
    fn keeps_head_and_tail_and_counts_dropped_bytes() {
        let mut text = CappedText::new(20);
        for i in 0..100 {
            text.push(&format!("line{i:03}\n"));
        }
        let (out, truncated) = text.finish();
        assert!(truncated);
        assert!(out.starts_with("line000\nli\n[... 780 bytes"), "{out}");
        assert!(out.ends_with("8\nline099\n"), "{out}");
    }

    #[test]
    fn multibyte_text_is_cut_on_boundaries() {
        let mut text = CappedText::new(7);
        text.push(&"é".repeat(20));
        let (out, truncated) = text.finish();
        assert!(truncated);
        assert!(!out.contains('\u{FFFD}'));
    }
}
