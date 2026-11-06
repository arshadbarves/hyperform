//! The plan's ledger: design decisions, discoveries, experiments, evidence
//! notes and plan changes. Entries are never deleted; they are dismissed or
//! superseded.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{EntryId, SessionId, TaskId};
use crate::verification::CheckKind;
use crate::work::plan::Provenance;

/// Something that backs a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum EvidenceRef {
    /// A `CheckRecord` of a session; `fingerprint` is the tree it ran on.
    Check {
        session_id: SessionId,
        record_id: String,
        passed: bool,
        fingerprint: String,
        #[serde(default)]
        kind: Option<CheckKind>,
        #[serde(default)]
        label: String,
        #[ts(type = "number")]
        #[serde(default)]
        at: u64,
    },
    /// Changed files, optionally against a checkpoint.
    Diff {
        files: Vec<String>,
        #[serde(default)]
        checkpoint: Option<String>,
        #[serde(default)]
        summary: String,
    },
    Note {
        text: String,
    },
    External {
        url: String,
        #[serde(default)]
        title: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DesignDecision {
    pub title: String,
    pub choice: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Discovery {
    pub text: String,
    /// A file, URL or command the fact was observed in.
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExperimentOutcome {
    #[default]
    Open,
    Confirmed,
    Refuted,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Experiment {
    pub hypothesis: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub outcome: ExperimentOutcome,
}

/// Evidence linked to a task (and possibly one of its criteria).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EvidenceEntry {
    pub task_id: TaskId,
    #[serde(default)]
    pub criterion_id: Option<String>,
    pub evidence: EvidenceRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChangeCause {
    User,
    Discovery,
    Experiment,
    Reconciliation,
    WorkerReport,
    Replan,
    /// What reopening the chat (or attaching the plan) found: interrupted
    /// tasks and files changed outside the plan.
    Resume,
}

/// A recorded change of plan: why, from what, to what, backed by what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanChange {
    pub cause: ChangeCause,
    /// The discovery or experiment behind it, if any.
    #[serde(default)]
    pub source: Option<EntryId>,
    pub original: String,
    pub new: String,
    pub reason: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub affected: Vec<TaskId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum EntryBody {
    DesignDecision(DesignDecision),
    Discovery(Discovery),
    Experiment(Experiment),
    Evidence(EvidenceEntry),
    PlanChange(PlanChange),
}

impl EntryBody {
    pub fn label(&self) -> &'static str {
        match self {
            Self::DesignDecision(_) => "design decision",
            Self::Discovery(_) => "discovery",
            Self::Experiment(_) => "experiment",
            Self::Evidence(_) => "evidence",
            Self::PlanChange(_) => "plan change",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum EntryStatus {
    /// Unconfirmed (for example extracted during compaction).
    Proposed,
    #[default]
    Accepted,
    Dismissed,
    Superseded {
        by: EntryId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Entry {
    pub id: EntryId,
    pub body: EntryBody,
    pub provenance: Provenance,
    pub status: EntryStatus,
    /// Tasks the entry is about.
    #[serde(default)]
    pub tasks: Vec<TaskId>,
    /// Files the entry is about (for relevance ranking).
    #[serde(default)]
    pub files: Vec<String>,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub created_revision: u64,
    /// The revision that last changed it.
    #[ts(type = "number")]
    pub revision: u64,
}
