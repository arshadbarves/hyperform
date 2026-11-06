//! Folding a helper's report: its lists are trimmed, deduplicated and
//! capped, the changed files and the checks come from what the engine
//! observed, and the helper's own lists of those stay apart as claims. A
//! missing report becomes an `Unreported` one built from the observations.

use hyperform_protocol::work::{EvidenceRef, ReportClaims, ReportDraft, ReportSource, TaskReport};

use crate::error::Reason;
use crate::facts::RunFacts;

/// Items kept per list, and characters per item.
pub const REPORT_ITEMS: usize = 40;
pub const REPORT_ITEM_CHARS: usize = 500;
pub const REPORT_SUMMARY_CHARS: usize = 4_000;

/// The report of run `attempt`; `draft: None` is the `Unreported` one.
pub fn fold(
    draft: Option<&ReportDraft>,
    run: &RunFacts,
    attempt: u32,
    now: u64,
) -> Result<TaskReport, Reason> {
    let mut report = TaskReport {
        attempt,
        files_changed: run.files_changed.as_deref().map(paths).unwrap_or_default(),
        diff_known: run.files_changed.is_some(),
        tests_run: run
            .checks
            .iter()
            .filter(|evidence| matches!(evidence, EvidenceRef::Check { .. }))
            .cloned()
            .collect(),
        at: now,
        ..TaskReport::default()
    };
    let Some(draft) = draft else {
        report.source = ReportSource::Unreported;
        return Ok(report);
    };
    let summary = clip(draft.summary.trim(), REPORT_SUMMARY_CHARS);
    if summary.is_empty() {
        return Err(Reason::invalid("a report needs a summary of what was done"));
    }
    report.summary = summary;
    report.symbols_changed = list(&draft.symbols_changed);
    report.discoveries = list(&draft.discoveries);
    report.decisions = list(&draft.decisions);
    report.experiments = list(&draft.experiments);
    report.blockers = list(&draft.blockers);
    report.follow_ups = list(&draft.follow_ups);
    report.risks = list(&draft.risks);
    report.confidence = draft.confidence;
    report.claimed = ReportClaims {
        files_changed: paths(&draft.files_changed),
        tests_run: list(&draft.tests_run),
    };
    Ok(report)
}

fn list(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let item = clip(item.trim(), REPORT_ITEM_CHARS);
        if !item.is_empty() && !out.contains(&item) && out.len() < REPORT_ITEMS {
            out.push(item);
        }
    }
    out
}

/// Root-relative paths, sorted and deduplicated.
fn paths(files: &[String]) -> Vec<String> {
    let mut out: Vec<String> = files
        .iter()
        .map(|file| file.trim().trim_start_matches("./").to_string())
        .filter(|file| !file.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn clip(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use hyperform_protocol::work::ReportConfidence;

    use super::*;
    use crate::testkit::{FP, check};

    fn run() -> RunFacts {
        RunFacts {
            files_changed: Some(vec![
                "./src/b.rs".into(),
                "src/a.rs".into(),
                "src/a.rs".into(),
            ]),
            checks: vec![check("chk_1", true, FP)],
        }
    }

    #[test]
    fn derived_files_and_checks_win_over_the_claims() {
        let draft = ReportDraft {
            summary: "  Added retries  ".into(),
            files_changed: vec!["src/a.rs".into(), "src/imagined.rs".into()],
            tests_run: vec!["cargo test".into(), "cargo test".into(), " ".into()],
            risks: vec!["429 storms".into()],
            confidence: Some(ReportConfidence::High),
            ..ReportDraft::default()
        };
        let report = fold(Some(&draft), &run(), 2, 7).unwrap();
        assert_eq!(report.source, ReportSource::Worker);
        assert_eq!(report.summary, "Added retries");
        assert_eq!(report.files_changed, ["src/a.rs", "src/b.rs"]);
        assert!(report.diff_known);
        assert_eq!(report.tests_run, [check("chk_1", true, FP)]);
        assert_eq!(
            report.claimed.files_changed,
            ["src/a.rs", "src/imagined.rs"]
        );
        assert_eq!(report.claimed.tests_run, ["cargo test"]);
        assert_eq!((report.attempt, report.at), (2, 7));
        assert_eq!(report.confidence, Some(ReportConfidence::High));
    }

    #[test]
    fn a_missing_report_is_unreported_and_a_blank_summary_is_refused() {
        let unreported = fold(None, &RunFacts::default(), 1, 7).unwrap();
        assert_eq!(unreported.source, ReportSource::Unreported);
        assert!(!unreported.diff_known && unreported.files_changed.is_empty());
        let blank = ReportDraft {
            summary: "  ".into(),
            ..ReportDraft::default()
        };
        assert!(fold(Some(&blank), &run(), 1, 7).is_err());
    }

    #[test]
    fn lists_are_capped() {
        let draft = ReportDraft {
            summary: "x".repeat(REPORT_SUMMARY_CHARS + 10),
            discoveries: (0..100).map(|n| format!("d{n}")).collect(),
            ..ReportDraft::default()
        };
        let report = fold(Some(&draft), &run(), 1, 7).unwrap();
        assert_eq!(report.discoveries.len(), REPORT_ITEMS);
        assert_eq!(report.summary.chars().count(), REPORT_SUMMARY_CHARS + 1);
    }
}
