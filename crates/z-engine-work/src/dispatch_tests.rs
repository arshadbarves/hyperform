use z_engine_protocol::work::{ParallelPolicy, PlanStatus, TaskOwner, TaskStatus};
use z_engine_protocol::{AgentId, SessionId};

use super::*;
use crate::Proposal;
use crate::testkit::{
    add, created, main_agent, parallel_facts, plan, scoped, set, submit, tid, user,
};

fn helper(id: &str) -> TaskOwner {
    TaskOwner {
        session_id: SessionId::from("S"),
        agent_id: AgentId::from(id),
    }
}

fn approve(state: &mut PlanState) {
    submit(
        state,
        user(),
        vec![Proposal::SetParallel {
            parallel: ParallelPolicy::Approved,
        }],
    );
}

fn apart() -> PlanState {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "a", &[], &["src/a/**"]);
    scoped(&mut state, "b", &[], &["src/b/**"]);
    state
}

#[test]
fn a_ready_task_is_dispatched_as_its_next_attempt() {
    let state = apart();
    let check = check(&state, &tid("a"), None, &parallel_facts(3)).unwrap();
    assert_eq!(
        check,
        DispatchCheck {
            attempt: 1,
            worktree_required: false,
        }
    );
}

#[test]
fn waiting_and_running_tasks_are_not_dispatched() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    let reasons = check(&state, &tid("b"), None, &parallel_facts(3)).unwrap_err();
    assert_eq!(
        reasons,
        [Reason::NotReady {
            task_id: tid("b"),
            detail: "it waits on a".into(),
        }]
    );
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    let reasons = check(&state, &tid("a"), None, &parallel_facts(3)).unwrap_err();
    assert!(matches!(
        &reasons[..],
        [Reason::NotReady { detail, .. }, ..] if detail == "it is active"
    ));
    let missing = check(&state, &tid("zz"), None, &parallel_facts(3)).unwrap_err();
    assert_eq!(missing, [Reason::UnknownTask(tid("zz"))]);
}

#[test]
fn a_task_bound_to_another_helper_is_refused() {
    let mut state = apart();
    submit(
        &mut state,
        main_agent(),
        vec![Proposal::BindTask {
            task_id: tid("a"),
            owner: helper("w1"),
        }],
    );
    let facts = parallel_facts(3);
    assert!(check(&state, &tid("a"), Some(&helper("w1")), &facts).is_ok());
    let reasons = check(&state, &tid("a"), Some(&helper("w2")), &facts).unwrap_err();
    assert_eq!(
        reasons,
        [Reason::Owned {
            task_id: tid("a"),
            owner: "w1".into(),
        }]
    );
}

#[test]
fn beside_a_running_task_the_helper_needs_approval_and_a_worktree() {
    let mut state = apart();
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    let reasons = check(&state, &tid("b"), None, &parallel_facts(3)).unwrap_err();
    assert!(matches!(
        &reasons[..],
        [Reason::Parallel { task_id, detail }]
            if *task_id == tid("b") && detail.contains("parallel work is off")
    ));
    approve(&mut state);
    let check = check(&state, &tid("b"), None, &parallel_facts(3)).unwrap();
    assert_eq!(
        check,
        DispatchCheck {
            attempt: 1,
            worktree_required: true,
        }
    );
}

#[test]
fn the_attempt_budget_counts_only_unforgiven_starts() {
    let mut task = created("a");
    let limits = parallel_facts(3).limits;
    assert_eq!(attempts_left(&task, &limits), Some(3));
    task.attempts = 3;
    assert_eq!(attempts_used(&task), 3);
    assert_eq!(attempts_left(&task, &limits), Some(0));
    assert_eq!(
        budget(&task, &limits),
        Some(Reason::AttemptsUsed {
            task_id: tid("a"),
            attempts: 3,
            max: 3,
        })
    );
    task.run.forgiven_attempts = 3;
    assert_eq!(attempts_used(&task), 0);
    assert_eq!(budget(&task, &limits), None);
    let unlimited = Limits::default();
    task.attempts = 50;
    assert_eq!(attempts_left(&task, &unlimited), None);
    assert_eq!(budget(&task, &unlimited), None);
}

#[test]
fn a_used_up_budget_refuses_the_dispatch() {
    let mut state = apart();
    for _ in 0..3 {
        submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
        submit(
            &mut state,
            main_agent(),
            vec![set("a", TaskStatus::Pending)],
        );
    }
    let reasons = check(&state, &tid("a"), None, &parallel_facts(3)).unwrap_err();
    assert_eq!(
        reasons,
        [Reason::AttemptsUsed {
            task_id: tid("a"),
            attempts: 3,
            max: 3,
        }]
    );
}
