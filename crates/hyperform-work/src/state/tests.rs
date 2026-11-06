use hyperform_protocol::work::{
    Actor, Discovery, EntryBody, PlanItemKind, PlanStatus, TaskStatus, WorkCause, WorkEvent,
    WorkRecord,
};
use hyperform_protocol::{EntryId, TaskId};

use super::*;
use crate::testkit::*;
use crate::{EntryDraft, Proposal, TaskDraft};

#[test]
fn a_log_must_start_with_plan_created() {
    assert_eq!(PlanState::fold(&[]), Err(FoldError::Empty));
    let stray = WorkRecord {
        seq: 1,
        revision: 1,
        at: 1,
        actor: Actor::User,
        cause: WorkCause::Edit,
        event: WorkEvent::PlanStatusChanged {
            from: PlanStatus::Draft,
            to: PlanStatus::Active,
        },
    };
    assert_eq!(PlanState::fold(&[stray]), Err(FoldError::NotAPlanLog));
}

#[test]
fn folding_the_log_reproduces_the_live_state() {
    let mut live = plan(PlanStatus::Active);
    let mut log: Vec<WorkRecord> = Vec::new();
    let created = crate::create(
        &crate::PlanDraft {
            plan_id: live.header.plan_id.clone(),
            project_root: "/p".into(),
            goal: "Ship it".into(),
            document: String::new(),
            depth: live.header.depth,
            status: PlanStatus::Active,
        },
        &crate::Submission::new(user(), WorkCause::Approval, vec![]),
        &facts(),
    )
    .unwrap();
    log.extend(created);
    log.extend(submit(&mut live, user(), vec![create_task(task("a"))]));
    log.extend(submit(&mut live, user(), vec![create_task(task("b"))]));
    log.extend(submit(
        &mut live,
        main_agent(),
        vec![
            Proposal::AddDependency {
                task_id: tid("b"),
                depends_on: tid("a"),
            },
            set("a", TaskStatus::Active),
        ],
    ));
    assert_eq!(PlanState::fold(&log).unwrap(), live);
}

#[test]
fn each_submission_is_one_revision_with_increasing_seq() {
    let mut state = plan(PlanStatus::Active);
    assert_eq!((state.revision(), state.seq), (1, 1));
    let records = submit(
        &mut state,
        user(),
        vec![create_task(task("a")), create_task(task("b"))],
    );
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|record| record.revision == 2));
    assert_eq!(
        records.iter().map(|r| r.seq).collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!((state.revision(), state.seq), (2, 3));
    submit(&mut state, user(), vec![set("a", TaskStatus::Active)]);
    let a = state.task(&tid("a")).unwrap();
    assert_eq!((a.created_revision, a.revision, a.attempts), (2, 3, 1));
    assert_eq!(state.task(&tid("b")).unwrap().revision, 2);
}

#[test]
fn a_no_op_submission_creates_no_revision() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let records = submit(&mut state, user(), vec![set("a", TaskStatus::Pending)]);
    assert!(records.is_empty());
    assert_eq!(state.revision(), 2);
}

#[test]
fn moving_a_task_renumbers_both_sibling_groups() {
    let mut state = plan(PlanStatus::Draft);
    for id in ["p", "a", "b", "c"] {
        add(&mut state, id, &[]);
    }
    let order = |state: &PlanState, id: &str| state.task(&tid(id)).unwrap().order;
    assert_eq!((order(&state, "a"), order(&state, "c")), (1, 3));
    let move_task = |state: &mut PlanState, id: &str, parent: Option<&str>, index| {
        let proposal = Proposal::MoveTask {
            task_id: tid(id),
            parent: parent.map(tid),
            index,
        };
        let submission = crate::Submission::new(user(), cause(), vec![proposal]);
        let records = crate::decide(state, &submission, &facts()).unwrap();
        state.apply_all(&records).tasks
    };
    let changed = move_task(&mut state, "c", None, 0);
    assert_eq!(
        state.ordered_children(None, None),
        ["c", "p", "a", "b"].map(tid)
    );
    assert_eq!(changed.len(), 4);
    let changed = move_task(&mut state, "a", Some("p"), 5);
    assert_eq!(state.ordered_children(None, None), ["c", "p", "b"].map(tid));
    assert_eq!(
        state.ordered_children(Some(&tid("p")), None),
        vec![tid("a")]
    );
    assert_eq!((order(&state, "a"), order(&state, "b")), (0, 2));
    assert_eq!(changed.into_iter().collect::<Vec<_>>(), ["a", "b"].map(tid));
}

#[test]
fn updates_carry_only_what_changed_and_the_change_entry() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    let change = EntryId::from("ent_change");
    let record = WorkRecord {
        seq: state.seq + 1,
        revision: state.revision() + 1,
        at: 5_000,
        actor: Actor::Kernel,
        cause: WorkCause::Change {
            entry_id: change.clone(),
        },
        event: WorkEvent::TaskHeld {
            task_id: tid("b"),
            held: true,
        },
    };
    let changes = state.apply(&record);
    let update = state.update(&changes);
    assert_eq!(update.revision, 4);
    assert_eq!(update.tasks.len(), 1);
    assert_eq!(update.tasks[0].id, tid("b"));
    assert_eq!(update.change, Some(change));
    assert_eq!(update.plan.updated_at, 5_000);
}

#[test]
fn records_naming_unknown_things_are_skipped() {
    let mut state = plan(PlanStatus::Active);
    let record = WorkRecord {
        seq: 9,
        revision: 9,
        at: 1,
        actor: Actor::User,
        cause: WorkCause::Edit,
        event: WorkEvent::TaskHeld {
            task_id: TaskId::from("ghost"),
            held: true,
        },
    };
    let changes = state.apply(&record);
    assert!(changes.tasks.is_empty());
    assert_eq!((state.skipped, state.seq), (1, 9));
}

#[test]
fn summaries_count_planned_tasks_only() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    add(&mut state, "c", &[]);
    submit(&mut state, user(), vec![set("c", TaskStatus::Cancelled)]);
    submit(
        &mut state,
        user(),
        vec![create_task(TaskDraft {
            status: Some(TaskStatus::Proposed),
            ..task("p")
        })],
    );
    submit(&mut state, user(), vec![set("a", TaskStatus::Active)]);
    submit(&mut state, user(), vec![set("a", TaskStatus::Done)]);
    let summary = state.summary();
    assert_eq!((summary.tasks_total, summary.tasks_done), (2, 1));
    assert_eq!(summary.status, PlanStatus::Active);
    let snapshot = state.snapshot();
    assert_eq!(snapshot.tasks.len(), 4);
}

#[test]
fn items_and_entries_fold_into_the_header_and_ledger() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![
            Proposal::AddItem {
                id: Some(EntryId::from("ent_r")),
                kind: PlanItemKind::Requirement,
                text: "Works offline".into(),
            },
            Proposal::AddItem {
                id: Some(EntryId::from("ent_i")),
                kind: PlanItemKind::Invariant,
                text: "No data loss".into(),
            },
            Proposal::Record(Box::new(EntryDraft::new(EntryBody::Discovery(Discovery {
                text: "IndexedDB caps at 50MB".into(),
                source: None,
            })))),
        ],
    );
    assert_eq!(state.header.requirements.len(), 1);
    assert_eq!(state.header.invariants[0].text, "No data loss");
    assert_eq!(state.header.invariants[0].revision, 2);
    let entry = state.entries.values().next().unwrap();
    assert_eq!((entry.created_revision, entry.created_at), (2, 1_000));
}
