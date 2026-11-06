//! Why the kernel refused a submission, and why a log could not be folded.
//! Every reason names exactly what is missing so the model or the user can
//! act on it.

use hyperform_protocol::work::{PlanStatus, TaskStatus};
use hyperform_protocol::{EntryId, TaskId};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Reason {
    #[error("unknown task {0}")]
    UnknownTask(TaskId),
    #[error("unknown entry {0}")]
    UnknownEntry(EntryId),
    #[error("unknown requirement or invariant {0}")]
    UnknownItem(EntryId),
    #[error("task {task_id} has no criterion {criterion_id}")]
    UnknownCriterion {
        task_id: TaskId,
        criterion_id: String,
    },
    #[error("{0} already exists")]
    DuplicateId(String),
    #[error("{0}")]
    Invalid(String),
    #[error("cannot change task {task_id} from {} to {}", from.label(), to.label())]
    Transition {
        task_id: TaskId,
        from: TaskStatus,
        to: TaskStatus,
    },
    #[error("cannot change the plan from {} to {}", from.label(), to.label())]
    PlanTransition { from: PlanStatus, to: PlanStatus },
    #[error("task {task_id} cannot start: {detail}")]
    NotReady { task_id: TaskId, detail: String },
    #[error("task {task_id} is bound to agent {owner}")]
    Owned { task_id: TaskId, owner: String },
    #[error("dependency cycle: {}", arrows(path))]
    Cycle { path: Vec<TaskId> },
    #[error("task {task_id}: criterion {criterion_id} ({text}) is not met")]
    CriterionUnmet {
        task_id: TaskId,
        criterion_id: String,
        text: String,
    },
    #[error(
        "task {task_id}: criterion {criterion_id} ({text}) needs a passing {kind} check on the current tree: {detail}"
    )]
    MissingCheck {
        task_id: TaskId,
        criterion_id: String,
        text: String,
        kind: &'static str,
        detail: String,
    },
    #[error("task {task_id} has open subtasks: {}", join(children))]
    OpenChildren {
        task_id: TaskId,
        children: Vec<TaskId>,
    },
    #[error(
        "task {task_id} waits on unfinished dependencies: {}",
        join(dependencies)
    )]
    OpenDependencies {
        task_id: TaskId,
        dependencies: Vec<TaskId>,
    },
    #[error("the plan still has open tasks: {}", join(tasks))]
    OpenTasks { tasks: Vec<TaskId> },
    #[error("only the user can {0}")]
    UserOnly(String),
    #[error(
        "this turn read web content, so it cannot {0}; record what you found with PlanRecord (it waits for the user), or ask the user to make the change in the Plan tab"
    )]
    Untrusted(String),
    #[error("a compaction summary may only record discoveries, not {0}")]
    Compaction(String),
    #[error(
        "a worker may only change its bound task{}: {detail}",
        bound_label(bound)
    )]
    WorkerScope {
        bound: Option<TaskId>,
        detail: String,
    },
    #[error("{0} is done by the engine")]
    EngineOnly(String),
    #[error(
        "task {task_id} was already started {attempts} times (the limit is {max}); mark it failed with a reason or ask the user to retry it"
    )]
    AttemptsUsed {
        task_id: TaskId,
        attempts: u32,
        max: u32,
    },
    #[error("task {task_id} cannot run in parallel: {detail}")]
    Parallel { task_id: TaskId, detail: String },
    #[error("unknown audit finding {0}")]
    UnknownFinding(String),
}

impl Reason {
    pub(crate) fn invalid(text: impl Into<String>) -> Self {
        Self::Invalid(text.into())
    }
}

/// A refused submission: nothing was appended.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", self.messages().join("; "))]
pub struct Rejection {
    pub reasons: Vec<Reason>,
}

impl Rejection {
    pub fn new(reasons: Vec<Reason>) -> Self {
        Self { reasons }
    }

    pub fn messages(&self) -> Vec<String> {
        self.reasons.iter().map(ToString::to_string).collect()
    }

    /// Every reason is that only the user may do it: the engine can offer
    /// the proposal to the user as a suggestion instead.
    pub fn needs_user(&self) -> bool {
        !self.reasons.is_empty()
            && self
                .reasons
                .iter()
                .all(|reason| matches!(reason, Reason::UserOnly(_)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FoldError {
    #[error("the plan log is empty")]
    Empty,
    #[error("the plan log does not start with planCreated")]
    NotAPlanLog,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PacketError {
    #[error("unknown task {0}")]
    UnknownTask(TaskId),
}

fn join(ids: &[TaskId]) -> String {
    ids.iter()
        .map(TaskId::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn arrows(ids: &[TaskId]) -> String {
    ids.iter()
        .map(TaskId::as_str)
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn bound_label(bound: &Option<TaskId>) -> String {
    match bound {
        Some(task) => format!(" {task}"),
        None => " (it has none)".to_string(),
    }
}
