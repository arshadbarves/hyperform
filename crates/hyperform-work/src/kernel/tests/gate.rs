use hyperform_protocol::verification::CheckKind;
use hyperform_protocol::work::{
    Actor, CriterionMethod, CriterionStatus, PlanStatus, TaskStatus, WorkEvent,
};

use crate::testkit::*;
use crate::{Facts, PlanState, Proposal, Reason, TaskDraft};

/// An active plan with task `a` (Active) whose only criterion `c1` needs a
/// passing test check.
fn tested() -> PlanState {
    let mut state = plan(PlanStatus::Active);
    let draft = TaskDraft {
        acceptance: vec![check_criterion("tests pass", CheckKind::Test)],
        ..task("a")
    };
    submit(&mut state, user(), vec![create_task(draft)]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    state
}

fn done_refusal(state: &PlanState, facts: &Facts) -> String {
    refuse_with(state, main_agent(), vec![set("a", TaskStatus::Done)], facts).to_string()
}

#[test]
fn done_needs_a_linked_passing_check() {
    let state = tested();
    assert_eq!(
        done_refusal(&state, &facts()),
        "task a: criterion c1 (tests pass) needs a passing test check on the current tree: no check has been linked"
    );
}

#[test]
fn a_passing_check_on_the_current_tree_completes_the_task() {
    let mut state = tested();
    let records = submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r1", true, FP))],
    );
    assert_eq!(records.len(), 2, "evidence plus the criterion met");
    assert_eq!(records[1].actor, Actor::Kernel);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    assert_eq!(status_of(&state, "a"), TaskStatus::Done);
}

#[test]
fn checks_on_an_older_tree_or_judged_stale_do_not_count() {
    let mut state = tested();
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r1", true, "fp0"))],
    );
    assert!(done_refusal(&state, &facts()).ends_with("the latest check ran on an older tree"));
    let stale = Facts {
        stale_records: ["r1".to_string()].into(),
        ..Facts::at(1_000).with_fingerprint("fp0")
    };
    assert!(done_refusal(&state, &stale).ends_with("the latest check ran on an older tree"));
    let unknown = Facts::at(1_000);
    assert!(done_refusal(&state, &unknown).ends_with("the current tree's fingerprint is unknown"));
}

#[test]
fn the_latest_fresh_check_must_pass() {
    let mut state = tested();
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r1", true, FP))],
    );
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r2", false, FP))],
    );
    let criterion = &state.task(&tid("a")).unwrap().acceptance[0];
    assert_eq!(criterion.status, CriterionStatus::Failed);
    assert!(done_refusal(&state, &facts()).ends_with("the latest check on this tree failed"));
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r3", true, "fp0"))],
    );
    assert!(done_refusal(&state, &facts()).ends_with("the latest check on this tree failed"));
}

#[test]
fn the_gate_marks_a_satisfied_check_criterion_met() {
    let mut state = tested();
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r1", true, FP))],
    );
    let task = state.tasks.get_mut(&tid("a")).unwrap();
    task.acceptance[0].status = CriterionStatus::Pending;
    let records = submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    assert!(records.iter().any(|record| record.actor == Actor::Kernel
        && matches!(
            record.event,
            WorkEvent::CriterionChanged {
                status: CriterionStatus::Met,
                ..
            }
        )));
    assert_eq!(
        state.task(&tid("a")).unwrap().acceptance[0].status,
        CriterionStatus::Met
    );
}

#[test]
fn checks_of_another_kind_do_not_satisfy_a_criterion() {
    let mut state = tested();
    let mut lint = check("r1", true, FP);
    if let hyperform_protocol::work::EvidenceRef::Check { kind, .. } = &mut lint {
        *kind = Some(CheckKind::Lint);
    }
    let records = submit(&mut state, Actor::Kernel, vec![link("a", "c1", lint)]);
    assert_eq!(
        records.len(),
        1,
        "evidence only, the criterion stays pending"
    );
    assert!(done_refusal(&state, &facts()).ends_with("no check has been linked"));
}

#[test]
fn inspection_and_user_criteria_are_met_by_the_right_party() {
    let mut state = plan(PlanStatus::Active);
    let draft = TaskDraft {
        acceptance: vec![
            criterion("reads well", CriterionMethod::Inspection),
            criterion("looks right", CriterionMethod::User),
            check_criterion("tests pass", CheckKind::Test),
        ],
        ..task("a")
    };
    submit(&mut state, user(), vec![create_task(draft)]);
    let meet = |id: &str| Proposal::SetCriterion {
        task_id: tid("a"),
        criterion_id: id.into(),
        status: CriterionStatus::Met,
    };
    submit(&mut state, main_agent(), vec![meet("c1")]);
    assert!(refuse(&state, main_agent(), vec![meet("c2")]).needs_user());
    assert!(matches!(
        refuse(&state, main_agent(), vec![meet("c3")]).reasons[0],
        Reason::EngineOnly(_)
    ));
    submit(&mut state, user(), vec![meet("c2")]);
    let reasons = refuse(&state, main_agent(), vec![set("a", TaskStatus::Done)]).reasons;
    assert_eq!(reasons.len(), 1);
    assert!(matches!(reasons[0], Reason::MissingCheck { .. }));
}

#[test]
fn waived_criteria_are_skipped_and_only_the_user_waives() {
    let mut state = tested();
    let waive = Proposal::SetCriterion {
        task_id: tid("a"),
        criterion_id: "c1".into(),
        status: CriterionStatus::Waived,
    };
    assert!(refuse(&state, main_agent(), vec![waive.clone()]).needs_user());
    submit(&mut state, user(), vec![waive]);
    let back = Proposal::SetCriterion {
        task_id: tid("a"),
        criterion_id: "c1".into(),
        status: CriterionStatus::Pending,
    };
    assert!(refuse(&state, main_agent(), vec![back]).needs_user());
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    assert_eq!(status_of(&state, "a"), TaskStatus::Done);
}

#[test]
fn done_needs_closed_subtasks_and_satisfied_dependencies() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "dep", &[]);
    add(&mut state, "a", &["dep"]);
    let child = TaskDraft {
        parent: Some(tid("a")),
        ..task("a1")
    };
    submit(&mut state, user(), vec![create_task(child)]);
    let reasons = refuse(&state, main_agent(), vec![set("a", TaskStatus::Done)]).reasons;
    assert_eq!(
        reasons,
        [
            Reason::OpenChildren {
                task_id: tid("a"),
                children: vec![tid("a1")],
            },
            Reason::OpenDependencies {
                task_id: tid("a"),
                dependencies: vec![tid("dep")],
            },
        ]
    );
    submit(
        &mut state,
        main_agent(),
        vec![
            set("a1", TaskStatus::Cancelled),
            set("dep", TaskStatus::Done),
        ],
    );
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    assert_eq!(status_of(&state, "a"), TaskStatus::Done);
}
