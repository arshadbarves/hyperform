//! Deterministic signals about a request: the complexity tiers and their
//! effort, whether a request is clearly large (never routed down), and
//! whether a message is a short reply that continues the current task.

use std::collections::HashSet;

use z_engine_protocol::Effort;

/// Longer requests are never given low effort or the fast model.
const LARGE_CHARS: usize = 1_500;
/// Requests naming this many files count as large.
const LARGE_FILES: usize = 4;
/// A message this short continues the task ("yes", "go on", "thanks").
const REPLY_CHARS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tier {
    Simple,
    Moderate,
    Complex,
}

impl Tier {
    pub(super) fn parse(key: &str) -> Option<Self> {
        match key {
            "simple" => Some(Self::Simple),
            "moderate" => Some(Self::Moderate),
            "complex" => Some(Self::Complex),
            _ => None,
        }
    }

    pub(super) fn effort(self) -> Effort {
        match self {
            Self::Simple => Effort::Low,
            Self::Moderate => Effort::Medium,
            Self::Complex => Effort::High,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Moderate => "moderate",
            Self::Complex => "complex",
        }
    }
}

/// Long, or naming many files: too big to route down.
pub(super) fn looks_large(text: &str) -> bool {
    text.chars().count() > LARGE_CHARS || file_mentions(text) >= LARGE_FILES
}

/// A short reply that continues the task without asking.
pub(super) fn is_reply(text: &str) -> bool {
    text.trim().chars().count() <= REPLY_CHARS
}

fn file_mentions(text: &str) -> usize {
    text.split_whitespace()
        .map(|word| word.trim_matches(|c: char| !(c.is_alphanumeric() || "/._-".contains(c))))
        .filter(|word| looks_like_path(word))
        .collect::<HashSet<_>>()
        .len()
}

/// `src/lib.rs`, `main.ts`, `docs/`: a stem and a short extension, or a
/// slash. Abbreviations like "e.g." have one-letter parts and do not count.
fn looks_like_path(word: &str) -> bool {
    if word.len() > 1 && word.contains('/') {
        return true;
    }
    let Some((stem, extension)) = word.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && (2..=5).contains(&extension.len())
        && extension.chars().all(|c| c.is_ascii_alphanumeric())
        && extension.chars().any(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_map_to_efforts() {
        assert_eq!(Tier::parse("simple").map(Tier::effort), Some(Effort::Low));
        assert_eq!(Tier::parse("complex").map(Tier::effort), Some(Effort::High));
        assert_eq!(Tier::parse("huge"), None);
    }

    #[test]
    fn long_requests_and_many_files_are_large() {
        assert!(looks_large(&"word ".repeat(400)));
        assert!(looks_large(
            "update src/a.rs, src/b.rs, lib/c.ts and docs/d.md"
        ));
        assert!(!looks_large("fix the typo in README.md, e.g. the title"));
    }

    #[test]
    fn only_short_messages_are_replies() {
        assert!(is_reply("yes, go on"));
        assert!(!is_reply("now add a settings page for the theme"));
    }
}
