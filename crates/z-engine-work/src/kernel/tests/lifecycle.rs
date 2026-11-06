use z_engine_protocol::work::{Actor, PlanStatus, TaskOwner, TaskStatus, WorkEvent};
use z_engine_protocol::{AgentId, SessionId};

use crate::testkit::*;
use crate::{Facts, Proposal, Reason, Submission, TaskDraft, decide, rejected};

fn owner(agent: &str) -> TaskOwner {
    TaskOwner {
        session_id: SessionId::from("S"),
        agent_id: if agent == "main" {
            AgentId::main()
        } else {
            AgentId::from(agent)
        },
    }
}

#[test]
fn a_waiting_task_cannot_start() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    let rejection = refuse(&state, main_agent(), vec![set("b", TaskStatus::Active)]);
    assert_eq!(rejection.messages(), ["task b cannot start: it waits on a"]);
    state.header.status = PlanStatus::Draft;
    let rejection = refuse(&state, main_agent(), vec![set("a", TaskStatus::Active)]);
    assert_eq!(
        rejection.messages(),
        ["task a cannot start: the plan is draft"]
    );
}

#[test]
fn starting_binds_the_task_to_the_agent_and_others_are_refused() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let records = submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    assert!(matches!(records[1].event, WorkEvent::TaskBound { .. }));
    assert_eq!(state.task(&tid("a")).unwrap().owner, Some(owner("main")));
    submit(
        &mut state,
        main_agent(),
        vec![set("a", TaskStatus::Pending)],
    );
    let a = state.task(&tid("a")).unwrap();
    assert_eq!((a.owner.clone(), a.attempts), (None, 1));
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    submit(&mut state, user(), vec![set("a", TaskStatus::Verifying)]);
    let dispatched = Facts {
        bound_task: Some(tid("a")),
        ..facts()
    };
    let rejection = refuse_with(
        &state,
        worker("w1"),
        vec![set("a", TaskStatus::Active)],
        &dispatched,
    );
    assert_eq!(
        rejection.reasons,
        [Reason::Owned {
            task_id: tid("a"),
            owner: "main".into()
        }]
    );
}

#[test]
fn a_dispatched_worker_starts_and_finishes_its_own_task() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    let dispatched = Facts {
        bound_task: Some(tid("a")),
        ..facts()
    };
    submit_with(
        &mut state,
        worker("w1"),
        vec![set("a", TaskStatus::Active)],
        &dispatched,
    );
    assert_eq!(state.task(&tid("a")).unwrap().owner, Some(owner("w1")));
    let rejection = refuse_with(
        &state,
        worker("w1"),
        vec![set("b", TaskStatus::Active)],
        &dispatched,
    );
    assert!(matches!(rejection.reasons[0], Reason::WorkerScope { .. }));
    submit(&mut state, worker("w1"), vec![set("a", TaskStatus::Done)]);
    assert_eq!(state.task(&tid("a")).unwrap().owner, None);
}

#[test]
fn a_submission_is_all_or_nothing_and_lists_every_reason() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let before = state.clone();
    let rejection = refuse(
        &state,
        main_agent(),
        vec![
            set("a", TaskStatus::Active),
            set("ghost", TaskStatus::Active),
            set("a", TaskStatus::Proposed),
        ],
    );
    assert_eq!(rejection.reasons.len(), 2);
    assert!(rejection.messages()[0].contains("unknown task ghost"));
    assert!(rejection.messages()[1].contains("from active to proposed"));
    assert_eq!(state, before);
    let audit = rejected(
        &state,
        &Submission::new(main_agent(), cause(), vec![]),
        &rejection,
        &facts(),
    );
    assert_eq!(
        (audit.seq, audit.revision),
        (state.seq + 1, state.revision())
    );
    assert!(matches!(audit.event, WorkEvent::Rejected { ref reasons } if reasons.len() == 2));
    state.apply(&audit);
    assert_eq!(state.revision(), before.revision());
}

#[test]
fn later_proposals_see_earlier_ones() {
    let mut state = plan(PlanStatus::Active);
    let records = submit(
        &mut state,
        main_agent(),
        vec![
            create_task(task("a")),
            create_task(TaskDraft {
                depends_on: vec![tid("a")],
                ..task("b")
            }),
            set("a", TaskStatus::Active),
        ],
    );
    assert_eq!(records.len(), 4);
    assert_eq!(status_of(&state, "a"), TaskStatus::Active);
    assert_eq!(state.task(&tid("b")).unwrap().depends_on, vec![tid("a")]);
}

#[test]
fn workers_propose_tasks_and_the_coordinator_accepts_them() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let dispatched = Facts {
        bound_task: Some(tid("a")),
        ..facts()
    };
    let child = TaskDraft {
        parent: Some(tid("a")),
        ..task("a1")
    };
    submit_with(
        &mut state,
        worker("w1"),
        vec![create_task(child)],
        &dispatched,
    );
    assert_eq!(status_of(&state, "a1"), TaskStatus::Proposed);
    let refused = refuse(&state, main_agent(), vec![set("a1", TaskStatus::Active)]);
    assert!(matches!(refused.reasons[0], Reason::Transition { .. }));
    submit(
        &mut state,
        main_agent(),
        vec![set("a1", TaskStatus::Pending)],
    );
    assert_eq!(status_of(&state, "a1"), TaskStatus::Pending);
}

#[test]
fn retry_moves_failed_or_blocked_work_back_to_pending() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let retry = || Proposal::RetryTask { task_id: tid("a") };
    assert!(matches!(
        refuse(&state, user(), vec![retry()]).reasons[0],
        Reason::Transition { .. }
    ));
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    submit(
        &mut state,
        main_agent(),
        vec![set(
            "a",
            TaskStatus::Failed {
                reason: "tests fail".into(),
            },
        )],
    );
    assert_eq!(state.task(&tid("a")).unwrap().owner, None);
    submit(&mut state, user(), vec![retry()]);
    assert_eq!(status_of(&state, "a"), TaskStatus::Pending);
}

#[test]
fn a_held_task_cannot_start() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let hold = Proposal::HoldTask {
        task_id: tid("a"),
        held: true,
    };
    assert!(refuse(&state, main_agent(), vec![hold.clone()]).needs_user());
    submit(&mut state, user(), vec![hold]);
    let rejection = refuse(&state, main_agent(), vec![set("a", TaskStatus::Active)]);
    assert_eq!(
        rejection.messages(),
        ["task a cannot start: the user holds it"]
    );
}

#[test]
fn stopping_pauses_the_plan_and_releases_every_bound_task() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    submit(
        &mut state,
        user(),
        vec![Proposal::BindTask {
            task_id: tid("b"),
            owner: owner("w2"),
        }],
    );
    submit(&mut state, user(), vec![Proposal::StopPlan]);
    assert_eq!(state.header.status, PlanStatus::Paused);
    assert_eq!(status_of(&state, "a"), TaskStatus::Pending);
    assert!(state.tasks.values().all(|task| task.owner.is_none()));
    assert!(refuse(&state, main_agent(), vec![Proposal::StopPlan]).needs_user());
}

#[test]
fn every_record_of_a_submission_is_stamped_alike() {
    let mut state = plan(PlanStatus::Active);
    let facts = Facts::at(5_000).with_fingerprint(FP);
    let submission = Submission::new(
        main_agent(),
        cause(),
        vec![create_task(task("a")), create_task(task("b"))],
    );
    let records = decide(&state, &submission, &facts).unwrap();
    assert!(records.iter().all(|r| r.at == 5_000 && r.cause == cause()));
    assert!(
        records
            .iter()
            .all(|r| matches!(r.actor, Actor::Agent { .. }))
    );
    state.apply_all(&records);
    assert_eq!(state.header.updated_at, 5_000);
    assert_eq!(state.task(&tid("b")).unwrap().created_at, 5_000);
}
