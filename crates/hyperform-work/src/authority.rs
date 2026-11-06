//! Who may propose what. The actor kind decides the role; `admit` is the
//! coarse gate per proposal, and the kernel's operations add the checks
//! that depend on the target (who created an item, a criterion's method).

use hyperform_protocol::TaskId;
use hyperform_protocol::work::{Actor, EntryBody, EvidenceRef, Provenance, TaskOwner, TaskStatus};

use crate::error::Reason;
use crate::facts::Facts;
use crate::proposal::Proposal;
use crate::state::PlanState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    User,
    /// The engine itself (propagation, reconciliation, linked checks).
    Kernel,
    /// The main agent.
    Coordinator,
    /// A subagent: it may change only its bound task.
    Worker {
        bound: Option<TaskId>,
    },
    /// An agent whose turn read web content (`Research`), or compaction
    /// extraction.
    Untrusted(Provenance),
}

impl Role {
    pub fn of(actor: &Actor, state: &PlanState, facts: &Facts) -> Self {
        let base = Self::base(actor, state, facts);
        let agent = matches!(base, Self::Coordinator | Self::Worker { .. });
        if agent && facts.untrusted_input {
            Self::Untrusted(Provenance::Research)
        } else {
            base
        }
    }

    /// The role ignoring web content: a researching worker still may
    /// change only its bound task.
    pub fn base(actor: &Actor, state: &PlanState, facts: &Facts) -> Self {
        match actor {
            Actor::User => Self::User,
            Actor::Kernel => Self::Kernel,
            Actor::Compaction => Self::Untrusted(Provenance::Compaction),
            Actor::Agent { agent_id, .. } if agent_id.is_main() => Self::Coordinator,
            Actor::Agent {
                session_id,
                agent_id,
            } => {
                let owner = TaskOwner {
                    session_id: session_id.clone(),
                    agent_id: agent_id.clone(),
                };
                let bound = facts.bound_task.clone().or_else(|| {
                    state
                        .tasks
                        .values()
                        .find(|task| task.owner.as_ref() == Some(&owner))
                        .map(|task| task.id.clone())
                });
                Self::Worker { bound }
            }
        }
    }

    pub fn is_user(&self) -> bool {
        matches!(self, Self::User)
    }

    pub fn is_research(&self) -> bool {
        matches!(self, Self::Untrusted(Provenance::Research))
    }

    /// Provenance of what this role creates.
    pub fn provenance(&self) -> Provenance {
        match self {
            Self::User => Provenance::User,
            Self::Kernel | Self::Coordinator | Self::Worker { .. } => Provenance::Agent,
            Self::Untrusted(provenance) => *provenance,
        }
    }

    /// `UserOnly(what)` unless this is the user.
    pub fn require_user(&self, what: &str) -> Result<(), Reason> {
        if self.is_user() {
            Ok(())
        } else {
            Err(Reason::UserOnly(what.to_string()))
        }
    }
}

/// The coarse gate: may `role` make this kind of proposal at all? A
/// researching agent passes both the research gate and its `base` role's.
pub fn admit(role: &Role, base: &Role, proposal: &Proposal) -> Result<(), Reason> {
    match role {
        Role::Untrusted(Provenance::Research) => {
            admit_research(proposal)?;
            match base {
                Role::Untrusted(_) => Ok(()),
                _ => admit(base, base, proposal),
            }
        }
        Role::Untrusted(_) => admit_compaction(proposal),
        Role::Worker { bound } => admit_worker(bound.as_ref(), proposal),
        Role::User | Role::Kernel | Role::Coordinator => admit_trusted(role, proposal),
    }
}

fn admit_trusted(role: &Role, proposal: &Proposal) -> Result<(), Reason> {
    match proposal {
        Proposal::Reconcile { .. } | Proposal::LinkCheckpoint { .. } if *role != Role::Kernel => {
            Err(Reason::EngineOnly(label(proposal).into()))
        }
        Proposal::LinkEvidence {
            evidence: EvidenceRef::Check { .. },
            ..
        } if *role != Role::Kernel => Err(Reason::EngineOnly("linking check evidence".into())),
        Proposal::SetDepth { .. }
        | Proposal::InterruptTask { .. }
        | Proposal::DiscardResumeWorktree { .. }
            if *role != Role::Kernel =>
        {
            Err(Reason::EngineOnly(label(proposal).into()))
        }
        Proposal::SetParallel { .. }
        | Proposal::StopPlan
        | Proposal::HoldTask { .. }
        | Proposal::Recheck { .. } => role.require_user(label(proposal)),
        _ => Ok(()),
    }
}

/// A turn that read web content may add what waits for the user (proposed
/// tasks and items, unconfirmed entries) and report progress (including
/// dispatching an accepted task and a helper's report, whose lists are
/// marked `Research`); the kernel's research checks refuse edits to what
/// the plan already says. Integration, audits and findings stay with the
/// kernel and the user, whose gates `base` keeps.
fn admit_research(proposal: &Proposal) -> Result<(), Reason> {
    match proposal {
        Proposal::CreateTask(_)
        | Proposal::AddItem { .. }
        | Proposal::EditPlan { .. }
        | Proposal::EditTask { .. }
        | Proposal::MoveTask { .. }
        | Proposal::SetCriteria { .. }
        | Proposal::AddDependency { .. }
        | Proposal::RemoveDependency { .. }
        | Proposal::SetTaskStatus { .. }
        | Proposal::RetryTask { .. }
        | Proposal::RetrySerial { .. }
        | Proposal::DispatchTask { .. }
        | Proposal::Report { .. }
        | Proposal::BindTask { .. }
        | Proposal::ReleaseTask { .. }
        | Proposal::RecordSteps { .. }
        | Proposal::SetCriterion { .. }
        | Proposal::LinkEvidence { .. }
        | Proposal::ConcludeExperiment { .. } => Ok(()),
        Proposal::Record(draft) if !matches!(draft.body, EntryBody::PlanChange(_)) => Ok(()),
        Proposal::Record(_) => Err(Reason::Untrusted("record a plan change".into())),
        Proposal::Reconcile { .. }
        | Proposal::LinkCheckpoint { .. }
        | Proposal::Integrate { .. }
        | Proposal::Audit { .. }
        | Proposal::SetDepth { .. }
        | Proposal::InterruptTask { .. }
        | Proposal::DiscardResumeWorktree { .. } => Err(Reason::EngineOnly(label(proposal).into())),
        Proposal::SetFindingStatus { .. } | Proposal::Recheck { .. } => {
            Err(Reason::UserOnly(label(proposal).into()))
        }
        other => Err(Reason::Untrusted(label(other).into())),
    }
}

fn admit_compaction(proposal: &Proposal) -> Result<(), Reason> {
    match proposal {
        Proposal::Record(draft) if matches!(draft.body, EntryBody::Discovery(_)) => Ok(()),
        Proposal::Record(draft) => Err(Reason::Compaction(draft.body.label().into())),
        other => Err(Reason::Compaction(label(other).into())),
    }
}

fn admit_worker(bound: Option<&TaskId>, proposal: &Proposal) -> Result<(), Reason> {
    let scope = |detail: &str| Reason::WorkerScope {
        bound: bound.cloned(),
        detail: detail.to_string(),
    };
    let own = |task_id: &TaskId| {
        if bound == Some(task_id) {
            Ok(())
        } else {
            Err(scope(&format!("task {task_id} is not its own")))
        }
    };
    match proposal {
        Proposal::CreateTask(draft) => {
            if draft.parent.as_ref() != bound || bound.is_none() {
                return Err(scope("new tasks must be children of its task"));
            }
            match &draft.status {
                None | Some(TaskStatus::Proposed) => Ok(()),
                Some(_) => Err(scope("new tasks must be proposed")),
            }
        }
        Proposal::LinkEvidence {
            evidence: EvidenceRef::Check { .. },
            ..
        } => Err(Reason::EngineOnly("linking check evidence".into())),
        Proposal::EditTask { task_id, .. }
        | Proposal::SetTaskStatus { task_id, .. }
        | Proposal::RecordSteps { task_id, .. }
        | Proposal::SetCriterion { task_id, .. }
        | Proposal::LinkEvidence { task_id, .. }
        | Proposal::AddDependency { task_id, .. }
        | Proposal::RemoveDependency { task_id, .. }
        | Proposal::Report { task_id, .. } => own(task_id),
        Proposal::Record(draft) => match &draft.body {
            EntryBody::PlanChange(_) => Err(scope("plan changes belong to the coordinator")),
            EntryBody::Evidence(evidence) => own(&evidence.task_id),
            _ => Ok(()),
        },
        Proposal::ConcludeExperiment { .. } => Ok(()),
        other => Err(scope(label(other))),
    }
}

/// A short description of a proposal for rejection messages.
pub fn label(proposal: &Proposal) -> &'static str {
    match proposal {
        Proposal::EditPlan { .. } => "edit the plan",
        Proposal::SetPlanStatus { .. } => "change the plan status",
        Proposal::StopPlan => "stop the plan",
        Proposal::SetParallel { .. } => "change the parallel policy",
        Proposal::SetDepth { .. } => "change the plan depth",
        Proposal::AddItem { .. } => "add a requirement or invariant",
        Proposal::EditItem { .. } => "reword a requirement or invariant",
        Proposal::SetItemStatus { .. } => "accept or retire a requirement or invariant",
        Proposal::CreateTask(_) => "create a task",
        Proposal::EditTask { .. } => "edit a task",
        Proposal::MoveTask { .. } => "move a task",
        Proposal::SetTaskStatus { .. } => "change a task status",
        Proposal::HoldTask { .. } => "hold or resume a task",
        Proposal::RetryTask { .. } => "retry a task",
        Proposal::Recheck { .. } => "run a task's checks again",
        Proposal::BindTask { .. } => "bind a task",
        Proposal::ReleaseTask { .. } => "release a task",
        Proposal::AddDependency { .. } => "add a dependency",
        Proposal::RemoveDependency { .. } => "remove a dependency",
        Proposal::RecordSteps { .. } => "record steps",
        Proposal::SetCriteria { .. } => "edit criteria",
        Proposal::SetCriterion { .. } => "change a criterion",
        Proposal::LinkEvidence { .. } => "link evidence",
        Proposal::Record(_) => "record a ledger entry",
        Proposal::SetEntryStatus { .. } => "confirm or dismiss an entry",
        Proposal::ConcludeExperiment { .. } => "conclude an experiment",
        Proposal::Reconcile { .. } => "reconciliation",
        Proposal::LinkCheckpoint { .. } => "linking a checkpoint",
        Proposal::DispatchTask { .. } => "dispatch a task to a helper",
        Proposal::Report { .. } => "report a task's result",
        Proposal::Integrate { .. } => "integrating a helper's change",
        Proposal::RetrySerial { .. } => "retry a task alone",
        Proposal::InterruptTask { .. } => "interrupting a task's helper",
        Proposal::DiscardResumeWorktree { .. } => "dropping a resume pointer's worktree",
        Proposal::Audit { .. } => "recording audit findings",
        Proposal::SetFindingStatus { .. } => "add or dismiss an audit finding",
    }
}

#[cfg(test)]
#[path = "authority_tests.rs"]
mod tests;
