//! Builders shared by the unit tests.

use hyperform_protocol::verification::CheckKind;
use hyperform_protocol::work::{
    Actor, CriterionDraft, CriterionMethod, EvidenceRef, PlanDepth, PlanStatus, ReconcileFinding,
    Task, TaskScope, TaskStatus, WorkCause, WorkRecord,
};
use hyperform_protocol::{AgentId, PlanId, SessionId, TaskId};

use crate::{
    Facts, Limits, PlanDraft, PlanState, Proposal, Rejection, Submission, TaskDraft, create, decide,
};

pub const FP: &str = "fp1";

pub fn facts() -> Facts {
    Facts::at(1_000).with_fingerprint(FP)
}

pub fn user() -> Actor {
    Actor::User
}

pub fn main_agent() -> Actor {
    Actor::Agent {
        session_id: SessionId::from("S"),
        agent_id: AgentId::main(),
    }
}

pub fn worker(id: &str) -> Actor {
    Actor::Agent {
        session_id: SessionId::from("S"),
        agent_id: AgentId::from(id),
    }
}

pub fn tid(id: &str) -> TaskId {
    TaskId::from(id)
}

pub fn cause() -> WorkCause {
    WorkCause::Tool {
        tool: "Test".into(),
        call_id: None,
    }
}

/// A plan created by the user; statuses other than Draft and Active are
/// set directly.
pub fn plan(status: PlanStatus) -> PlanState {
    let draft = PlanDraft {
        plan_id: PlanId::from("pln_t"),
        project_root: "/p".into(),
        goal: "Ship it".into(),
        document: String::new(),
        depth: PlanDepth::Standard,
        status: if status == PlanStatus::Draft {
            PlanStatus::Draft
        } else {
            PlanStatus::Active
        },
    };
    let submission = Submission::new(user(), WorkCause::Approval, vec![]);
    let records = create(&draft, &submission, &facts()).expect("plan created");
    let mut state = PlanState::fold(&records).expect("fold");
    state.header.status = status;
    state
}

pub fn submit(state: &mut PlanState, actor: Actor, proposals: Vec<Proposal>) -> Vec<WorkRecord> {
    submit_with(state, actor, proposals, &facts())
}

pub fn submit_with(
    state: &mut PlanState,
    actor: Actor,
    proposals: Vec<Proposal>,
    facts: &Facts,
) -> Vec<WorkRecord> {
    let submission = Submission::new(actor, cause(), proposals);
    let records =
        decide(state, &submission, facts).unwrap_or_else(|rejection| panic!("{rejection}"));
    state.apply_all(&records);
    records
}

pub fn refuse(state: &PlanState, actor: Actor, proposals: Vec<Proposal>) -> Rejection {
    refuse_with(state, actor, proposals, &facts())
}

pub fn refuse_with(
    state: &PlanState,
    actor: Actor,
    proposals: Vec<Proposal>,
    facts: &Facts,
) -> Rejection {
    let submission = Submission::new(actor, cause(), proposals);
    match decide(state, &submission, facts) {
        Ok(records) => panic!("accepted: {records:?}"),
        Err(rejection) => rejection,
    }
}

pub fn task(id: &str) -> TaskDraft {
    TaskDraft {
        id: Some(tid(id)),
        ..TaskDraft::new(format!("Task {id}"))
    }
}

pub fn create_task(draft: TaskDraft) -> Proposal {
    Proposal::CreateTask(Box::new(draft))
}

/// Add a pending task (by the user) depending on `deps`.
pub fn add(state: &mut PlanState, id: &str, deps: &[&str]) {
    let draft = TaskDraft {
        depends_on: deps.iter().map(|dep| tid(dep)).collect(),
        ..task(id)
    };
    submit(state, user(), vec![create_task(draft)]);
}

pub fn status_of(state: &PlanState, id: &str) -> TaskStatus {
    state.task(&tid(id)).expect("task").status.clone()
}

pub fn set(id: &str, status: TaskStatus) -> Proposal {
    Proposal::SetTaskStatus {
        task_id: tid(id),
        status,
    }
}

pub fn check_criterion(text: &str, kind: CheckKind) -> CriterionDraft {
    CriterionDraft {
        id: None,
        text: text.into(),
        method: CriterionMethod::Check { kind },
    }
}

pub fn criterion(text: &str, method: CriterionMethod) -> CriterionDraft {
    CriterionDraft {
        id: None,
        text: text.into(),
        method,
    }
}

pub fn check(record: &str, passed: bool, fingerprint: &str) -> EvidenceRef {
    EvidenceRef::Check {
        session_id: SessionId::from("S"),
        record_id: record.into(),
        passed,
        fingerprint: fingerprint.into(),
        kind: Some(CheckKind::Test),
        label: "cargo test".into(),
        at: 900,
    }
}

pub fn link(id: &str, criterion: &str, evidence: EvidenceRef) -> Proposal {
    Proposal::LinkEvidence {
        task_id: tid(id),
        criterion_id: Some(criterion.into()),
        evidence,
    }
}

/// A pending task as the kernel creates it.
pub fn created(id: &str) -> Task {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, id, &[]);
    state.task(&tid(id)).expect("task").clone()
}

/// Add a pending task (by the user) with declared files.
pub fn scoped(state: &mut PlanState, id: &str, deps: &[&str], files: &[&str]) {
    let draft = TaskDraft {
        depends_on: deps.iter().map(|dep| tid(dep)).collect(),
        scope: TaskScope {
            files: files.iter().map(|file| file.to_string()).collect(),
            symbols: Vec::new(),
        },
        ..task(id)
    };
    submit(state, user(), vec![create_task(draft)]);
}

/// An Active plan with `a`, `b` after `a`, and `c`, then the kernel
/// records `findings` (moving it to Replanning when there are any).
pub fn reconciled(findings: Vec<ReconcileFinding>) -> PlanState {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    add(&mut state, "c", &[]);
    submit(
        &mut state,
        Actor::Kernel,
        vec![Proposal::Reconcile { findings }],
    );
    state
}

/// Files in `task`'s scope were deleted.
pub fn deleted(task: &str) -> ReconcileFinding {
    ReconcileFinding::ScopeDeleted {
        task_id: tid(task),
        files: vec!["src/gone.rs".into()],
    }
}

/// `facts()` with room for `max` parallel helpers and `max_attempts`.
pub fn parallel_facts(max: u32) -> Facts {
    Facts {
        limits: Limits {
            max_parallel: max,
            max_attempts: Some(3),
            serial_paths: vec!["**/Cargo.lock".into(), "**/Cargo.toml".into()],
        },
        ..facts()
    }
}
