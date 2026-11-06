//! A failure holds only what it affects. When task A's check fails, the
//! dependency finding moves the plan to Replanning but holds only its
//! dependent B (`replanHeld` in the snapshot and updates): an unrelated
//! task C still starts and finishes, and B, once unblocked, waits until the
//! user accepts the plan change. Replanning the user asked for (planning
//! turned on) still holds every task.

mod support;

use serde_json::json;
use support::{Harness, new_task, report, route_task, task, work_settings};
use z_engine_protocol::work::{PlanStatus, TaskStatus, WorkEdit};
use z_engine_protocol::{Command, Event, TaskId};
use z_engine_testkit::{FixtureRepo, Script};

const CHECKED: &str = "[permissions]\nmode = \"bypass\"\n\n\
    [[verification.checks]]\nid = \"marker\"\nlabel = \"Marker test\"\ncommand = \"test -f src/ok.txt\"\nkind = \"test\"\n";
const HELD: &str = "it is held until the plan change is accepted";

/// A (`t1`, checked by `test -f src/ok.txt`), B (`t2`, needs A) and C
/// (`t3`, unrelated), each scoped to a directory of the project.
async fn planned() -> Harness {
    let repo = FixtureRepo::git(&[
        ("src/lib.rs", "lib\n"),
        ("docs/README.md", "docs\n"),
        ("notes/README.md", "notes\n"),
    ]);
    let mut h = Harness::builder(repo)
        .settings(&work_settings(CHECKED))
        .start()
        .await;
    let checked = json!({
        "scope": ["src"],
        "acceptance": [{ "text": "The marker test passes", "method": "test" }]
    });
    let dependent = json!({ "scope": ["docs"], "blockedBy": ["t1"] });
    let unrelated = json!({ "scope": ["notes"] });
    for (title, input) in [
        ("Build lib", checked),
        ("Document lib", dependent),
        ("Write notes", unrelated),
    ] {
        h.model
            .push(Script::tool("TaskCreate", new_task(title, input)));
    }
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan the lib").await;
    h
}

/// The helper with `marker` reports on `task_id` without changing files.
fn reporter(h: &Harness, marker: &'static str, task_id: &str) {
    let summary = json!({ "summary": format!("Finished {task_id}") });
    let rounds = vec![
        Script::tool("TaskUpdate", report(task_id, summary)),
        Script::text("Reported."),
    ];
    route_task(&h.model, marker, rounds);
}

/// `replanHeld` of the latest `WorkUpdated` event.
fn last_held(h: &mut Harness) -> Vec<TaskId> {
    h.events.drain();
    let latest = h.events.seen().iter().rev().find_map(|event| match event {
        Event::WorkUpdated { update } => Some(update.replan_held.clone()),
        _ => None,
    });
    latest.expect("a work update")
}

#[tokio::test]
async fn a_failed_task_holds_only_its_dependents_while_the_plan_replans() {
    let mut h = planned().await;
    let write = json!({ "file_path": "src/one.txt", "content": "x\n" });
    let rounds = vec![
        Script::tools(&[
            ("Write", write),
            (
                "TaskUpdate",
                report("t1", json!({ "summary": "Wrote one" })),
            ),
        ]),
        Script::text("Reported."),
    ];
    route_task(&h.model, "KEEP-A", rounds);
    let replies = h.dispatch_turn(&[("t1", "KEEP-A")]).await;
    assert!(!replies[0].0, "{replies:?}");
    let failed = h.finished("t1").await;
    assert!(
        matches!(failed.status, TaskStatus::Failed { .. }),
        "{failed:?}"
    );

    let plan = h
        .plan_until(|plan| plan.header.status == PlanStatus::Replanning)
        .await;
    assert!(plan.replan_by_findings, "{:?}", plan.findings);
    let held: Vec<TaskId> = z_engine_work::replan_held(&plan).into_iter().collect();
    assert_eq!(held, [task("t2")], "a failed task is not open, C is free");
    assert_eq!(last_held(&mut h), [task("t2")]);

    reporter(&h, "KEEP-C", "t3");
    let replies = h.dispatch_turn(&[("t2", "KEEP-B"), ("t3", "KEEP-C")]).await;
    assert!(
        replies[0].0 && replies[0].1.contains("it is blocked"),
        "{replies:?}"
    );
    assert!(!replies[1].0, "{replies:?}");
    assert_eq!(h.finished("t3").await.status, TaskStatus::Done);
    assert_eq!(h.plan().header.status, PlanStatus::Replanning);

    h.edit_plan(WorkEdit::CancelTask {
        task_id: task("t1"),
    });
    h.task_until("t2", |t| t.status == TaskStatus::Pending)
        .await;
    let replies = h.dispatch_turn(&[("t2", "KEEP-B")]).await;
    assert!(replies[0].0 && replies[0].1.contains(HELD), "{replies:?}");
    assert!(h.plan().task(&task("t2")).unwrap().run.dispatch.is_none());

    h.edit_plan(WorkEdit::Approve);
    h.plan_until(|plan| plan.header.status == PlanStatus::Active)
        .await;
    assert!(last_held(&mut h).is_empty());
    reporter(&h, "KEEP-B", "t2");
    let replies = h.dispatch_turn(&[("t2", "KEEP-B")]).await;
    assert!(!replies[0].0, "{replies:?}");
    assert_eq!(h.finished("t2").await.status, TaskStatus::Done);
}

#[tokio::test]
async fn replanning_the_user_asked_for_holds_every_task() {
    let mut h = planned().await;
    h.send(Command::SetPlanning { planning: true });
    let plan = h
        .plan_until(|plan| plan.header.status == PlanStatus::Replanning)
        .await;
    assert!(!plan.replan_by_findings);
    assert!(z_engine_work::replan_held(&plan).is_empty());
    h.send(Command::SetPlanning { planning: false });
    h.wait(|e| matches!(e, Event::PlanningChanged { planning: false }))
        .await;

    let replies = h.dispatch_turn(&[("t3", "USER-C")]).await;
    assert!(
        replies[0].0 && replies[0].1.contains("the plan is replanning"),
        "{replies:?}"
    );
    assert_eq!(h.plan().header.status, PlanStatus::Replanning);
}
