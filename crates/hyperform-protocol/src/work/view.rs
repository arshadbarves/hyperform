//! What the GUI receives: the full plan in a session snapshot, upserts
//! after each revision, and the steps of a task's "Why?" timeline.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{EntryId, PlanId, TaskId};
use crate::work::audit::AuditFinding;
use crate::work::finding::ReconcileFinding;
use crate::work::health::PlanHealth;
use crate::work::ledger::Entry;
use crate::work::plan::PlanHeader;
use crate::work::record::{Actor, WorkCause, WorkEvent};
use crate::work::task::Task;

/// The plan attached to a session, in full.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkSnapshot {
    pub plan: PlanHeader,
    pub tasks: Vec<Task>,
    pub entries: Vec<Entry>,
    /// Findings of the latest reconciliation.
    #[serde(default)]
    pub findings: Vec<ReconcileFinding>,
    /// Every audit finding, resolved and dismissed ones included.
    #[serde(default)]
    pub audit: Vec<AuditFinding>,
    #[serde(default)]
    pub health: PlanHealth,
    /// While findings keep the plan Replanning: the tasks they affect (and
    /// their dependents), held until the user accepts the plan change.
    #[serde(default)]
    pub replan_held: Vec<TaskId>,
}

/// One revision for the GUI: task and entry upserts, so the GUI never
/// re-implements the fold (tasks and entries are never removed, only
/// closed), plus the current audit findings, reconciliation findings and
/// health, which replace the previous ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkUpdate {
    pub plan_id: PlanId,
    #[ts(type = "number")]
    pub revision: u64,
    pub plan: PlanHeader,
    /// Tasks changed by this revision.
    pub tasks: Vec<Task>,
    /// Entries changed by this revision.
    pub entries: Vec<Entry>,
    /// The `PlanChange` entry this revision belongs to.
    pub change: Option<EntryId>,
    #[serde(default)]
    pub audit: Vec<AuditFinding>,
    #[serde(default)]
    pub findings: Vec<ReconcileFinding>,
    #[serde(default)]
    pub health: PlanHealth,
    /// Replaces the previous held set (see `WorkSnapshot::replan_held`).
    #[serde(default)]
    pub replan_held: Vec<TaskId>,
}

/// One step of `why(task)`: a record that touched the task, with the plan
/// change and the discovery or experiment behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WhyStep {
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "number")]
    pub at: u64,
    pub actor: Actor,
    pub cause: WorkCause,
    pub event: WorkEvent,
    pub entries: Vec<Entry>,
}
