//! User edits from the Plan tab, sent as `Command::EditWork`. Each edit is
//! one revision with the user as actor; the kernel still validates it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{EntryId, TaskId};
use crate::work::plan::{ParallelPolicy, PlanItemKind};
use crate::work::record::TaskPatch;
use crate::work::task::CriterionMethod;

/// A criterion as written; `id: None` creates a new one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CriterionDraft {
    #[serde(default)]
    pub id: Option<String>,
    pub text: String,
    pub method: CriterionMethod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum WorkEdit {
    EditGoal {
        goal: String,
    },
    EditDocument {
        document: String,
    },
    /// Recorded with the user as its source.
    AddItem {
        kind: PlanItemKind,
        text: String,
    },
    EditItem {
        item_id: EntryId,
        text: String,
    },
    /// Accept a requirement or invariant an agent proposed.
    AcceptItem {
        item_id: EntryId,
    },
    RetireItem {
        item_id: EntryId,
    },
    SetParallel {
        parallel: ParallelPolicy,
    },
    /// Let independent tasks run in parallel helpers (policy `approved`).
    ApproveParallel,
    /// Add an audit finding to the plan: the agent is asked to address it.
    AddFinding {
        finding_id: String,
    },
    DismissFinding {
        finding_id: String,
    },
    /// Ask the audit agent to review the plan now.
    RequestAudit,
    /// Build, or accept the changes of a replanned plan: Draft or
    /// Replanning becomes Active.
    Approve,
    /// No new dispatch; running task helpers stop at their next step and
    /// their tasks wait with a resume pointer.
    Pause,
    /// A Paused plan becomes Active.
    Resume,
    /// Pause, cancel every task helper now and release every bound task.
    Stop,
    Abandon,
    /// Done or Abandoned becomes Active again.
    Reopen,
    RequestReplan,
    RenameTask {
        task_id: TaskId,
        title: String,
    },
    /// Criteria change through `SetCriteria`, not `patch.acceptance`.
    EditTask {
        task_id: TaskId,
        patch: Box<TaskPatch>,
    },
    MoveTask {
        task_id: TaskId,
        parent: Option<TaskId>,
        index: u32,
    },
    SetCriteria {
        task_id: TaskId,
        criteria: Vec<CriterionDraft>,
    },
    SetRunAlone {
        task_id: TaskId,
        run_alone: bool,
    },
    /// Held: never dispatched; a running helper stops at its next step.
    HoldTask {
        task_id: TaskId,
        held: bool,
    },
    /// Release a held task (`HoldTask` with `held: false`).
    ReleaseTask {
        task_id: TaskId,
    },
    /// A Failed or Blocked task goes back to Pending with a fresh attempt
    /// budget.
    RetryTask {
        task_id: TaskId,
    },
    /// Resolve a merge conflict: drop the conflicted change and run the
    /// task again, alone, on the current tree.
    RetrySerially {
        task_id: TaskId,
    },
    /// Run a Verifying task's check criteria again in the project tree;
    /// the engine runs them and records the results as evidence.
    RecheckTask {
        task_id: TaskId,
    },
    /// Accept a proposed task: it becomes Pending.
    AcceptTask {
        task_id: TaskId,
    },
    /// Delete a draft task or mark one not needed: it becomes Cancelled.
    CancelTask {
        task_id: TaskId,
    },
    WaiveCriterion {
        task_id: TaskId,
        criterion_id: String,
    },
    /// Mark a criterion met (the only way for `User` criteria).
    ConfirmCriterion {
        task_id: TaskId,
        criterion_id: String,
    },
    /// Confirm an unconfirmed ledger entry.
    ConfirmEntry {
        entry_id: EntryId,
    },
    DismissEntry {
        entry_id: EntryId,
    },
}
