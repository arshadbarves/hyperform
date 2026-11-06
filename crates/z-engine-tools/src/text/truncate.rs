//! Keeps results inside the model's budget: the head and tail survive, the
//! middle becomes a marker, and the full text is spilled to an artifact the
//! model can read back.

use std::path::PathBuf;

use z_engine_protocol::ToolResultPart;

use crate::context::ToolCtx;

/// Characters reserved for the marker (at most a quarter of the budget).
const MARKER_RESERVE: usize = 400;

/// `text` within `ctx.limits.max_result_chars`, spilling the full text
/// through `ctx.spill` (tagged `hint`) when it had to be cut.
pub(crate) fn truncate_output(ctx: &ToolCtx, hint: &str, text: &str) -> String {
    truncate_with(text, ctx.limits.max_result_chars, |full| {
        ctx.spill_output(hint, full)
    })
}

/// Every text part of `parts` within the budget; images pass through.
pub(crate) fn truncate_parts(
    ctx: &ToolCtx,
    hint: &str,
    parts: Vec<ToolResultPart>,
) -> Vec<ToolResultPart> {
    parts
        .into_iter()
        .map(|part| match part {
            ToolResultPart::Text { text } => ToolResultPart::Text {
                text: truncate_output(ctx, hint, &text),
            },
            image => image,
        })
        .collect()
}

pub(crate) fn truncate_with(
    text: &str,
    max_chars: usize,
    spill: impl FnOnce(&str) -> Option<PathBuf>,
) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    let saved = spill(text);
    let budget = max_chars
        .saturating_sub(MARKER_RESERVE.min(max_chars / 4))
        .max(2);
    let head_chars = budget / 2;
    let head_end = snap_head(text, byte_at(text, head_chars));
    let tail_start = snap_tail(text, byte_at(text, total - (budget - head_chars))).max(head_end);
    let middle = &text[head_end..tail_start];
    let omitted = middle.chars().count();
    let marker = match saved {
        Some(path) => format!(
            "\n\n[... {omitted} characters ({} lines) omitted. The full output ({total} characters) is saved at {}; read it with Read (offset and limit) or search it with Grep.]\n\n",
            middle.matches('\n').count(),
            path.display()
        ),
        None => format!("\n\n[... {omitted} characters omitted ...]\n\n"),
    };
    format!("{}{marker}{}", &text[..head_end], &text[tail_start..])
}

/// Byte offset of the `n`th character (the end when there are fewer).
fn byte_at(text: &str, n: usize) -> usize {
    text.char_indices().nth(n).map_or(text.len(), |(i, _)| i)
}

/// Ends the head after a newline when one is in its last quarter.
fn snap_head(text: &str, end: usize) -> usize {
    match text[..end].rfind('\n') {
        Some(newline) if newline + 1 >= end - end / 4 => newline + 1,
        _ => end,
    }
}

/// Starts the tail after a newline when one is in its first quarter.
fn snap_tail(text: &str, start: usize) -> usize {
    let tail = &text[start..];
    match tail.find('\n') {
        Some(newline) if newline < tail.len() / 4 => start + newline + 1,
        _ => start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_passes_through_without_spilling() {
        let out = truncate_with("hello", 10, |_| panic!("no spill expected"));
        assert_eq!(out, "hello");
    }

    #[test]
    fn long_text_keeps_head_and_tail_and_spills_the_full_text() {
        let text: String = (0..2_000).map(|i| format!("line {i}\n")).collect();
        let mut spilled = None;
        let out = truncate_with(&text, 1_000, |full| {
            spilled = Some(full.len());
            Some(PathBuf::from("/artifacts/out.log"))
        });
        assert_eq!(spilled, Some(text.len()));
        assert!(out.starts_with("line 0\n"));
        assert!(out.ends_with("line 1999\n"));
        assert!(out.contains("saved at /artifacts/out.log"), "{out}");
        assert!(out.chars().count() < 1_300);
    }

    #[test]
    fn multibyte_text_is_cut_on_char_boundaries() {
        let text = "é".repeat(5_000);
        let out = truncate_with(&text, 500, |_| None);
        assert!(out.contains("characters omitted"));
        assert!(out.starts_with('é') && out.ends_with('é'));
    }
}
