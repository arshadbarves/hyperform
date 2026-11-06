//! Relevance-aware trimming of long command output (`decisions_output_trim`).
//! The middle of the output is cut into 40-line windows the decision model
//! scores for the task; the head, the tail and every window it did not
//! rule out stay, in order, with markers where windows were left out, and
//! the full output is spilled. Without a ranker, or without an answer,
//! the output goes through [`truncate_output`] exactly as before.

use std::ops::Range;

use crate::context::ToolCtx;
use crate::ports::{RankRequest, RankTarget};
use crate::text::truncate_output;

pub(crate) const WINDOW_LINES: usize = 40;
/// Shorter output (head, tail and two middle windows) is left alone.
const MIN_WINDOWS: usize = 4;
/// Middle windows asked about at most; beyond that the ones with the most
/// error-like lines are asked and the rest are left out.
const MAX_ASKED: usize = 24;
/// Characters of a window the model reads.
const EXCERPT_CHARS: usize = 1_200;
const SIGNALS: [&str; 9] = [
    "error",
    "fail",
    "panic",
    "warn",
    "exception",
    "traceback",
    "assert",
    "expected",
    "denied",
];

/// `text` for the model: trimmed by relevance when the ranker answers,
/// else truncated as before. `subject` is the command that produced it.
pub(crate) async fn trim_output(
    ctx: &ToolCtx,
    hint: &str,
    tool: &str,
    subject: &str,
    text: &str,
) -> String {
    let fallback = || truncate_output(ctx, hint, text);
    let Some(ranker) = ctx.ports.ranker(RankTarget::OutputWindows) else {
        return fallback();
    };
    let windows = windows(text);
    if windows.len() < MIN_WINDOWS {
        return fallback();
    }
    let asked = candidates(text, &windows);
    let middle = windows[1].start..windows[windows.len() - 2].end;
    let request = RankRequest {
        target: RankTarget::OutputWindows,
        tool: tool.to_string(),
        subject: subject.to_string(),
        items: asked
            .iter()
            .map(|&i| excerpt(&text[windows[i].clone()]))
            .collect(),
        chars: text[middle].chars().count(),
    };
    let Some(answers) = ranker.rank(ctx, request).await else {
        return fallback();
    };
    let Some(keep) = keep_mask(windows.len(), &asked, &answers) else {
        return fallback();
    };
    let budget = ctx.limits.max_result_chars;
    let keep = if size(text, &windows, &keep) <= budget {
        keep
    } else {
        let sure: Vec<Keep> = keep.iter().map(|k| k.only_sure()).collect();
        if size(text, &windows, &sure) > budget {
            return fallback();
        }
        sure
    };
    let saved = ctx.spill_output(hint, text);
    assemble(text, &windows, &keep, saved.as_deref())
}

/// Characters reserved for each elision marker when checking the budget.
const MARKER_ROOM: usize = 240;

/// Characters of the trimmed output, markers counted generously.
fn size(text: &str, windows: &[Range<usize>], keep: &[Keep]) -> usize {
    let kept: usize = windows
        .iter()
        .zip(keep)
        .filter(|(_, keep)| **keep != Keep::No)
        .map(|(range, _)| text[range.clone()].chars().count())
        .sum();
    let gaps = keep
        .windows(2)
        .filter(|pair| pair[0] != Keep::No && pair[1] == Keep::No)
        .count();
    kept + gaps * MARKER_ROOM
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Keep {
    /// The head, the tail, or a window the model called relevant.
    Yes,
    /// Asked, but the model was unsure.
    Unsure,
    No,
}

impl Keep {
    fn only_sure(self) -> Self {
        match self {
            Self::Unsure => Self::No,
            other => other,
        }
    }
}

/// Byte ranges of consecutive 40-line windows covering `text`.
fn windows(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let (mut start, mut lines) = (0, 0);
    for (index, _) in text.match_indices('\n') {
        lines += 1;
        if lines == WINDOW_LINES {
            out.push(start..index + 1);
            (start, lines) = (index + 1, 0);
        }
    }
    if start < text.len() {
        out.push(start..text.len());
    }
    out
}

/// Middle windows to ask about, in order: all of them, or the `MAX_ASKED`
/// with the most error-like lines.
fn candidates(text: &str, windows: &[Range<usize>]) -> Vec<usize> {
    let middle: Vec<usize> = (1..windows.len() - 1).collect();
    if middle.len() <= MAX_ASKED {
        return middle;
    }
    let signal = |i: usize| {
        let lower = text[windows[i].clone()].to_lowercase();
        let lines = lower.lines();
        lines
            .filter(|line| SIGNALS.iter().any(|word| line.contains(word)))
            .count()
    };
    let mut ranked: Vec<(usize, usize)> = middle.into_iter().map(|i| (signal(i), i)).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut picked: Vec<usize> = ranked.into_iter().take(MAX_ASKED).map(|(_, i)| i).collect();
    picked.sort_unstable();
    picked
}

fn excerpt(window: &str) -> String {
    window.chars().take(EXCERPT_CHARS).collect()
}

/// What to keep per window; `None` when the model left nothing out (every
/// answer yes or unsure), so today's output stands.
fn keep_mask(count: usize, asked: &[usize], answers: &[Option<bool>]) -> Option<Vec<Keep>> {
    if answers.len() != asked.len() || answers.iter().all(Option::is_none) {
        return None;
    }
    let mut keep = vec![Keep::No; count];
    keep[0] = Keep::Yes;
    keep[count - 1] = Keep::Yes;
    for (&index, answer) in asked.iter().zip(answers) {
        keep[index] = match answer {
            Some(true) => Keep::Yes,
            Some(false) => Keep::No,
            None => Keep::Unsure,
        };
    }
    keep.contains(&Keep::No).then_some(keep)
}

/// Kept windows in order; each run of left-out windows becomes one marker,
/// the first naming where the full output is saved.
fn assemble(
    text: &str,
    windows: &[Range<usize>],
    keep: &[Keep],
    saved: Option<&std::path::Path>,
) -> String {
    let total_lines = text.lines().count();
    let mut out = String::new();
    let mut gap: Option<(usize, usize)> = None;
    let mut first_marker = true;
    for (index, (range, keep)) in windows.iter().zip(keep).enumerate() {
        if *keep == Keep::No {
            let (from, _) = gap.unwrap_or((index, index));
            gap = Some((from, index));
            continue;
        }
        if let Some((from, to)) = gap.take() {
            let lines = (from * WINDOW_LINES + 1, (to + 1) * WINDOW_LINES);
            out.push_str(&marker(lines, total_lines, first_marker.then_some(saved)));
            first_marker = false;
        }
        out.push_str(&text[range.clone()]);
    }
    out
}

fn marker(
    (from, to): (usize, usize),
    total: usize,
    saved: Option<Option<&std::path::Path>>,
) -> String {
    let note = match saved {
        Some(Some(path)) => format!(
            " The full output ({total} lines) is saved at {}; read it with Read (offset and limit) or search it with Grep.",
            path.display()
        ),
        _ => String::new(),
    };
    format!("[... lines {from}-{to} left out: not relevant to the current task.{note}]\n")
}

#[cfg(test)]
#[path = "trim_tests.rs"]
mod tests;
