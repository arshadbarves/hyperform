//! The plan header: goal, status, depth, parallel policy, revision, and the
//! requirements and invariants the plan must honor.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{EntryId, PlanId, SessionId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlanStatus {
    /// Being planned; nothing is built yet.
    #[default]
    Draft,
    Active,
    /// No new work is dispatched.
    Paused,
    /// A change set waits for the user's re-approval.
    Replanning,
    Done,
    Abandoned,
}

impl PlanStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Replanning => "replanning",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }
}

/// How much structure the plan carries, set when it is created.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlanDepth {
    Light,
    #[default]
    Standard,
    Deep,
}

/// Whether independent tasks may run in parallel helpers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ParallelPolicy {
    #[default]
    Off,
    Ask,
    Approved,
}

/// Where a plan item or ledger entry came from. `Research` (web content)
/// and `Compaction` (summarizer extraction) are untrusted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Provenance {
    User,
    /// Also what tasks and reports logged before they carried one were.
    #[default]
    Agent,
    Research,
    Compaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlanItemKind {
    Requirement,
    Invariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlanItemStatus {
    /// Suggested by an agent; waits for the user.
    Proposed,
    Active,
    Retired,
}

/// A requirement or invariant. Items with `User` provenance are retired or
/// reworded only by the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanItem {
    pub id: EntryId,
    pub kind: PlanItemKind,
    pub text: String,
    pub provenance: Provenance,
    pub status: PlanItemStatus,
    /// The revision that last changed it.
    #[ts(type = "number")]
    pub revision: u64,
}

/// The plan-level state the GUI shows in the Plan tab's header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanHeader {
    pub plan_id: PlanId,
    pub project_root: String,
    pub goal: String,
    /// The plan document (the `ExitPlanMode` markdown); may be empty.
    #[serde(default)]
    pub document: String,
    pub status: PlanStatus,
    pub depth: PlanDepth,
    pub parallel: ParallelPolicy,
    #[ts(type = "number")]
    pub revision: u64,
    pub requirements: Vec<PlanItem>,
    pub invariants: Vec<PlanItem>,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
}

/// One plan in a project's plan list (also the store's `meta.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanSummary {
    pub plan_id: PlanId,
    pub project_root: String,
    pub goal: String,
    pub status: PlanStatus,
    pub depth: PlanDepth,
    #[ts(type = "number")]
    pub revision: u64,
    pub tasks_total: u32,
    pub tasks_done: u32,
    /// Tasks Blocked now.
    #[serde(default)]
    pub blocked: u32,
    /// What waits on the user: conflicts, pending applies, unreported or
    /// discarded runs, proposed tasks and items, open high-severity audit
    /// findings, and Verifying tasks whose checks need a recheck.
    #[serde(default)]
    pub needs_you: u32,
    /// Tasks Active now.
    #[serde(default)]
    pub running: u32,
    /// The chat that attached the plan last (it drives the plan); kept
    /// after a detach.
    #[serde(default)]
    pub last_session_id: Option<SessionId>,
    /// The chat the plan is attached to now (a closed chat included);
    /// `None` once it was detached.
    #[serde(default)]
    pub attached_to: Option<SessionId>,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
}
