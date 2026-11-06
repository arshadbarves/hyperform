//! The append-only plan log: one `WorkRecord` per accepted event. Every
//! record of one tool call or GUI edit shares a revision; folding the log
//! gives the current plan.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AgentId, CallId, EntryId, PlanId, SessionId, TaskId};
use crate::interaction::TodoItem;
use crate::work::audit::{AuditFinding, AuditStatus};
use crate::work::finding::ReconcileFinding;
use crate::work::ledger::{Entry, EntryStatus, EvidenceRef, ExperimentOutcome};
use crate::work::plan::{ParallelPolicy, PlanDepth, PlanItem, PlanItemStatus, PlanStatus};
use crate::work::report::TaskReport;
use crate::work::run::{Integration, TaskDispatch};
use crate::work::task::{
    Criterion, CriterionStatus, ResumePointer, Task, TaskKind, TaskNotes, TaskOwner, TaskPriority,
    TaskScope, TaskStatus,
};

/// Who made a change.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Actor {
    User,
    /// Follow-up changes the kernel derives (propagation, reconciliation).
    Kernel,
    Agent {
        session_id: SessionId,
        agent_id: AgentId,
    },
    /// Extraction from a compaction summary (untrusted).
    Compaction,
}

/// Why a revision happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum WorkCause {
    Tool {
        tool: String,
        #[serde(default)]
        call_id: Option<CallId>,
    },
    /// A `Command::EditWork` from the GUI.
    Edit,
    /// The user approved the plan.
    Approval,
    Reconciliation,
    /// Part of the change set of a `PlanChange` entry.
    Change {
        entry_id: EntryId,
    },
    Compaction,
    Resume,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkRecord {
    /// 1-based position in the plan log.
    #[ts(type = "number")]
    pub seq: u64,
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "number")]
    pub at: u64,
    pub actor: Actor,
    pub cause: WorkCause,
    pub event: WorkEvent,
}

/// Field changes of `TaskEdited`; `None` leaves a field as it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskPatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub active_form: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub kind: Option<TaskKind>,
    #[serde(default)]
    pub priority: Option<TaskPriority>,
    #[serde(default)]
    pub scope: Option<TaskScope>,
    #[serde(default)]
    pub notes: Option<TaskNotes>,
    /// Replaces the whole criteria list.
    #[serde(default)]
    pub acceptance: Option<Vec<Criterion>>,
    #[serde(default)]
    pub requirements: Option<Vec<EntryId>>,
    #[serde(default)]
    pub run_alone: Option<bool>,
    #[serde(default)]
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum WorkEvent {
    /// First record of every plan log.
    PlanCreated {
        plan_id: PlanId,
        project_root: String,
        goal: String,
        #[serde(default)]
        document: String,
        depth: PlanDepth,
        status: PlanStatus,
    },
    PlanEdited {
        #[serde(default)]
        goal: Option<String>,
        #[serde(default)]
        document: Option<String>,
    },
    PlanStatusChanged {
        from: PlanStatus,
        to: PlanStatus,
    },
    ParallelChanged {
        parallel: ParallelPolicy,
    },
    /// The engine re-assessed how much structure the plan deserves.
    DepthChanged {
        depth: PlanDepth,
    },
    ItemAdded {
        item: PlanItem,
    },
    ItemEdited {
        item_id: EntryId,
        text: String,
    },
    ItemStatusChanged {
        item_id: EntryId,
        status: PlanItemStatus,
    },
    TaskCreated {
        task: Box<Task>,
    },
    TaskEdited {
        task_id: TaskId,
        patch: Box<TaskPatch>,
    },
    /// Placed under `parent` at `index` among its siblings.
    TaskMoved {
        task_id: TaskId,
        parent: Option<TaskId>,
        index: u32,
    },
    TaskTransitioned {
        task_id: TaskId,
        from: TaskStatus,
        to: TaskStatus,
    },
    TaskHeld {
        task_id: TaskId,
        held: bool,
    },
    TaskBound {
        task_id: TaskId,
        owner: TaskOwner,
    },
    TaskReleased {
        task_id: TaskId,
    },
    DependencyAdded {
        task_id: TaskId,
        depends_on: TaskId,
    },
    DependencyRemoved {
        task_id: TaskId,
        depends_on: TaskId,
    },
    /// The micro-plan; replaces the previous steps.
    StepsRecorded {
        task_id: TaskId,
        steps: Vec<TodoItem>,
    },
    CriterionChanged {
        task_id: TaskId,
        criterion_id: String,
        status: CriterionStatus,
    },
    /// Attached to the criterion when `criterion_id` is set.
    EvidenceLinked {
        task_id: TaskId,
        #[serde(default)]
        criterion_id: Option<String>,
        evidence: EvidenceRef,
    },
    EntryRecorded {
        entry: Box<Entry>,
    },
    EntryStatusChanged {
        entry_id: EntryId,
        status: EntryStatus,
    },
    ExperimentConcluded {
        entry_id: EntryId,
        outcome: ExperimentOutcome,
        #[serde(default)]
        result: Option<String>,
        #[serde(default)]
        conclusion: Option<String>,
    },
    Reconciled {
        findings: Vec<ReconcileFinding>,
    },
    /// Sent to a parallel helper; starts a new run.
    TaskDispatched {
        task_id: TaskId,
        dispatch: TaskDispatch,
    },
    TaskReported {
        task_id: TaskId,
        report: Box<TaskReport>,
    },
    /// How the helper's worktree change came back.
    TaskIntegrated {
        task_id: TaskId,
        integration: Integration,
    },
    /// The task's helper stopped before it finished (the plan was paused,
    /// the task held, the chat closed or the plan moved): the task went back
    /// to Pending, the interrupted start no longer counts against
    /// `max_attempts`, and `resume` says where it can pick up.
    TaskInterrupted {
        task_id: TaskId,
        resume: ResumePointer,
    },
    /// The user discarded the worktree the task's resume pointer kept: the
    /// pointer keeps its agent and note, and the next run starts clean.
    ResumeWorktreeDiscarded {
        task_id: TaskId,
    },
    /// Back to Pending for a new run; `serial` makes it run alone, and
    /// `fresh_attempts` (a user retry) resets the attempt budget.
    TaskRetried {
        task_id: TaskId,
        #[serde(default)]
        serial: bool,
        #[serde(default)]
        fresh_attempts: bool,
    },
    /// Upserts of audit findings (new, changed or resolved).
    Audited {
        findings: Vec<AuditFinding>,
    },
    FindingStatusChanged {
        finding_id: String,
        status: AuditStatus,
    },
    /// The workspace fingerprint (and checkpoint) the plan last saw.
    CheckpointLinked {
        fingerprint: String,
        #[serde(default)]
        checkpoint: Option<String>,
    },
    /// Audit trail of a refused proposal; changes nothing.
    Rejected {
        reasons: Vec<String>,
    },
}
