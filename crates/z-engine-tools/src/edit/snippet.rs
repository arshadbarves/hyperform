//! Numbered excerpts of the new content around every changed region, so the
//! model sees the result of an edit without reading the file again.

use crate::text::{numbered, text_diff};

/// Unchanged lines shown around each change.
const CONTEXT: usize = 3;
/// Lines shown across all regions before the rest is summarized.
const MAX_LINES: usize = 80;

pub(crate) fn change_snippet(old: &str, new: &str, max_line_chars: usize) -> String {
    let lines: Vec<&str> = new.lines().collect();
    if lines.is_empty() {
        return "(the file is now empty)\n".to_string();
    }
    let groups = text_diff(old, new).grouped_ops(CONTEXT);
    let mut out = String::new();
    let mut shown = 0;
    for (index, group) in groups.iter().enumerate() {
        let (Some(first), Some(last)) = (group.first(), group.last()) else {
            continue;
        };
        let start = first.new_range().start.min(lines.len());
        let end = last.new_range().end.min(lines.len());
        let room = MAX_LINES.saturating_sub(shown);
        if room == 0 {
            out.push_str(&format!(
                "... ({} more changed regions not shown)\n",
                groups.len() - index
            ));
            break;
        }
        if index > 0 {
            out.push_str("   ...\n");
        }
        let visible = (end - start).min(room);
        out.push_str(&numbered(
            lines[start..start + visible].iter().copied(),
            start + 1,
            max_line_chars,
        ));
        if visible < end - start {
            out.push_str(&format!("... ({} more lines)\n", end - start - visible));
        }
        shown += visible;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_numbered_context_around_each_change() {
        let old: String = (1..=20).map(|i| format!("line {i}\n")).collect();
        let new = old
            .replace("line 3\n", "LINE 3\n")
            .replace("line 18\n", "LINE 18\n");
        let snippet = change_snippet(&old, &new, 100);
        assert!(snippet.starts_with("     1\tline 1\n"), "{snippet}");
        assert!(snippet.contains("     3\tLINE 3\n"));
        assert!(snippet.contains("   ...\n"));
        assert!(snippet.contains("    18\tLINE 18\n"));
        assert!(!snippet.contains("\tline 10\n"));
    }

    #[test]
    fn empty_results_are_described() {
        assert_eq!(change_snippet("a\n", "", 10), "(the file is now empty)\n");
    }
}
