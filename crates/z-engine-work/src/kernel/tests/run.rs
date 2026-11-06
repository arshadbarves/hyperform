use z_engine_protocol::work::{
    Actor, BlockReason, ParallelPolicy, PlanStatus, ReportDraft, ReportSource, TaskDispatch,
    TaskStatus, WorkEvent,
};
use z_engine_protocol::{AgentId, SessionId};

use crate::testkit::*;
use crate::{Facts, PlanState, Proposal, Reason, RunFacts};

pub(super) fn send(id: &str, agent: &str, worktree: Option<&str>) -> Proposal {
    Proposal::DispatchTask {
        task_id: tid(id),
        dispatch: TaskDispatch {
            session_id: SessionId::from("S"),
            agent_id: AgentId::from(agent),
            worktree: worktree.map(String::from),
            attempt: 0,
            at: 0,
        },
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

/// `a` and `b` touch separate files; parallel work is approved.
fn approved() -> PlanState {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "a", &[], &["src/a/**"]);
    scoped(&mut state, "b", &[], &["src/b/**"]);
    approve(&mut state);
    state
}

/// `a` dispatched to helper `w1` in worktree `/wt/a`.
pub(super) fn running() -> PlanState {
    let mut state = approved();
    submit_with(
        &mut state,
        main_agent(),
        vec![send("a", "w1", Some("/wt/a"))],
        &parallel_facts(3),
    );
    state
}

pub(super) fn bound(id: &str) -> Facts {
    Facts {
        bound_task: Some(tid(id)),
        ..parallel_facts(3)
    }
}

#[test]
fn dispatching_starts_the_task_and_binds_its_helper() {
    let mut state = approved();
    let records = submit_with(
        &mut state,
        main_agent(),
        vec![send("a", "w1", None)],
        &parallel_facts(3),
    );
    assert!(matches!(records[0].event, WorkEvent::TaskDispatched { .. }));
    assert!(matches!(
        records[1].event,
        WorkEvent::TaskTransitioned { .. }
    ));
    assert!(matches!(records[2].event, WorkEvent::TaskBound { .. }));
    let a = state.task(&tid("a")).unwrap();
    assert_eq!(a.status, TaskStatus::Active);
    assert_eq!(a.owner.as_ref().unwrap().agent_id, AgentId::from("w1"));
    assert_eq!(a.attempts, 1);
    let dispatch = a.run.dispatch.as_ref().unwrap();
    assert_eq!(
        (dispatch.attempt, dispatch.at, dispatch.worktree.clone()),
        (1, 1_000, None)
    );
}

#[test]
fn a_second_helper_needs_approval_and_its_own_worktree() {
    let mut state = running();
    let facts = parallel_facts(3);
    for worktree in [None, Some("  ")] {
        let rejection = refuse_with(
            &state,
            main_agent(),
            vec![send("b", "w2", worktree)],
            &facts,
        );
        assert!(
            rejection.to_string().contains("its own worktree"),
            "{rejection}"
        );
    }
    submit_with(
        &mut state,
        main_agent(),
        vec![send("b", "w2", Some("/wt/b"))],
        &facts,
    );
    let b = state.task(&tid("b")).unwrap();
    assert_eq!(
        b.run.dispatch.as_ref().unwrap().worktree.as_deref(),
        Some("/wt/b")
    );

    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "a", &[], &["src/a/**"]);
    scoped(&mut state, "b", &[], &["src/b/**"]);
    submit_with(
        &mut state,
        main_agent(),
        vec![send("a", "w1", None)],
        &facts,
    );
    let rejection = refuse_with(
        &state,
        main_agent(),
        vec![send("b", "w2", Some("/wt"))],
        &facts,
    );
    assert!(rejection.to_string().contains("parallel work is off"));
}

#[test]
fn the_main_agent_is_not_a_helper() {
    let state = approved();
    let mut proposal = send("a", "w1", None);
    if let Proposal::DispatchTask { dispatch, .. } = &mut proposal {
        dispatch.agent_id = AgentId::main();
    }
    let rejection = refuse_with(&state, main_agent(), vec![proposal], &parallel_facts(3));
    assert!(rejection.to_string().contains("not to the main agent"));
}

#[test]
fn a_helper_starting_beside_an_overlapping_task_is_refused() {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "a", &[], &["src/api/**"]);
    scoped(&mut state, "b", &[], &["src/api/client.rs"]);
    approve(&mut state);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    let rejection = refuse_with(
        &state,
        worker("w2"),
        vec![set("b", TaskStatus::Active)],
        &bound("b"),
    );
    assert!(matches!(
        &rejection.reasons[..],
        [Reason::Parallel { task_id, detail }] if *task_id == tid("b") && detail.contains("overlaps task a")
    ));
}

#[test]
fn a_report_keeps_the_observed_files_and_checks_apart_from_claims() {
    let mut state = running();
    let draft = ReportDraft {
        summary: "Added backoff".into(),
        files_changed: vec!["src/a/claimed.rs".into()],
        tests_run: vec!["cargo test".into()],
        ..ReportDraft::default()
    };
    let facts = Facts {
        run: RunFacts {
            files_changed: Some(vec!["src/a/retry.rs".into()]),
            checks: vec![check("chk_1", true, FP)],
        },
        ..bound("a")
    };
    let report = Proposal::Report {
        task_id: tid("a"),
        report: Some(Box::new(draft)),
    };
    submit_with(&mut state, worker("w1"), vec![report], &facts);
    let a = state.task(&tid("a")).unwrap();
    let report = a.run.report.as_ref().unwrap();
    assert_eq!(report.source, ReportSource::Worker);
    assert_eq!((report.attempt, report.diff_known), (1, true));
    assert_eq!(report.files_changed, ["src/a/retry.rs"]);
    assert_eq!(report.claimed.files_changed, ["src/a/claimed.rs"]);
    assert_eq!(report.tests_run, [check("chk_1", true, FP)]);
    assert_eq!(report.claimed.tests_run, ["cargo test"]);
    assert_eq!(a.result.as_deref(), Some("Added backoff"));
    assert_eq!(a.status, TaskStatus::Active);
}

#[test]
fn a_missing_report_blocks_the_task_as_unreported() {
    let mut state = running();
    let missing = || Proposal::Report {
        task_id: tid("a"),
        report: None,
    };
    let rejection = refuse_with(&state, worker("w1"), vec![missing()], &bound("a"));
    assert!(matches!(rejection.reasons[0], Reason::EngineOnly(_)));
    submit_with(
        &mut state,
        Actor::Kernel,
        vec![missing()],
        &parallel_facts(3),
    );
    let a = state.task(&tid("a")).unwrap();
    let unreported = TaskStatus::Blocked {
        reason: BlockReason::Unreported,
    };
    assert_eq!((a.status.clone(), a.owner.clone()), (unreported, None));
    assert_eq!(
        a.run.report.as_ref().unwrap().source,
        ReportSource::Unreported
    );
    let rejection = refuse(&state, Actor::Kernel, vec![missing()]);
    assert!(rejection.to_string().contains("only running work reports"));
}

#[test]
fn agent_retries_stay_within_the_attempt_budget() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let facts = parallel_facts(3);
    let fail = || set("a", TaskStatus::Failed { reason: "x".into() });
    let retry = || Proposal::RetryTask { task_id: tid("a") };
    for _ in 0..2 {
        submit_with(
            &mut state,
            main_agent(),
            vec![set("a", TaskStatus::Active)],
            &facts,
        );
        submit_with(&mut state, main_agent(), vec![fail()], &facts);
        submit_with(&mut state, main_agent(), vec![retry()], &facts);
    }
    submit_with(
        &mut state,
        main_agent(),
        vec![set("a", TaskStatus::Active)],
        &facts,
    );
    submit_with(
        &mut state,
        main_agent(),
        vec![set("a", TaskStatus::Pending)],
        &facts,
    );
    let used = Reason::AttemptsUsed {
        task_id: tid("a"),
        attempts: 3,
        max: 3,
    };
    let start = refuse_with(
        &state,
        main_agent(),
        vec![set("a", TaskStatus::Active)],
        &facts,
    );
    assert_eq!(start.reasons, [used]);
    submit(&mut state, user(), vec![set("a", TaskStatus::Active)]);
    submit(&mut state, user(), vec![fail()]);
    let again = refuse_with(&state, main_agent(), vec![retry()], &facts);
    assert_eq!(
        again.reasons,
        [Reason::AttemptsUsed {
            task_id: tid("a"),
            attempts: 4,
            max: 3,
        }]
    );
    submit_with(&mut state, user(), vec![retry()], &facts);
    assert_eq!(state.task(&tid("a")).unwrap().run.forgiven_attempts, 4);
    submit_with(
        &mut state,
        main_agent(),
        vec![set("a", TaskStatus::Active)],
        &facts,
    );
    assert_eq!(status_of(&state, "a"), TaskStatus::Active);
}
