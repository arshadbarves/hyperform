use hyperform_protocol::work::{
    Actor, PlanDepth, PlanItemKind, PlanItemStatus, PlanStatus, Provenance, ReconcileFinding,
    TaskStatus, WorkCause, WorkEdit, WorkEvent,
};
use hyperform_protocol::{EntryId, PlanId};

use crate::testkit::*;
use crate::{Facts, PlanDraft, PlanState, Proposal, Reason, Submission, TaskDraft, create, decide};

fn draft(goal: &str, status: PlanStatus) -> PlanDraft {
    PlanDraft {
        plan_id: PlanId::from("pln_x"),
        project_root: "/p".into(),
        goal: goal.into(),
        document: "# Plan".into(),
        depth: PlanDepth::Light,
        status,
    }
}

fn requirement(id: &str) -> Proposal {
    Proposal::AddItem {
        id: Some(EntryId::from(id)),
        kind: PlanItemKind::Requirement,
        text: format!("requirement {id}"),
    }
}

#[test]
fn a_new_plan_and_its_first_tasks_share_revision_one() {
    let submission = Submission::new(
        main_agent(),
        WorkCause::Approval,
        vec![
            requirement("ent_r"),
            create_task(TaskDraft {
                requirements: vec![EntryId::from("ent_r")],
                ..task("a")
            }),
        ],
    );
    let records = create(
        &draft(" Ship it ", PlanStatus::Draft),
        &submission,
        &facts(),
    )
    .unwrap();
    assert_eq!(records.len(), 3);
    assert!(records.iter().all(|record| record.revision == 1));
    assert_eq!(records.iter().map(|r| r.seq).collect::<Vec<_>>(), [1, 2, 3]);
    let state = PlanState::fold(&records).unwrap();
    assert_eq!(state.header.goal, "Ship it");
    assert_eq!(
        state.header.requirements[0].status,
        PlanItemStatus::Proposed
    );
    assert_eq!(state.header.requirements[0].provenance, Provenance::Agent);
}

#[test]
fn only_the_user_or_main_agent_creates_plans() {
    let none = Submission::new(worker("w1"), cause(), vec![]);
    let rejection = create(&draft("x", PlanStatus::Draft), &none, &facts()).unwrap_err();
    assert!(matches!(rejection.reasons[0], Reason::WorkerScope { .. }));
    let research = Facts {
        untrusted_input: true,
        ..facts()
    };
    let main = Submission::new(main_agent(), cause(), vec![]);
    assert!(create(&draft("x", PlanStatus::Draft), &main, &research).is_ok());
    let compaction = Submission::new(Actor::Compaction, cause(), vec![]);
    let rejection = create(&draft("x", PlanStatus::Draft), &compaction, &facts()).unwrap_err();
    assert!(matches!(rejection.reasons[0], Reason::Compaction(_)));
    let rejection = create(&draft(" ", PlanStatus::Paused), &main, &facts()).unwrap_err();
    assert_eq!(
        rejection.messages(),
        ["a plan needs a goal", "a plan starts as draft or active"]
    );
}

#[test]
fn approving_resuming_and_abandoning_belong_to_the_user() {
    let mut state = plan(PlanStatus::Draft);
    let approve = Proposal::from(WorkEdit::Approve);
    let rejection = refuse(&state, main_agent(), vec![approve.clone()]);
    assert!(rejection.needs_user());
    submit(&mut state, user(), vec![approve]);
    assert_eq!(state.header.status, PlanStatus::Active);
    let replan = Proposal::from(WorkEdit::RequestReplan);
    submit(&mut state, main_agent(), vec![replan]);
    assert_eq!(state.header.status, PlanStatus::Replanning);
    let resume = Proposal::from(WorkEdit::Resume);
    assert_eq!(
        refuse(&state, user(), vec![resume]).reasons,
        [Reason::PlanTransition {
            from: PlanStatus::Replanning,
            to: PlanStatus::Active,
        }]
    );
    let edit = Submission::edit(WorkEdit::Approve);
    let records = decide(&state, &edit, &facts()).unwrap();
    assert_eq!(records[0].actor, Actor::User);
    assert_eq!(records[0].cause, WorkCause::Edit);
}

#[test]
fn a_plan_is_done_only_when_every_task_is_closed() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    let finish = Proposal::SetPlanStatus {
        status: PlanStatus::Done,
        only_from: vec![],
    };
    submit(
        &mut state,
        main_agent(),
        vec![set("a", TaskStatus::Cancelled)],
    );
    assert_eq!(
        refuse(&state, main_agent(), vec![finish.clone()]).reasons,
        [Reason::OpenTasks {
            tasks: vec![tid("b")],
        }]
    );
    submit(&mut state, main_agent(), vec![set("b", TaskStatus::Done)]);
    submit(&mut state, main_agent(), vec![finish]);
    assert_eq!(state.header.status, PlanStatus::Done);
}

#[test]
fn findings_move_an_active_plan_to_replanning() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let quiet = Proposal::Reconcile { findings: vec![] };
    assert!(submit(&mut state, Actor::Kernel, vec![quiet.clone()]).is_empty());
    let finding = ReconcileFinding::ScopeDeleted {
        task_id: tid("a"),
        files: vec!["src/gone.rs".into()],
    };
    let reconcile = Proposal::Reconcile {
        findings: vec![finding.clone()],
    };
    assert!(matches!(
        refuse(&state, main_agent(), vec![reconcile.clone()]).reasons[0],
        Reason::EngineOnly(_)
    ));
    let records = submit(&mut state, Actor::Kernel, vec![reconcile]);
    assert!(matches!(
        records[1].event,
        WorkEvent::PlanStatusChanged { .. }
    ));
    assert_eq!(state.header.status, PlanStatus::Replanning);
    assert_eq!(state.findings, vec![finding]);
    submit(&mut state, Actor::Kernel, vec![quiet]);
    assert!(
        state.findings.is_empty(),
        "a clean pass clears old findings"
    );
    assert_eq!(state.header.status, PlanStatus::Replanning);
}

#[test]
fn agent_items_wait_for_the_user_and_user_items_are_protected() {
    let mut state = plan(PlanStatus::Active);
    submit(&mut state, main_agent(), vec![requirement("ent_a")]);
    submit(&mut state, user(), vec![requirement("ent_u")]);
    let item = |state: &PlanState, id: &str| state.item(&EntryId::from(id)).unwrap().clone();
    assert_eq!(item(&state, "ent_a").status, PlanItemStatus::Proposed);
    assert_eq!(item(&state, "ent_u").status, PlanItemStatus::Active);
    let accept = Proposal::from(WorkEdit::AcceptItem {
        item_id: EntryId::from("ent_a"),
    });
    assert!(refuse(&state, main_agent(), vec![accept.clone()]).needs_user());
    submit(&mut state, user(), vec![accept]);
    assert_eq!(item(&state, "ent_a").status, PlanItemStatus::Active);
    assert_eq!(
        item(&state, "ent_a").provenance,
        Provenance::User,
        "an accepted item is the user's"
    );
    submit(&mut state, main_agent(), vec![requirement("ent_b")]);
    let reword = Proposal::EditItem {
        item_id: EntryId::from("ent_u"),
        text: "changed".into(),
    };
    assert!(refuse(&state, main_agent(), vec![reword]).needs_user());
    let retire = |id: &str| Proposal::SetItemStatus {
        item_id: EntryId::from(id),
        status: PlanItemStatus::Retired,
    };
    assert!(refuse(&state, main_agent(), vec![retire("ent_u")]).needs_user());
    assert!(refuse(&state, main_agent(), vec![retire("ent_a")]).needs_user());
    submit(&mut state, main_agent(), vec![retire("ent_b")]);
    assert_eq!(item(&state, "ent_b").status, PlanItemStatus::Retired);
    assert!(matches!(
        refuse(&state, user(), vec![requirement("ent_u")]).reasons[0],
        Reason::DuplicateId(_)
    ));
}

#[test]
fn tasks_may_only_trace_to_known_requirements() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![Proposal::AddItem {
            id: Some(EntryId::from("ent_i")),
            kind: PlanItemKind::Invariant,
            text: "no data loss".into(),
        }],
    );
    let draft = TaskDraft {
        requirements: vec![EntryId::from("ent_i"), EntryId::from("ent_none")],
        ..task("a")
    };
    let reasons = refuse(&state, main_agent(), vec![create_task(draft)]).reasons;
    assert_eq!(
        reasons,
        [
            Reason::UnknownItem(EntryId::from("ent_i")),
            Reason::UnknownItem(EntryId::from("ent_none")),
        ]
    );
}

#[test]
fn plan_edits_and_checkpoints_change_only_what_differs() {
    let mut state = plan(PlanStatus::Active);
    let same = Proposal::EditPlan {
        goal: Some("Ship it".into()),
        document: None,
    };
    assert!(submit(&mut state, main_agent(), vec![same]).is_empty());
    let blank = Proposal::from(WorkEdit::EditGoal { goal: "  ".into() });
    assert!(
        refuse(&state, user(), vec![blank])
            .to_string()
            .contains("needs a goal")
    );
    let link = Proposal::LinkCheckpoint {
        fingerprint: "fp9".into(),
        checkpoint: None,
    };
    assert!(
        refuse(&state, main_agent(), vec![link.clone()])
            .to_string()
            .contains("engine")
    );
    submit(&mut state, Actor::Kernel, vec![link.clone()]);
    assert_eq!(state.fingerprint.as_deref(), Some("fp9"));
    assert!(submit(&mut state, Actor::Kernel, vec![link]).is_empty());
}

#[test]
fn only_the_engine_changes_the_plan_depth() {
    let mut state = plan(PlanStatus::Active);
    let deep = || Proposal::SetDepth {
        depth: PlanDepth::Deep,
    };
    let rejection = refuse(&state, user(), vec![deep()]);
    assert!(matches!(rejection.reasons[0], Reason::EngineOnly(_)));
    let records = submit(&mut state, Actor::Kernel, vec![deep()]);
    assert!(matches!(
        records[0].event,
        WorkEvent::DepthChanged {
            depth: PlanDepth::Deep
        }
    ));
    assert_eq!(state.header.depth, PlanDepth::Deep);
    assert!(submit(&mut state, Actor::Kernel, vec![deep()]).is_empty());
}

#[test]
fn a_recheck_is_the_users_and_needs_a_verifying_task() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let recheck = || Proposal::from(WorkEdit::RecheckTask { task_id: tid("a") });
    let rejection = refuse(&state, user(), vec![recheck()]);
    assert!(
        rejection.messages()[0].contains("not verifying"),
        "{rejection}"
    );
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    submit(&mut state, user(), vec![set("a", TaskStatus::Verifying)]);
    let rejection = refuse(&state, main_agent(), vec![recheck()]);
    assert!(
        matches!(rejection.reasons[0], Reason::UserOnly(_)),
        "{rejection}"
    );
    assert!(submit(&mut state, user(), vec![recheck()]).is_empty());
    assert_eq!(status_of(&state, "a"), TaskStatus::Verifying);
}
