//! One task of a work plan with its status, scope, notes and acceptance
//! criteria. `Ready` and `Waiting` are derived from `Pending` plus the
//! dependency graph; they are never stored.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AgentId, EntryId, SessionId, TaskId};
use crate::interaction::TodoItem;
use crate::verification::CheckKind;
use crate::work::ledger::EvidenceRef;
use crate::work::plan::Provenance;
use crate::work::run::TaskRun;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TaskKind {
    Research,
    Design,
    #[default]
    Implement,
    Verify,
    Integrate,
    Experiment,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TaskPriority {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum BlockReason {
    /// Set by the kernel when a dependency failed; cleared when it succeeds.
    Dependency {
        task_id: TaskId,
    },
    /// Set by the kernel when a dependency was superseded by `by`; cleared
    /// when the dependency is removed.
    DependencySuperseded {
        task_id: TaskId,
        by: TaskId,
    },
    MergeConflict {
        detail: String,
    },
    /// A bound worker ended without a report.
    Unreported,
    /// The user discarded the helper's worktree change instead of applying it.
    ChangeDiscarded,
    Other {
        text: String,
    },
}

/// Stored task statuses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum TaskStatus {
    /// Suggested (by a worker or an untrusted source); not yet planned.
    Proposed,
    #[default]
    Pending,
    Active,
    Blocked {
        reason: BlockReason,
    },
    Verifying,
    Done,
    Failed {
        #[serde(default)]
        reason: String,
    },
    Cancelled,
    Superseded {
        by: TaskId,
    },
}

impl TaskStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Blocked { .. } => "blocked",
            Self::Verifying => "verifying",
            Self::Done => "done",
            Self::Failed { .. } => "failed",
            Self::Cancelled => "cancelled",
            Self::Superseded { .. } => "superseded",
        }
    }
}

/// Files (globs, root-relative) and symbols a task expects to touch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskScope {
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub symbols: Vec<String>,
}

impl TaskScope {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.symbols.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskNotes {
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub rollback: Option<String>,
}

/// How a criterion is shown to hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum CriterionMethod {
    /// A passing check of this kind on the current tree.
    Check { kind: CheckKind },
    /// An agent inspects and confirms it.
    Inspection,
    /// Only the user confirms it.
    User,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CriterionStatus {
    #[default]
    Pending,
    Met,
    Failed,
    /// Set aside by the user.
    Waived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Criterion {
    /// Unique within its task (`c1`, `c2`, ...).
    pub id: String,
    pub text: String,
    pub method: CriterionMethod,
    pub status: CriterionStatus,
    /// Linked evidence, oldest first.
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
}

/// The agent a task is bound to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskOwner {
    pub session_id: SessionId,
    pub agent_id: AgentId,
}

/// Where an interrupted task can pick up: the agent transcript and the
/// worktree it left behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResumePointer {
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub worktree: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Task {
    pub id: TaskId,
    /// Hierarchy only; dependencies are `depends_on`.
    pub parent: Option<TaskId>,
    pub title: String,
    /// Present-continuous label shown while active ("Writing the parser").
    #[serde(default)]
    pub active_form: String,
    #[serde(default)]
    pub description: String,
    pub kind: TaskKind,
    pub priority: TaskPriority,
    /// Position among its siblings.
    pub order: u32,
    pub status: TaskStatus,
    /// Paused by the user: never Ready while set.
    pub held: bool,
    /// The user asked for it to run alone (never in parallel).
    pub run_alone: bool,
    pub depends_on: Vec<TaskId>,
    /// Requirement ids this task serves.
    pub requirements: Vec<EntryId>,
    pub scope: TaskScope,
    pub notes: TaskNotes,
    pub acceptance: Vec<Criterion>,
    /// The micro-plan (the bound agent's `TodoWrite` list).
    pub steps: Vec<TodoItem>,
    pub owner: Option<TaskOwner>,
    /// What the task produced, shown to its dependents.
    pub result: Option<String>,
    pub resume: Option<ResumePointer>,
    /// Times the task was started.
    pub attempts: u32,
    /// The current run in a parallel helper (dispatch, merge, report).
    #[serde(default)]
    pub run: TaskRun,
    /// Who added it: `Research` when the turn that added it read web content.
    #[serde(default)]
    pub provenance: Provenance,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
    #[ts(type = "number")]
    pub created_revision: u64,
    /// The revision that last changed it.
    #[ts(type = "number")]
    pub revision: u64,
}
