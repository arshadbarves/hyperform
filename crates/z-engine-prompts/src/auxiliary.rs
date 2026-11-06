//! Side-request prompts: session titles, compaction summaries, web extraction.

/// System prompt for the 3-7 word sidebar title of a new session.
pub const TITLE: &str = include_str!("../prompts/auxiliary/title.md");

/// System prompt for summarizing a long conversation during compaction.
pub const COMPACT: &str = include_str!("../prompts/auxiliary/compact.md");

/// System prompt for answering a `WebFetch` prompt from fetched page content.
pub const WEB_EXTRACT: &str = include_str!("../prompts/auxiliary/web-extract.md");

/// Second system block of the compaction request while a work plan is
/// attached: summarize the plan briefly and end with a `json` block of
/// candidate discoveries.
pub const COMPACT_WORK: &str = include_str!("../prompts/auxiliary/compact-work.md");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_are_not_empty() {
        for prompt in [TITLE, COMPACT, WEB_EXTRACT, COMPACT_WORK] {
            assert!(!prompt.trim().is_empty());
        }
    }
}
