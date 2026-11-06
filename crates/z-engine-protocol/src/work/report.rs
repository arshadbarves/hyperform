//! A helper's result. What the helper writes is advisory; the files it
//! changed and the checks it ran come from the kernel (the worktree diff
//! and the check records), and its own lists are kept apart as claims.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::work::ledger::EvidenceRef;
use crate::work::plan::Provenance;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReportConfidence {
    Low,
    Medium,
    High,
}

/// What a helper sends with `TaskUpdate { report }`. Never evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ReportDraft {
    pub summary: String,
    pub files_changed: Vec<String>,
    pub symbols_changed: Vec<String>,
    pub discoveries: Vec<String>,
    pub decisions: Vec<String>,
    pub experiments: Vec<String>,
    pub tests_run: Vec<String>,
    pub blockers: Vec<String>,
    pub follow_ups: Vec<String>,
    pub risks: Vec<String>,
    pub confidence: Option<ReportConfidence>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReportSource {
    #[default]
    Worker,
    /// The helper ended without a report; the kernel wrote this one from
    /// what it observed and blocked the task.
    Unreported,
}

/// The helper's own lists of what the kernel derives itself: shown beside
/// the derived lists, never trusted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ReportClaims {
    pub files_changed: Vec<String>,
    pub tests_run: Vec<String>,
}

/// A helper's report as the kernel folded it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct TaskReport {
    pub source: ReportSource,
    /// The run it belongs to (`TaskDispatch::attempt`).
    pub attempt: u32,
    pub summary: String,
    /// From the worktree diff. Empty with `diffKnown: false` when the
    /// engine could not read the diff.
    pub files_changed: Vec<String>,
    pub diff_known: bool,
    pub symbols_changed: Vec<String>,
    pub discoveries: Vec<String>,
    pub decisions: Vec<String>,
    pub experiments: Vec<String>,
    /// Check records of the run.
    pub tests_run: Vec<EvidenceRef>,
    pub blockers: Vec<String>,
    pub follow_ups: Vec<String>,
    pub risks: Vec<String>,
    pub confidence: Option<ReportConfidence>,
    pub claimed: ReportClaims,
    /// `research` when the helper's run read web content: its discoveries,
    /// decisions and experiments are unconfirmed until the user takes them.
    pub provenance: Provenance,
    #[ts(type = "number")]
    pub at: u64,
}
