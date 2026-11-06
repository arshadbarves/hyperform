//! Unified diffs for approval previews and change summaries.

use std::time::Duration;

use similar::{Algorithm, ChangeTag, TextDiff};

/// Largest diff an approval preview carries.
pub(crate) const PREVIEW_MAX_CHARS: usize = 20_000;
/// Myers gives up refining after this long on pathological inputs.
const DIFF_DEADLINE: Duration = Duration::from_millis(500);

pub(crate) fn text_diff<'a>(old: &'a str, new: &'a str) -> TextDiff<'a, 'a, 'a, str> {
    TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .timeout(DIFF_DEADLINE)
        .diff_lines(old, new)
}

/// `a/<path>` -> `b/<path>` with three lines of context.
pub(crate) fn unified_diff(old: &str, new: &str, display: &str) -> String {
    text_diff(old, new)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{display}"), &format!("b/{display}"))
        .to_string()
}

/// A new file as a diff from `/dev/null`.
pub(crate) fn creation_diff(new: &str, display: &str) -> String {
    text_diff("", new)
        .unified_diff()
        .context_radius(3)
        .header("/dev/null", &format!("b/{display}"))
        .to_string()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DiffStats {
    pub(crate) added: usize,
    pub(crate) removed: usize,
}

impl DiffStats {
    /// "+3 -1".
    pub(crate) fn label(self) -> String {
        format!("+{} -{}", self.added, self.removed)
    }
}

pub(crate) fn diff_stats(old: &str, new: &str) -> DiffStats {
    let mut stats = DiffStats::default();
    for change in text_diff(old, new).iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => stats.added += 1,
            ChangeTag::Delete => stats.removed += 1,
            ChangeTag::Equal => {}
        }
    }
    stats
}

/// `diff` cut at a line boundary to fit an approval card.
pub(crate) fn cap_preview(diff: String) -> String {
    if diff.len() <= PREVIEW_MAX_CHARS {
        return diff;
    }
    let mut end = PREVIEW_MAX_CHARS;
    while !diff.is_char_boundary(end) {
        end -= 1;
    }
    let end = diff[..end].rfind('\n').map_or(end, |newline| newline + 1);
    let hidden = diff[end..].lines().count();
    format!("{}... ({hidden} more diff lines not shown)\n", &diff[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_diff_has_headers_and_hunks() {
        let diff = unified_diff("a\nb\nc\n", "a\nB\nc\n", "src/x.rs");
        assert!(
            diff.starts_with("--- a/src/x.rs\n+++ b/src/x.rs\n@@"),
            "{diff}"
        );
        assert!(diff.contains("-b\n+B\n"));
        assert_eq!(diff_stats("a\nb\nc\n", "a\nB\nc\nd\n").label(), "+2 -1");
    }

    #[test]
    fn creation_diff_starts_from_dev_null() {
        let diff = creation_diff("x\ny\n", "new.txt");
        assert!(diff.starts_with("--- /dev/null\n+++ b/new.txt\n"), "{diff}");
        assert!(diff.contains("+x\n+y\n"));
    }

    #[test]
    fn previews_are_capped_on_line_boundaries() {
        let long: String = (0..5_000).map(|i| format!("+line {i}\n")).collect();
        let capped = cap_preview(long);
        assert!(capped.len() < PREVIEW_MAX_CHARS + 100);
        assert!(capped.ends_with("more diff lines not shown)\n"));
    }
}
