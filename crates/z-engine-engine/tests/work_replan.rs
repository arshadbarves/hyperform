//! Replanning: turning planning on over an active plan moves it to
//! Replanning and opens one plan change that every revision until the
//! approval belongs to; `/replan` with nothing found makes no model call
//! unless the user says what to reconsider; and a file deleted from a
//! task's scope between turns is found at the next turn start, which
//! reminds the agent and holds the tasks it names (and their dependents)
//! until the user accepts. A done task's own change is not stale
//! evidence; a later edit is.

mod support;

use serde_json::json;
use support::{Harness, new_task, task, tool_reply, update_task, work_settings};
use z_engine_protocol::work::{
    ChangeCause, EntryBody, PlanStatus, ReconcileFinding, WorkCause, WorkEdit, WorkEvent,
};
use z_engine_protocol::{Command, Event, PlanDecision};
use z_engine_testkit::{FixtureRepo, Script};

fn changes(h: &Harness) -> Vec<(z_engine_protocol::EntryId, ChangeCause)> {
    h.plan()
        .entries
        .values()
        .filter_map(|entry| match &entry.body {
            EntryBody::PlanChange(change) => Some((entry.id.clone(), change.cause)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn planning_on_groups_the_revision_under_one_plan_change() {
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&work_settings(""))
        .start()
        .await;
    h.model.push(Script::tool(
        "TaskCreate",
        new_task("Add the read cache", json!({})),
    ));
    h.model.push(Script::text("Planned."));
    h.settled_turn("cache the reads").await;

    h.send(Command::SetPlanning { planning: true });
    h.wait(|e| matches!(e, Event::WorkUpdated { update } if update.plan.status == PlanStatus::Replanning))
        .await;
    let [(change, cause)] = changes(&h).try_into().unwrap();
    assert_eq!(cause, ChangeCause::User);

    let calls = [
        (
            "TaskCreate",
            new_task("Invalidate on write", json!({ "blockedBy": ["t1"] })),
        ),
        (
            "TaskUpdate",
            update_task("t1", json!({ "subject": "Add the LRU read cache" })),
        ),
        (
            "PlanRecord",
            json!({ "kind": "decision", "title": "Which cache?", "text": "An LRU", "reason": "Bounded memory" }),
        ),
        (
            "ExitPlanMode",
            json!({ "plan": "1. LRU cache\n2. Invalidate on write" }),
        ),
    ];
    for (tool, input) in calls {
        h.model.push(Script::tool(tool, input));
    }
    h.model.push(Script::text("Revised."));
    h.submit("make it an LRU and invalidate on write");
    let request_id = match h.wait(|e| matches!(e, Event::PlanProposed { .. })).await {
        Event::PlanProposed { request_id, .. } => request_id,
        other => panic!("{other:?}"),
    };
    h.send(Command::ResolvePlan {
        request_id,
        decision: PlanDecision::Approve {
            mode: None,
            edited_plan: None,
            handoff: false,
        },
    });
    h.turn_finished().await;

    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Active);
    assert_eq!(
        plan.task(&task("t1")).unwrap().title,
        "Add the LRU read cache"
    );
    assert_eq!(changes(&h).len(), 1, "one plan change for the revision");
    let log = h.plan_log().records;
    let opened = log
        .iter()
        .position(|r| {
            matches!(
                r.event,
                WorkEvent::PlanStatusChanged {
                    to: PlanStatus::Replanning,
                    ..
                }
            )
        })
        .unwrap();
    let approved = log
        .iter()
        .position(|r| {
            matches!(
                r.event,
                WorkEvent::PlanStatusChanged {
                    to: PlanStatus::Active,
                    ..
                }
            )
        })
        .unwrap();
    let revised: Vec<&WorkCause> = log[opened + 1..approved]
        .iter()
        .filter(|record| !matches!(record.event, WorkEvent::EntryRecorded { ref entry } if entry.id == change))
        .map(|record| &record.cause)
        .collect();
    assert!(revised.len() >= 3, "{revised:?}");
    let grouped = WorkCause::Change {
        entry_id: change.clone(),
    };
    assert!(
        revised.iter().all(|cause| **cause == grouped),
        "{revised:?}"
    );
    assert_eq!(log[approved].cause, WorkCause::Approval);
    let updates = h.events.count(
        |e| matches!(e, Event::WorkUpdated { update } if update.change.as_ref() == Some(&change)),
    );
    assert!(
        updates >= 3,
        "the GUI hears which change each revision joins"
    );
}

#[tokio::test]
async fn replan_with_nothing_found_makes_no_model_call() {
    let mut h = Harness::builder(FixtureRepo::git(&[("src/a.rs", "fn a() {}\n")]))
        .settings(&work_settings(""))
        .start()
        .await;
    h.model.push(Script::tool(
        "TaskCreate",
        new_task("Tidy a", json!({ "scope": ["src/a.rs"] })),
    ));
    h.model.push(Script::text("Planned."));
    h.settled_turn("tidy a").await;
    let before = h.model.requests().len();

    h.command("replan", "");
    let nothing = h.command_output("replan").await;
    assert!(nothing.contains("nothing to replan"), "{nothing}");
    assert_eq!(h.model.requests().len(), before, "no model call");
    assert_eq!(h.plan().header.status, PlanStatus::Active);
    assert!(changes(&h).is_empty());

    h.model.push(Script::text("Reconsidered."));
    h.command("replan", "split the tidy into two");
    h.turn_finished().await;
    assert_eq!(h.plan().header.status, PlanStatus::Replanning);
    let [(_, cause)] = changes(&h).try_into().unwrap();
    assert_eq!(cause, ChangeCause::Replan);
    let asked = support::last_user_text(h.main_requests().last().unwrap());
    assert!(asked.contains("split the tidy into two"), "{asked}");
}

#[tokio::test]
async fn a_scope_file_the_task_will_create_is_not_a_deletion() {
    let mut h = Harness::builder(FixtureRepo::git(&[("src/a.rs", "fn a() {}\n")]))
        .settings(&work_settings(""))
        .start()
        .await;
    h.model.push(Script::tool(
        "TaskCreate",
        new_task("Add b", json!({ "scope": ["src/b.rs"] })),
    ));
    h.model.push(Script::text("Planned."));
    h.run_turn("add b").await;

    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "status": "in_progress" })),
    ));
    h.model.push(Script::text("Started."));
    h.run_turn("go on").await;

    let (error, reply) = tool_reply(h.main_requests().last().unwrap());
    assert!(!error, "{reply}");
    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Active);
    assert!(plan.findings.is_empty(), "{:?}", plan.findings);
}

#[tokio::test]
async fn a_scope_file_deleted_between_turns_is_found_at_the_next_turn() {
    let mut h = Harness::builder(FixtureRepo::git(&[("src/a.rs", "fn a() {}\n")]))
        .settings(&work_settings(""))
        .start()
        .await;
    h.model.push(Script::tool(
        "TaskCreate",
        new_task("Tidy a", json!({ "scope": ["src/a.rs"] })),
    ));
    h.model.push(Script::text("Planned."));
    h.run_turn("tidy a").await;
    std::fs::remove_file(h.repo.join("src/a.rs")).unwrap();

    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "status": "in_progress" })),
    ));
    h.model.push(Script::text("Noted."));
    h.run_turn("go on").await;

    let requests = h.main_requests();
    let first = &requests[requests.len() - 2];
    let reminded = support::last_user_text(first);
    assert!(
        reminded.contains("Checking the work plan against the workspace")
            && reminded.contains("t1: files in its scope were deleted: src/a.rs"),
        "{reminded}"
    );
    let (error, refused) = tool_reply(requests.last().unwrap());
    assert!(
        error && refused.contains("task t1 cannot start"),
        "{refused}"
    );
    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Replanning);
    assert!(matches!(
        plan.findings.as_slice(),
        [ReconcileFinding::ScopeDeleted { .. }]
    ));
    let [(_, cause)] = changes(&h).try_into().unwrap();
    assert_eq!(cause, ChangeCause::Reconciliation);

    h.send(Command::EditWork {
        plan_id: h.plan_id(),
        edit: WorkEdit::Approve,
    });
    h.wait(
        |e| matches!(e, Event::WorkUpdated { update } if update.plan.status == PlanStatus::Active),
    )
    .await;
}

#[tokio::test]
async fn a_done_tasks_own_change_is_not_stale_but_a_later_edit_is() {
    let repo = FixtureRepo::git(&[("src/a.rs", "fn a() {}\n")]);
    let settings = work_settings("[permissions]\nmode = \"bypass\"\n");
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    let calls = [
        (
            "TaskCreate",
            new_task("Tidy a", json!({ "scope": ["src/a.rs"] })),
        ),
        ("TaskCreate", new_task("Document a", json!({}))),
        (
            "TaskUpdate",
            update_task("t1", json!({ "status": "in_progress" })),
        ),
        ("Read", json!({ "file_path": "src/a.rs" })),
        (
            "Write",
            json!({ "file_path": "src/a.rs", "content": "fn a() { tidy() }\n" }),
        ),
        (
            "TaskUpdate",
            update_task("t1", json!({ "status": "completed" })),
        ),
    ];
    for (tool, input) in calls {
        h.model.push(Script::tool(tool, input));
    }
    h.model.push(Script::text("Tidied."));
    h.settled_turn("tidy a").await;

    h.model.push(Script::text("Nothing else."));
    h.settled_turn("anything else?").await;
    let plan = h.plan();
    assert_eq!(
        plan.header.status,
        PlanStatus::Active,
        "{:?}",
        plan.findings
    );

    h.repo.write("src/a.rs", "fn a() { changed() }\n");
    h.model.push(Script::text("Noted."));
    h.settled_turn("I edited a").await;
    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Replanning);
    assert!(
        matches!(plan.findings.as_slice(), [ReconcileFinding::StaleEvidence { task_id, .. }] if *task_id == task("t1")),
        "{:?}",
        plan.findings
    );
}
