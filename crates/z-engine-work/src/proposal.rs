//! What tools, the GUI and the engine submit. A submission is all or
//! nothing and becomes one revision.

use z_engine_protocol::work::{
    Actor, AuditFinding, AuditSource, AuditStatus, CriterionDraft, CriterionStatus, EntryBody,
    EntryStatus, EvidenceRef, ExperimentOutcome, Integration, ParallelPolicy, PlanDepth,
    PlanItemKind, PlanItemStatus, PlanStatus, ReconcileFinding, ReportDraft, ResumePointer,
    TaskDispatch, TaskKind, TaskNotes, TaskOwner, TaskPatch, TaskPriority, TaskScope, TaskStatus,
    WorkCause, WorkEdit,
};
use z_engine_protocol::{AgentId, EntryId, PlanId, TaskId, TodoItem};

/// A new plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDraft {
    pub plan_id: PlanId,
    pub project_root: String,
    pub goal: String,
    pub document: String,
    pub depth: PlanDepth,
    /// `Draft` (planning) or `Active` (created while building).
    pub status: PlanStatus,
}

/// A new task; `id: None` lets the kernel pick one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskDraft {
    pub id: Option<TaskId>,
    pub parent: Option<TaskId>,
    pub title: String,
    pub active_form: String,
    pub description: String,
    pub kind: TaskKind,
    pub priority: TaskPriority,
    pub depends_on: Vec<TaskId>,
    pub requirements: Vec<EntryId>,
    pub scope: TaskScope,
    pub notes: TaskNotes,
    pub acceptance: Vec<CriterionDraft>,
    pub run_alone: bool,
    /// `Proposed` or `Pending`; `None` picks by actor (workers propose).
    pub status: Option<TaskStatus>,
}

impl TaskDraft {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
}

/// A new ledger entry; `proposed` records it unconfirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDraft {
    pub id: Option<EntryId>,
    pub body: EntryBody,
    pub tasks: Vec<TaskId>,
    pub files: Vec<String>,
    pub proposed: bool,
}

impl EntryDraft {
    pub fn new(body: EntryBody) -> Self {
        Self {
            id: None,
            body,
            tasks: Vec::new(),
            files: Vec::new(),
            proposed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proposal {
    EditPlan {
        goal: Option<String>,
        document: Option<String>,
    },
    /// `only_from` (when not empty) lists the statuses this may leave.
    SetPlanStatus {
        status: PlanStatus,
        only_from: Vec<PlanStatus>,
    },
    /// Pause the plan and release every bound task.
    StopPlan,
    SetParallel {
        parallel: ParallelPolicy,
    },
    /// The plan's re-assessed depth (engine only).
    SetDepth {
        depth: PlanDepth,
    },
    AddItem {
        id: Option<EntryId>,
        kind: PlanItemKind,
        text: String,
    },
    EditItem {
        item_id: EntryId,
        text: String,
    },
    SetItemStatus {
        item_id: EntryId,
        status: PlanItemStatus,
    },
    CreateTask(Box<TaskDraft>),
    EditTask {
        task_id: TaskId,
        patch: Box<TaskPatch>,
    },
    MoveTask {
        task_id: TaskId,
        parent: Option<TaskId>,
        index: u32,
    },
    SetTaskStatus {
        task_id: TaskId,
        status: TaskStatus,
    },
    HoldTask {
        task_id: TaskId,
        held: bool,
    },
    /// Failed or Blocked back to Pending.
    RetryTask {
        task_id: TaskId,
    },
    /// Changes nothing: refused unless the task is Verifying, so the
    /// engine may run its checks again (user only).
    Recheck {
        task_id: TaskId,
    },
    BindTask {
        task_id: TaskId,
        owner: TaskOwner,
    },
    ReleaseTask {
        task_id: TaskId,
    },
    AddDependency {
        task_id: TaskId,
        depends_on: TaskId,
    },
    RemoveDependency {
        task_id: TaskId,
        depends_on: TaskId,
    },
    RecordSteps {
        task_id: TaskId,
        steps: Vec<TodoItem>,
    },
    SetCriteria {
        task_id: TaskId,
        criteria: Vec<CriterionDraft>,
    },
    SetCriterion {
        task_id: TaskId,
        criterion_id: String,
        status: CriterionStatus,
    },
    /// Check evidence is linked by the engine (`Actor::Kernel`) only.
    LinkEvidence {
        task_id: TaskId,
        criterion_id: Option<String>,
        evidence: EvidenceRef,
    },
    Record(Box<EntryDraft>),
    SetEntryStatus {
        entry_id: EntryId,
        status: EntryStatus,
    },
    ConcludeExperiment {
        entry_id: EntryId,
        outcome: ExperimentOutcome,
        result: Option<String>,
        conclusion: Option<String>,
    },
    /// Record findings; any finding moves an Active plan to Replanning.
    Reconcile {
        findings: Vec<ReconcileFinding>,
    },
    LinkCheckpoint {
        fingerprint: String,
        checkpoint: Option<String>,
    },
    /// Send a Ready task to a parallel helper: binds it to the helper and
    /// starts it (see `dispatch::check`).
    DispatchTask {
        task_id: TaskId,
        dispatch: TaskDispatch,
    },
    /// A helper's report; `None` (engine only) records that it ended
    /// without one and blocks the task.
    Report {
        task_id: TaskId,
        report: Option<Box<ReportDraft>>,
    },
    /// How a helper's worktree change came back (engine only).
    Integrate {
        task_id: TaskId,
        integration: Integration,
    },
    /// `RetryTask`, and the task runs alone from now on (after a conflict).
    RetrySerial {
        task_id: TaskId,
    },
    /// The agent `resume` names stopped before finishing the task it runs
    /// (engine only): Pending again, with the pointer.
    InterruptTask {
        task_id: TaskId,
        resume: ResumePointer,
    },
    /// The user discarded the worktree of `agent_id`, which the task's
    /// resume pointer names (engine only): the pointer drops its worktree.
    DiscardResumeWorktree {
        task_id: TaskId,
        agent_id: AgentId,
    },
    /// Upsert audit findings (engine only); a deterministic audit resolves
    /// the open findings it no longer finds.
    Audit {
        source: AuditSource,
        findings: Vec<AuditFinding>,
    },
    /// Add a finding to the plan, dismiss it, or reopen it (user only).
    SetFindingStatus {
        finding_id: String,
        status: AuditStatus,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    pub actor: Actor,
    pub cause: WorkCause,
    pub proposals: Vec<Proposal>,
}

impl Submission {
    pub fn new(actor: Actor, cause: WorkCause, proposals: Vec<Proposal>) -> Self {
        Self {
            actor,
            cause,
            proposals,
        }
    }

    /// A Plan-tab edit: the user is the actor.
    pub fn edit(edit: WorkEdit) -> Self {
        Self::new(Actor::User, WorkCause::Edit, vec![edit.into()])
    }
}

impl From<WorkEdit> for Proposal {
    fn from(edit: WorkEdit) -> Self {
        use PlanStatus::{Abandoned, Active, Done, Draft, Paused, Replanning};
        let status = |status, only_from: &[PlanStatus]| Proposal::SetPlanStatus {
            status,
            only_from: only_from.to_vec(),
        };
        let patch = |task_id, patch| Proposal::EditTask {
            task_id,
            patch: Box::new(patch),
        };
        match edit {
            WorkEdit::EditGoal { goal } => Proposal::EditPlan {
                goal: Some(goal),
                document: None,
            },
            WorkEdit::EditDocument { document } => Proposal::EditPlan {
                goal: None,
                document: Some(document),
            },
            WorkEdit::AddItem { kind, text } => Proposal::AddItem {
                id: None,
                kind,
                text,
            },
            WorkEdit::EditItem { item_id, text } => Proposal::EditItem { item_id, text },
            WorkEdit::AcceptItem { item_id } => Proposal::SetItemStatus {
                item_id,
                status: PlanItemStatus::Active,
            },
            WorkEdit::RetireItem { item_id } => Proposal::SetItemStatus {
                item_id,
                status: PlanItemStatus::Retired,
            },
            WorkEdit::SetParallel { parallel } => Proposal::SetParallel { parallel },
            WorkEdit::ApproveParallel => Proposal::SetParallel {
                parallel: ParallelPolicy::Approved,
            },
            WorkEdit::AddFinding { finding_id } => Proposal::SetFindingStatus {
                finding_id,
                status: AuditStatus::Added,
            },
            WorkEdit::DismissFinding { finding_id } => Proposal::SetFindingStatus {
                finding_id,
                status: AuditStatus::Dismissed,
            },
            // The engine runs the audit agent for it; submitted as a user
            // proposal it is refused like any other audit.
            WorkEdit::RequestAudit => Proposal::Audit {
                source: AuditSource::AuditAgent,
                findings: Vec::new(),
            },
            WorkEdit::Approve => status(Active, &[Draft, Replanning]),
            WorkEdit::Pause => status(Paused, &[]),
            WorkEdit::Resume => status(Active, &[Paused]),
            WorkEdit::Stop => Proposal::StopPlan,
            WorkEdit::Abandon => status(Abandoned, &[]),
            WorkEdit::Reopen => status(Active, &[Done, Abandoned]),
            WorkEdit::RequestReplan => status(Replanning, &[]),
            WorkEdit::RenameTask { task_id, title } => patch(
                task_id,
                TaskPatch {
                    title: Some(title),
                    ..TaskPatch::default()
                },
            ),
            WorkEdit::EditTask { task_id, patch } => Proposal::EditTask { task_id, patch },
            WorkEdit::MoveTask {
                task_id,
                parent,
                index,
            } => Proposal::MoveTask {
                task_id,
                parent,
                index,
            },
            WorkEdit::SetCriteria { task_id, criteria } => {
                Proposal::SetCriteria { task_id, criteria }
            }
            WorkEdit::SetRunAlone { task_id, run_alone } => patch(
                task_id,
                TaskPatch {
                    run_alone: Some(run_alone),
                    ..TaskPatch::default()
                },
            ),
            WorkEdit::HoldTask { task_id, held } => Proposal::HoldTask { task_id, held },
            WorkEdit::ReleaseTask { task_id } => Proposal::HoldTask {
                task_id,
                held: false,
            },
            WorkEdit::RetryTask { task_id } => Proposal::RetryTask { task_id },
            WorkEdit::RetrySerially { task_id } => Proposal::RetrySerial { task_id },
            WorkEdit::RecheckTask { task_id } => Proposal::Recheck { task_id },
            WorkEdit::AcceptTask { task_id } => Proposal::SetTaskStatus {
                task_id,
                status: TaskStatus::Pending,
            },
            WorkEdit::CancelTask { task_id } => Proposal::SetTaskStatus {
                task_id,
                status: TaskStatus::Cancelled,
            },
            WorkEdit::WaiveCriterion {
                task_id,
                criterion_id,
            } => Proposal::SetCriterion {
                task_id,
                criterion_id,
                status: CriterionStatus::Waived,
            },
            WorkEdit::ConfirmCriterion {
                task_id,
                criterion_id,
            } => Proposal::SetCriterion {
                task_id,
                criterion_id,
                status: CriterionStatus::Met,
            },
            WorkEdit::ConfirmEntry { entry_id } => Proposal::SetEntryStatus {
                entry_id,
                status: EntryStatus::Accepted,
            },
            WorkEdit::DismissEntry { entry_id } => Proposal::SetEntryStatus {
                entry_id,
                status: EntryStatus::Dismissed,
            },
        }
    }
}
