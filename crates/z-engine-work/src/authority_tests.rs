use z_engine_protocol::work::{
    Actor, ChangeCause, DesignDecision, Discovery, EntryBody, ParallelPolicy, PlanChange,
    PlanStatus, Provenance, ReconcileFinding, TaskOwner, TaskStatus,
};
use z_engine_protocol::{AgentId, SessionId};

use super::*;
use crate::testkit::*;
use crate::{EntryDraft, Proposal, TaskDraft};

fn discovery() -> Proposal {
    Proposal::Record(Box::new(EntryDraft::new(EntryBody::Discovery(Discovery {
        text: "found".into(),
        source: None,
    }))))
}

fn decision() -> Proposal {
    Proposal::Record(Box::new(EntryDraft::new(EntryBody::DesignDecision(
        DesignDecision {
            title: "t".into(),
            choice: "c".into(),
            alternatives: vec![],
            reason: "r".into(),
        },
    ))))
}

#[test]
fn roles_come_from_the_actor_and_the_facts() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let facts = facts();
    assert_eq!(Role::of(&Actor::User, &state, &facts), Role::User);
    assert_eq!(Role::of(&Actor::Kernel, &state, &facts), Role::Kernel);
    assert_eq!(
        Role::of(&Actor::Compaction, &state, &facts),
        Role::Untrusted(Provenance::Compaction)
    );
    assert_eq!(Role::of(&main_agent(), &state, &facts), Role::Coordinator);
    assert_eq!(
        Role::of(&worker("w1"), &state, &facts),
        Role::Worker { bound: None }
    );
    let research = Facts {
        untrusted_input: true,
        ..facts.clone()
    };
    assert_eq!(
        Role::of(&main_agent(), &state, &research),
        Role::Untrusted(Provenance::Research)
    );
    assert_eq!(
        Role::base(&main_agent(), &state, &research),
        Role::Coordinator
    );
    assert_eq!(Role::of(&Actor::User, &state, &research), Role::User);
    let dispatched = Facts {
        bound_task: Some(tid("a")),
        ..facts.clone()
    };
    assert_eq!(
        Role::of(&worker("w1"), &state, &dispatched),
        Role::Worker {
            bound: Some(tid("a"))
        }
    );
}

#[test]
fn a_worker_is_bound_to_the_task_it_owns() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    state.tasks.get_mut(&tid("a")).unwrap().owner = Some(TaskOwner {
        session_id: SessionId::from("S"),
        agent_id: AgentId::from("w1"),
    });
    assert_eq!(
        Role::of(&worker("w1"), &state, &facts()),
        Role::Worker {
            bound: Some(tid("a"))
        }
    );
}

fn alone(role: &Role, proposal: &Proposal) -> Result<(), Reason> {
    admit(role, role, proposal)
}

#[test]
fn research_adds_and_reports_progress_but_never_steers_the_plan() {
    let role = Role::Untrusted(Provenance::Research);
    let base = Role::Coordinator;
    for allowed in [
        discovery(),
        decision(),
        create_task(task("x")),
        set("a", TaskStatus::Active),
    ] {
        assert_eq!(admit(&role, &base, &allowed), Ok(()), "{allowed:?}");
    }
    let change = Proposal::Record(Box::new(EntryDraft::new(EntryBody::PlanChange(
        PlanChange {
            cause: ChangeCause::WorkerReport,
            source: None,
            original: "x".into(),
            new: "y".into(),
            reason: "z".into(),
            evidence: vec![],
            affected: vec![],
        },
    ))));
    let steering = [
        change,
        Proposal::StopPlan,
        Proposal::SetPlanStatus {
            status: PlanStatus::Paused,
            only_from: vec![],
        },
        Proposal::SetParallel {
            parallel: ParallelPolicy::Approved,
        },
        Proposal::HoldTask {
            task_id: tid("a"),
            held: true,
        },
    ];
    for refused in steering {
        assert!(
            matches!(admit(&role, &base, &refused), Err(Reason::Untrusted(_))),
            "{refused:?}"
        );
    }
}

#[test]
fn a_researching_worker_keeps_its_scope() {
    let role = Role::Untrusted(Provenance::Research);
    let base = Role::Worker {
        bound: Some(tid("a")),
    };
    assert_eq!(admit(&role, &base, &set("a", TaskStatus::Done)), Ok(()));
    assert!(matches!(
        admit(&role, &base, &set("b", TaskStatus::Done)),
        Err(Reason::WorkerScope { .. })
    ));
    assert!(admit(&role, &base, &create_task(task("top"))).is_err());
}

#[test]
fn compaction_may_only_record_discoveries() {
    let role = Role::Untrusted(Provenance::Compaction);
    assert_eq!(alone(&role, &discovery()), Ok(()));
    assert!(matches!(
        alone(&role, &decision()),
        Err(Reason::Compaction(_))
    ));
    assert!(matches!(
        alone(&role, &create_task(task("x"))),
        Err(Reason::Compaction(_))
    ));
}

#[test]
fn workers_change_only_their_own_task() {
    let role = Role::Worker {
        bound: Some(tid("a")),
    };
    assert_eq!(alone(&role, &set("a", TaskStatus::Done)), Ok(()));
    assert!(matches!(
        alone(&role, &set("b", TaskStatus::Done)),
        Err(Reason::WorkerScope { .. })
    ));
    assert_eq!(alone(&role, &discovery()), Ok(()));
    let child = TaskDraft {
        parent: Some(tid("a")),
        ..task("a1")
    };
    assert_eq!(alone(&role, &create_task(child.clone())), Ok(()));
    let pending_child = TaskDraft {
        status: Some(TaskStatus::Pending),
        ..child
    };
    assert!(alone(&role, &create_task(pending_child)).is_err());
    assert!(alone(&role, &create_task(task("top"))).is_err());
    let change = Proposal::Record(Box::new(EntryDraft::new(EntryBody::PlanChange(
        PlanChange {
            cause: ChangeCause::WorkerReport,
            source: None,
            original: "x".into(),
            new: "y".into(),
            reason: "z".into(),
            evidence: vec![],
            affected: vec![],
        },
    ))));
    assert!(alone(&role, &change).is_err());
    assert!(
        alone(
            &role,
            &Proposal::MoveTask {
                task_id: tid("a"),
                parent: None,
                index: 0
            }
        )
        .is_err()
    );
    let unbound = Role::Worker { bound: None };
    assert!(alone(&unbound, &set("a", TaskStatus::Done)).is_err());
}

#[test]
fn engine_work_and_user_decisions_are_reserved() {
    let reconcile = Proposal::Reconcile {
        findings: vec![ReconcileFinding::Duplicate {
            tasks: vec![],
            title: String::new(),
        }],
    };
    assert!(matches!(
        alone(&Role::Coordinator, &reconcile),
        Err(Reason::EngineOnly(_))
    ));
    assert!(matches!(
        alone(&Role::User, &reconcile),
        Err(Reason::EngineOnly(_))
    ));
    assert_eq!(alone(&Role::Kernel, &reconcile), Ok(()));
    let evidence = link("a", "c1", check("r1", true, FP));
    assert!(matches!(
        alone(&Role::Coordinator, &evidence),
        Err(Reason::EngineOnly(_))
    ));
    assert_eq!(alone(&Role::Kernel, &evidence), Ok(()));
    let parallel = Proposal::SetParallel {
        parallel: ParallelPolicy::Approved,
    };
    assert!(matches!(
        alone(&Role::Coordinator, &parallel),
        Err(Reason::UserOnly(_))
    ));
    assert_eq!(alone(&Role::User, &parallel), Ok(()));
    assert!(alone(&Role::Coordinator, &Proposal::StopPlan).is_err());
}
