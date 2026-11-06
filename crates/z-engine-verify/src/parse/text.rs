//! Text shared by the summary parsers: output lines with color codes and
//! carriage-return rewrites removed, `N word` terms, and a running total.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

/// A lazily compiled literal pattern (`None` only if the literal is invalid).
pub(super) fn regex(
    cell: &'static OnceLock<Option<Regex>>,
    pattern: &str,
) -> Option<&'static Regex> {
    cell.get_or_init(|| Regex::new(pattern).ok()).as_ref()
}

/// Output lines without ANSI escapes; a line redrawn with `\r` keeps only
/// its final text. Plain lines are borrowed, so clean output costs no
/// copies.
pub(super) fn clean_lines(output: &str) -> Vec<Cow<'_, str>> {
    static ANSI: OnceLock<Option<Regex>> = OnceLock::new();
    let ansi = regex(
        &ANSI,
        r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-Z\\-_]",
    );
    output
        .lines()
        .map(|line| {
            if !line.contains(['\x1b', '\r']) {
                return Cow::Borrowed(line);
            }
            let last = line
                .rsplit('\r')
                .find(|part| !part.is_empty())
                .unwrap_or("");
            match ansi {
                Some(ansi) if last.contains('\x1b') => ansi.replace_all(last, ""),
                _ => Cow::Borrowed(last),
            }
        })
        .collect()
}

/// A decimal count; values beyond `u32` saturate.
pub(super) fn number(digits: &str) -> u32 {
    digits
        .parse::<u64>()
        .map_or(u32::MAX, |n| u32::try_from(n).unwrap_or(u32::MAX))
}

/// Whether `text` starts with an ASCII digit (a cheap guard before a regex).
pub(super) fn starts_with_digit(text: &str) -> bool {
    text.as_bytes().first().is_some_and(u8::is_ascii_digit)
}

/// `(count, word)` pairs of a summary such as `1 failed, 5 passed`.
pub(super) fn terms(summary: &str) -> Vec<(u32, String)> {
    static TERM: OnceLock<Option<Regex>> = OnceLock::new();
    let Some(term) = regex(&TERM, r"(\d+) ([A-Za-z]+)") else {
        return Vec::new();
    };
    term.captures_iter(summary)
        .map(|c| (number(&c[1]), c[2].to_ascii_lowercase()))
        .collect()
}

/// Sums counts over every summary a run printed (one per test binary,
/// package, assembly or session).
#[derive(Debug, Default)]
pub(super) struct Tally {
    counts: TestCounts,
    seen: bool,
}

impl Tally {
    pub(super) fn add(&mut self, passed: u32, failed: u32, skipped: u32) {
        self.counts.passed = self.counts.passed.saturating_add(passed);
        self.counts.failed = self.counts.failed.saturating_add(failed);
        self.counts.skipped = self.counts.skipped.saturating_add(skipped);
        self.seen = true;
    }

    /// `None` when no summary was seen.
    pub(super) fn finish(self) -> Option<TestCounts> {
        self.seen.then_some(self.counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_redraws_are_removed() {
        let lines = clean_lines("\x1b[32mok\x1b[0m\r\nloading 10%\rloading 100%\rdone\r\nplain\n");
        assert_eq!(lines, ["ok", "done", "plain"]);
        assert!(matches!(lines[2], Cow::Borrowed(_)));
    }

    #[test]
    fn terms_are_lowercased_and_saturate() {
        assert_eq!(
            terms("1 Failed, 99999999999 passed"),
            [(1, "failed".to_string()), (u32::MAX, "passed".to_string())]
        );
        assert!(starts_with_digit("5 passed") && !starts_with_digit("passed"));
    }

    #[test]
    fn an_empty_tally_is_none() {
        assert_eq!(Tally::default().finish(), None);
        let mut tally = Tally::default();
        tally.add(0, 0, 0);
        assert_eq!(tally.finish(), Some(TestCounts::default()));
    }
}
