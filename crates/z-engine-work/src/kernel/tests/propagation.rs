use z_engine_protocol::verification::CheckKind;
use z_engine_protocol::work::{
    Actor, BlockReason, CriterionStatus, PlanStatus, TaskPatch, TaskScope, TaskStatus, WorkEvent,
};

use crate::testkit::*;
use crate::{PlanState, Proposal, TaskDraft};

fn failed() -> TaskStatus {
    TaskStatus::Failed {
        reason: "broken".into(),
    }
}

fn blocked_by(id: &str) -> TaskStatus {
    TaskStatus::Blocked {
        reason: BlockReason::Dependency { task_id: tid(id) },
    }
}

/// `a` is Active; `b` and `c` depend on it.
fn chain() -> PlanState {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    add(&mut state, "c", &["a"]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    state
}

#[test]
fn a_failure_blocks_open_dependents_as_the_kernel() {
    let mut state = chain();
    submit(&mut state, user(), vec![set("c", TaskStatus::Cancelled)]);
    let records = submit(&mut state, main_agent(), vec![set("a", failed())]);
    let kernel: Vec<_> = records
        .iter()
        .filter(|record| record.actor == Actor::Kernel)
        .collect();
    assert_eq!(kernel.len(), 1);
    assert!(matches!(
        &kernel[0].event,
        WorkEvent::TaskTransitioned { task_id, .. } if *task_id == tid("b")
    ));
    assert_eq!(status_of(&state, "b"), blocked_by("a"));
    assert_eq!(status_of(&state, "c"), TaskStatus::Cancelled);
}

#[test]
fn dependents_unblock_once_the_failed_task_is_done() {
    let mut state = chain();
    submit(&mut state, main_agent(), vec![set("a", failed())]);
    submit(
        &mut state,
        user(),
        vec![Proposal::RetryTask { task_id: tid("a") }],
    );
    assert_eq!(
        status_of(&state, "b"),
        blocked_by("a"),
        "retrying is not success"
    );
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    assert_eq!(status_of(&state, "b"), TaskStatus::Pending);
    assert_eq!(status_of(&state, "c"), TaskStatus::Pending);
}

#[test]
fn cancelling_or_dropping_the_failed_dependency_unblocks() {
    let mut state = chain();
    submit(&mut state, main_agent(), vec![set("a", failed())]);
    submit(
        &mut state,
        main_agent(),
        vec![Proposal::RemoveDependency {
            task_id: tid("b"),
            depends_on: tid("a"),
        }],
    );
    assert_eq!(status_of(&state, "b"), TaskStatus::Pending);
    assert_eq!(status_of(&state, "c"), blocked_by("a"));
    submit(&mut state, user(), vec![set("a", TaskStatus::Cancelled)]);
    assert_eq!(status_of(&state, "c"), TaskStatus::Pending);
}

#[test]
fn another_failed_dependency_keeps_the_task_blocked() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "x", &[]);
    add(&mut state, "b", &["a", "x"]);
    submit(
        &mut state,
        main_agent(),
        vec![set("a", TaskStatus::Active), set("x", TaskStatus::Active)],
    );
    submit(&mut state, main_agent(), vec![set("a", failed())]);
    submit(&mut state, main_agent(), vec![set("x", failed())]);
    assert_eq!(status_of(&state, "b"), blocked_by("a"));
    submit(&mut state, user(), vec![set("a", TaskStatus::Cancelled)]);
    assert_eq!(status_of(&state, "b"), blocked_by("x"));
}

#[test]
fn editing_the_scope_of_a_done_task_reopens_it() {
    let mut state = chain();
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    let patch = TaskPatch {
        scope: Some(TaskScope {
            files: vec!["src/new.rs".into()],
            symbols: vec![],
        }),
        ..TaskPatch::default()
    };
    let records = submit(
        &mut state,
        main_agent(),
        vec![Proposal::EditTask {
            task_id: tid("a"),
            patch: Box::new(patch),
        }],
    );
    assert_eq!(records.last().unwrap().actor, Actor::Kernel);
    assert_eq!(status_of(&state, "a"), TaskStatus::Pending);
    let rename = TaskPatch {
        title: Some("Renamed".into()),
        ..TaskPatch::default()
    };
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    submit(
        &mut state,
        user(),
        vec![Proposal::EditTask {
            task_id: tid("a"),
            patch: Box::new(rename),
        }],
    );
    assert_eq!(
        status_of(&state, "a"),
        TaskStatus::Done,
        "a rename keeps it done"
    );
}

#[test]
fn changing_or_weakening_criteria_reopens_a_done_task() {
    let mut state = plan(PlanStatus::Active);
    let draft = TaskDraft {
        acceptance: vec![check_criterion("tests pass", CheckKind::Test)],
        ..task("a")
    };
    submit(&mut state, user(), vec![create_task(draft)]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r1", true, FP))],
    );
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r2", false, FP))],
    );
    assert_eq!(
        status_of(&state, "a"),
        TaskStatus::Done,
        "evidence alone does not reopen"
    );
    let weaken = Proposal::SetCriterion {
        task_id: tid("a"),
        criterion_id: "c1".into(),
        status: CriterionStatus::Pending,
    };
    submit(&mut state, main_agent(), vec![weaken]);
    assert_eq!(status_of(&state, "a"), TaskStatus::Pending);

    let mut state = plan(PlanStatus::Active);
    add(&mut state, "b", &[]);
    submit(&mut state, main_agent(), vec![set("b", TaskStatus::Active)]);
    submit(&mut state, main_agent(), vec![set("b", TaskStatus::Done)]);
    let records = submit(
        &mut state,
        user(),
        vec![Proposal::SetCriteria {
            task_id: tid("b"),
            criteria: vec![check_criterion("builds", CheckKind::Build)],
        }],
    );
    assert_eq!(status_of(&state, "b"), TaskStatus::Pending);
    assert!(records.iter().any(|r| r.actor == Actor::Kernel));
}

#[test]
fn a_superseded_dependency_blocks_until_it_is_rewired() {
    let mut state = chain();
    add(&mut state, "n", &[]);
    let superseded = TaskStatus::Superseded { by: tid("n") };
    submit(&mut state, user(), vec![set("a", superseded)]);
    let waiting = TaskStatus::Blocked {
        reason: BlockReason::DependencySuperseded {
            task_id: tid("a"),
            by: tid("n"),
        },
    };
    assert_eq!(status_of(&state, "b"), waiting);
    assert_eq!(status_of(&state, "c"), waiting);
    submit(
        &mut state,
        main_agent(),
        vec![
            Proposal::RemoveDependency {
                task_id: tid("b"),
                depends_on: tid("a"),
            },
            Proposal::AddDependency {
                task_id: tid("b"),
                depends_on: tid("n"),
            },
        ],
    );
    assert_eq!(status_of(&state, "b"), TaskStatus::Pending);
    assert_eq!(status_of(&state, "c"), waiting);
}

#[test]
fn a_superseded_dependency_still_blocks_once_another_is_dropped() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "s", &[]);
    add(&mut state, "n", &[]);
    add(&mut state, "b", &["a", "s"]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    submit(&mut state, main_agent(), vec![set("a", failed())]);
    let superseded = TaskStatus::Superseded { by: tid("n") };
    submit(&mut state, user(), vec![set("s", superseded)]);
    assert_eq!(status_of(&state, "b"), blocked_by("a"));
    submit(&mut state, user(), vec![set("a", TaskStatus::Cancelled)]);
    assert!(matches!(
        status_of(&state, "b"),
        TaskStatus::Blocked {
            reason: BlockReason::DependencySuperseded { .. }
        }
    ));
}
