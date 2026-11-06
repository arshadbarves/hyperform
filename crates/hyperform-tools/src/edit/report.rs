//! The model-facing result of an applied edit: how each replacement found
//! its target and a numbered snippet of the changed regions.

use std::path::Path;

use super::engine::{EditReport, MatchKind, rung_label};
use super::flow::FileEdit;
use super::snippet::change_snippet;
use crate::context::ToolCtx;
use crate::output::ToolOutput;
use crate::text::diff_stats;

/// Describes `edit` of `path`; `label_edits` prefixes notes with the edit
/// number (`MultiEdit`).
pub(crate) fn describe(
    ctx: &ToolCtx,
    path: &Path,
    edit: &FileEdit,
    label_edits: bool,
) -> ToolOutput {
    let display = ctx.display(path);
    let content = &edit.applied.content;
    let Some(before) = &edit.before else {
        return ToolOutput::text(
            format!("Created {display} ({} lines).", content.lines().count()),
            format!("Created {display}"),
        )
        .wrote(path);
    };
    let mut text = format!("The file {display} has been updated.");
    for (index, report) in edit.applied.reports.iter().enumerate() {
        if let Some(note) = note(report) {
            let prefix = if label_edits {
                format!("Edit {}: ", index + 1)
            } else {
                String::new()
            };
            text.push_str(&format!("\n{prefix}{note}"));
        }
    }
    text.push_str("\nHere is the result, numbered like Read:\n");
    text.push_str(&change_snippet(
        before,
        content,
        ctx.limits.read_max_line_chars,
    ));
    ToolOutput::text(
        text,
        format!(
            "Updated {display} ({})",
            diff_stats(before, content).label()
        ),
    )
    .wrote(path)
}

fn note(report: &EditReport) -> Option<String> {
    match report.kind {
        MatchKind::Exact if report.replacements > 1 => {
            Some(format!("Replaced {} occurrences.", report.replacements))
        }
        MatchKind::Exact => None,
        MatchKind::Ladder { rung, first, last } => Some(format!(
            "old_string was not found exactly; it matched lines {first}-{last} {}. Check the result below.",
            rung_label(rung)
        )),
        MatchKind::Created => Some("Wrote the content into the empty file.".to_string()),
    }
}
