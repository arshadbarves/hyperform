use z_engine_protocol::verification::CheckKind;
use z_engine_protocol::work::{
    CriterionDraft, CriterionMethod, CriterionStatus, PlanStatus, TaskOwner, TaskPatch, TaskStatus,
    WorkEvent,
};
use z_engine_protocol::{AgentId, SessionId, TodoItem, TodoStatus};

use crate::testkit::*;
use crate::{PlanState, Proposal, Reason, TaskDraft};

#[test]
fn dependency_cycles_are_refused_with_the_path() {
    let mut state = plan(PlanStatus::Draft);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    add(&mut state, "c", &["b"]);
    let rejection = refuse(
        &state,
        main_agent(),
        vec![Proposal::AddDependency {
            task_id: tid("a"),
            depends_on: tid("c"),
        }],
    );
    assert_eq!(rejection.to_string(), "dependency cycle: a -> c -> b -> a");
    let records = submit(
        &mut state,
        main_agent(),
        vec![Proposal::AddDependency {
            task_id: tid("c"),
            depends_on: tid("a"),
        }],
    );
    assert_eq!(records.len(), 1);
    let again = Proposal::AddDependency {
        task_id: tid("c"),
        depends_on: tid("a"),
    };
    assert!(submit(&mut state, main_agent(), vec![again]).is_empty());
}

#[test]
fn new_tasks_must_be_well_formed() {
    let mut state = plan(PlanStatus::Draft);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    let bad = TaskDraft {
        title: "  ".into(),
        parent: Some(tid("ghost")),
        depends_on: vec![tid("b"), tid("nope"), tid("b")],
        status: Some(TaskStatus::Active),
        ..task("a")
    };
    let messages = refuse(&state, main_agent(), vec![create_task(bad)]).messages();
    assert_eq!(
        messages,
        [
            "a task needs a title",
            "a already exists",
            "unknown task ghost",
            "unknown task nope",
            "a new task starts proposed or pending",
        ]
    );
    let selfish = TaskDraft {
        depends_on: vec![tid("s")],
        ..task("s")
    };
    assert!(matches!(
        refuse(&state, main_agent(), vec![create_task(selfish)]).reasons[0],
        Reason::Cycle { .. }
    ));
    let generated = TaskDraft::new("Generated id");
    submit(&mut state, main_agent(), vec![create_task(generated)]);
    assert!(state.tasks.keys().any(|id| id.as_str().starts_with("tsk_")));
}

#[test]
fn a_task_cannot_move_under_itself() {
    let mut state = plan(PlanStatus::Draft);
    add(&mut state, "a", &[]);
    let child = TaskDraft {
        parent: Some(tid("a")),
        ..task("a1")
    };
    submit(&mut state, user(), vec![create_task(child)]);
    let move_to = |parent: &str| Proposal::MoveTask {
        task_id: tid("a"),
        parent: Some(tid(parent)),
        index: 0,
    };
    for parent in ["a", "a1"] {
        assert!(
            refuse(&state, user(), vec![move_to(parent)])
                .to_string()
                .contains("under itself")
        );
    }
    let stay = Proposal::MoveTask {
        task_id: tid("a1"),
        parent: Some(tid("a")),
        index: 0,
    };
    assert!(submit(&mut state, user(), vec![stay]).is_empty());
}

#[test]
fn criteria_keep_their_ids_status_and_evidence() {
    let mut state = plan(PlanStatus::Active);
    let draft = TaskDraft {
        acceptance: vec![
            check_criterion("tests pass", CheckKind::Test),
            criterion("reads well", CriterionMethod::Inspection),
        ],
        ..task("a")
    };
    submit(&mut state, user(), vec![create_task(draft)]);
    submit(
        &mut state,
        z_engine_protocol::work::Actor::Kernel,
        vec![link("a", "c1", check("r1", true, FP))],
    );
    let keep = |id: &str, text: &str, method| CriterionDraft {
        id: Some(id.into()),
        text: text.into(),
        method,
    };
    let edit = Proposal::SetCriteria {
        task_id: tid("a"),
        criteria: vec![
            keep(
                "c1",
                "all tests pass",
                CriterionMethod::Check {
                    kind: CheckKind::Test,
                },
            ),
            criterion("docs updated", CriterionMethod::User),
        ],
    };
    submit(&mut state, main_agent(), vec![edit]);
    let acceptance = &state.task(&tid("a")).unwrap().acceptance;
    assert_eq!(acceptance.len(), 2);
    assert_eq!(
        (acceptance[0].id.as_str(), acceptance[0].status),
        ("c1", CriterionStatus::Met)
    );
    assert_eq!(acceptance[0].text, "all tests pass");
    assert_eq!(acceptance[0].evidence.len(), 1);
    assert_eq!(acceptance[1].id, "c3", "ids are never reused");
    let switch = Proposal::SetCriteria {
        task_id: tid("a"),
        criteria: vec![keep(
            "c1",
            "builds",
            CriterionMethod::Check {
                kind: CheckKind::Build,
            },
        )],
    };
    submit(&mut state, main_agent(), vec![switch]);
    assert_eq!(
        state.task(&tid("a")).unwrap().acceptance[0].status,
        CriterionStatus::Pending
    );
    let unknown = Proposal::SetCriteria {
        task_id: tid("a"),
        criteria: vec![keep("c7", "x", CriterionMethod::Inspection)],
    };
    assert!(matches!(
        refuse(&state, main_agent(), vec![unknown]).reasons[0],
        Reason::UnknownCriterion { .. }
    ));
}

#[test]
fn edits_emit_only_what_changed() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let patch = TaskPatch {
        title: Some(" Task a ".into()),
        description: Some("details".into()),
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
    let WorkEvent::TaskEdited { patch, .. } = &records[0].event else {
        panic!("edit");
    };
    assert_eq!(
        (patch.title.clone(), patch.description.as_deref()),
        (None, Some("details"))
    );
    let criteria = TaskPatch {
        acceptance: Some(vec![]),
        ..TaskPatch::default()
    };
    assert!(
        refuse(
            &state,
            main_agent(),
            vec![Proposal::EditTask {
                task_id: tid("a"),
                patch: Box::new(criteria),
            }]
        )
        .to_string()
        .contains("SetCriteria")
    );
    let steps = vec![TodoItem {
        content: "write it".into(),
        status: TodoStatus::Pending,
        active_form: "Writing it".into(),
    }];
    let record_steps = || Proposal::RecordSteps {
        task_id: tid("a"),
        steps: steps.clone(),
    };
    assert_eq!(
        submit(&mut state, main_agent(), vec![record_steps()]).len(),
        1
    );
    assert!(submit(&mut state, main_agent(), vec![record_steps()]).is_empty());
}

#[test]
fn binding_respects_the_current_owner() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let owner = |agent: &str| TaskOwner {
        session_id: SessionId::from("S"),
        agent_id: AgentId::from(agent),
    };
    let bind = |agent: &str| Proposal::BindTask {
        task_id: tid("a"),
        owner: owner(agent),
    };
    submit(&mut state, main_agent(), vec![bind("w1")]);
    assert!(submit(&mut state, main_agent(), vec![bind("w1")]).is_empty());
    assert_eq!(
        refuse(&state, main_agent(), vec![bind("w2")]).reasons,
        [Reason::Owned {
            task_id: tid("a"),
            owner: "w1".into(),
        }]
    );
    submit(
        &mut state,
        main_agent(),
        vec![Proposal::ReleaseTask { task_id: tid("a") }],
    );
    assert_eq!(state.task(&tid("a")).unwrap().owner, None);
    submit(&mut state, user(), vec![set("a", TaskStatus::Cancelled)]);
    assert!(
        refuse(&state, main_agent(), vec![bind("w2")])
            .to_string()
            .contains("cannot be bound")
    );
}

#[test]
fn superseding_names_an_existing_other_task() {
    let mut state: PlanState = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    let by = |id: &str| set("a", TaskStatus::Superseded { by: tid(id) });
    assert!(
        refuse(&state, main_agent(), vec![by("a")])
            .to_string()
            .contains("itself")
    );
    assert_eq!(
        refuse(&state, main_agent(), vec![by("z")]).reasons,
        [Reason::UnknownTask(tid("z"))]
    );
    submit(&mut state, main_agent(), vec![by("b")]);
    assert!(matches!(
        status_of(&state, "a"),
        TaskStatus::Superseded { .. }
    ));
}
