use z_engine_protocol::EntryId;
use z_engine_protocol::work::{
    CriterionMethod, EntryBody, EntryStatus, EvidenceRef, Experiment, ExperimentOutcome,
    PlanItemKind, PlanItemStatus, PlanStatus, Provenance, TaskPatch, TaskScope, TaskStatus,
    WorkEdit,
};
use z_engine_protocol::{TodoItem, TodoStatus};

use crate::testkit::*;
use crate::{EntryDraft, Facts, PlanState, Proposal, Reason, TaskDraft};

fn research() -> Facts {
    Facts {
        untrusted_input: true,
        ..facts()
    }
}

fn researched(state: &mut PlanState, proposals: Vec<Proposal>) {
    submit_with(state, main_agent(), proposals, &research());
}

fn refused(state: &PlanState, proposal: Proposal) -> Reason {
    let rejection = refuse_with(state, main_agent(), vec![proposal], &research());
    rejection.reasons[0].clone()
}

fn edit(id: &str, patch: TaskPatch) -> Proposal {
    Proposal::EditTask {
        task_id: tid(id),
        patch: Box::new(patch),
    }
}

fn provenance(state: &PlanState, id: &str) -> Provenance {
    state.task(&tid(id)).unwrap().provenance
}

#[test]
fn new_tasks_in_an_active_plan_wait_for_the_user() {
    let mut state = plan(PlanStatus::Active);
    let pending = TaskDraft {
        status: Some(TaskStatus::Pending),
        ..task("b")
    };
    researched(
        &mut state,
        vec![create_task(task("a")), create_task(pending)],
    );
    for id in ["a", "b"] {
        assert_eq!(status_of(&state, id), TaskStatus::Proposed);
        assert_eq!(provenance(&state, id), Provenance::Research);
    }
    add(&mut state, "mine", &[]);
    assert_eq!(provenance(&state, "mine"), Provenance::User);
}

#[test]
fn a_draft_or_replanning_approval_covers_research_tasks() {
    for status in [PlanStatus::Draft, PlanStatus::Replanning] {
        let mut state = plan(status);
        researched(&mut state, vec![create_task(task("a"))]);
        assert_eq!(status_of(&state, "a"), TaskStatus::Pending);
        assert_eq!(provenance(&state, "a"), Provenance::Research);
    }
}

#[test]
fn only_the_user_accepts_a_task_proposed_from_web_content() {
    let mut state = plan(PlanStatus::Active);
    researched(
        &mut state,
        vec![create_task(task("a")), create_task(task("b"))],
    );
    let accept = |id: &str| Proposal::from(WorkEdit::AcceptTask { task_id: tid(id) });
    assert!(matches!(refused(&state, accept("a")), Reason::Untrusted(_)));
    let clean = refuse(&state, main_agent(), vec![accept("a")]);
    assert!(clean.needs_user());
    submit(&mut state, user(), vec![accept("a")]);
    assert_eq!(status_of(&state, "a"), TaskStatus::Pending);
    assert_eq!(provenance(&state, "a"), Provenance::Research);
    let dismiss = Proposal::from(WorkEdit::CancelTask { task_id: tid("b") });
    submit(&mut state, user(), vec![dismiss]);
    assert_eq!(status_of(&state, "b"), TaskStatus::Cancelled);
}

#[test]
fn approving_a_replan_leaves_research_proposals_waiting() {
    let mut state = plan(PlanStatus::Active);
    researched(&mut state, vec![create_task(task("a"))]);
    state.header.status = PlanStatus::Replanning;
    submit(&mut state, user(), vec![Proposal::from(WorkEdit::Approve)]);
    assert_eq!(state.header.status, PlanStatus::Active);
    assert_eq!(status_of(&state, "a"), TaskStatus::Proposed);
}

#[test]
fn edits_to_what_the_plan_says_go_to_the_user() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    submit(
        &mut state,
        user(),
        vec![Proposal::AddItem {
            id: Some(EntryId::from("ent_r")),
            kind: PlanItemKind::Requirement,
            text: "keep the API".into(),
        }],
    );
    let scope = TaskScope {
        files: vec!["src/x.rs".into()],
        symbols: vec![],
    };
    let edits = [
        Proposal::EditPlan {
            goal: Some("Something else".into()),
            document: None,
        },
        edit(
            "a",
            TaskPatch {
                title: Some("Renamed".into()),
                ..TaskPatch::default()
            },
        ),
        edit(
            "a",
            TaskPatch {
                scope: Some(scope),
                ..TaskPatch::default()
            },
        ),
        Proposal::SetCriteria {
            task_id: tid("a"),
            criteria: vec![criterion("looks right", CriterionMethod::Inspection)],
        },
        Proposal::AddDependency {
            task_id: tid("a"),
            depends_on: tid("b"),
        },
        Proposal::MoveTask {
            task_id: tid("a"),
            parent: Some(tid("b")),
            index: 0,
        },
        set("a", TaskStatus::Cancelled),
        set("a", TaskStatus::Superseded { by: tid("b") }),
        Proposal::SetItemStatus {
            item_id: EntryId::from("ent_r"),
            status: PlanItemStatus::Retired,
        },
        Proposal::EditItem {
            item_id: EntryId::from("ent_r"),
            text: "break the API".into(),
        },
    ];
    for proposal in edits {
        let reason = refused(&state, proposal.clone());
        assert!(matches!(reason, Reason::Untrusted(_)), "{proposal:?}");
        assert!(reason.to_string().contains("Plan tab"), "{reason}");
    }
}

#[test]
fn progress_stays_allowed() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let steps = vec![TodoItem {
        content: "read the docs".into(),
        status: TodoStatus::InProgress,
        active_form: "Reading the docs".into(),
    }];
    researched(
        &mut state,
        vec![
            set("a", TaskStatus::Active),
            Proposal::RecordSteps {
                task_id: tid("a"),
                steps,
            },
            edit(
                "a",
                TaskPatch {
                    title: Some("Task a".into()),
                    result: Some("works with v2".into()),
                    ..TaskPatch::default()
                },
            ),
            set("a", TaskStatus::Done),
        ],
    );
    let task = state.task(&tid("a")).unwrap();
    assert_eq!(task.status, TaskStatus::Done);
    assert_eq!(task.result.as_deref(), Some("works with v2"));
    assert_eq!(task.steps.len(), 1);
}

#[test]
fn research_shapes_its_own_proposals_until_the_user_takes_them() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "base", &[]);
    researched(
        &mut state,
        vec![create_task(task("a")), create_task(task("b"))],
    );
    let rename = edit(
        "a",
        TaskPatch {
            title: Some("Pin the client".into()),
            ..TaskPatch::default()
        },
    );
    researched(
        &mut state,
        vec![
            rename,
            Proposal::AddDependency {
                task_id: tid("a"),
                depends_on: tid("base"),
            },
            set("b", TaskStatus::Cancelled),
        ],
    );
    assert_eq!(state.task(&tid("a")).unwrap().title, "Pin the client");
    assert_eq!(status_of(&state, "b"), TaskStatus::Cancelled);
    let blocks_base = Proposal::AddDependency {
        task_id: tid("base"),
        depends_on: tid("a"),
    };
    assert!(matches!(refused(&state, blocks_base), Reason::Untrusted(_)));
}

#[test]
fn research_may_outline_a_draft_and_propose_items() {
    let mut state = plan(PlanStatus::Draft);
    researched(
        &mut state,
        vec![
            Proposal::EditPlan {
                goal: None,
                document: Some("# From the docs".into()),
            },
            Proposal::AddItem {
                id: Some(EntryId::from("ent_i")),
                kind: PlanItemKind::Invariant,
                text: "never call v1".into(),
            },
        ],
    );
    assert_eq!(state.header.document, "# From the docs");
    let item = &state.header.invariants[0];
    assert_eq!(
        (item.status, item.provenance),
        (PlanItemStatus::Proposed, Provenance::Research)
    );
}

#[test]
fn research_evidence_waits_instead_of_meeting_a_criterion() {
    let mut state = plan(PlanStatus::Active);
    let draft = TaskDraft {
        acceptance: vec![criterion("reviewed", CriterionMethod::Inspection)],
        ..task("a")
    };
    submit(&mut state, user(), vec![create_task(draft)]);
    let note = EvidenceRef::Note {
        text: "the docs confirm it".into(),
    };
    researched(&mut state, vec![link("a", "c1", note)]);
    let task = state.task(&tid("a")).unwrap();
    assert!(task.acceptance[0].evidence.is_empty());
    let entry = state.entries.values().next().unwrap();
    assert_eq!(entry.status, EntryStatus::Proposed);
    let EntryBody::Evidence(evidence) = &entry.body else {
        panic!("not evidence: {entry:?}");
    };
    assert_eq!(evidence.criterion_id.as_deref(), Some("c1"));
}

#[test]
fn research_concludes_only_unconfirmed_experiments() {
    let mut state = plan(PlanStatus::Active);
    let open = |id: &str| {
        Proposal::Record(Box::new(EntryDraft {
            id: Some(EntryId::from(id)),
            ..EntryDraft::new(EntryBody::Experiment(Experiment {
                hypothesis: format!("hypothesis {id}"),
                method: String::new(),
                result: None,
                conclusion: None,
                outcome: ExperimentOutcome::Open,
            }))
        }))
    };
    submit(&mut state, main_agent(), vec![open("ent_ok")]);
    researched(&mut state, vec![open("ent_web")]);
    let conclude = |id: &str| Proposal::ConcludeExperiment {
        entry_id: EntryId::from(id),
        outcome: ExperimentOutcome::Confirmed,
        result: Some("held".into()),
        conclusion: None,
    };
    researched(&mut state, vec![conclude("ent_web")]);
    assert!(matches!(
        refused(&state, conclude("ent_ok")),
        Reason::Untrusted(_)
    ));
}
