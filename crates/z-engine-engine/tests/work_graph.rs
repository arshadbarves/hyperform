//! The task graph through the work tools: a dependency keeps a task from
//! starting until it is done, completing a task names the next ready one,
//! a dependency cycle or an unknown dependency is refused with its reason
//! (and kept in the plan log as an audit record), and nothing starts while
//! the plan is still a draft.

mod support;

use serde_json::json;
use support::{Harness, new_task, rejections, task, tool_reply, update_task, work_settings};
use z_engine_protocol::work::{PlanStatus, TaskStatus, WorkEdit};
use z_engine_protocol::{Command, Event};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn dependencies_gate_readiness_and_cycles_are_refused() {
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&work_settings(""))
        .start()
        .await;
    let calls = [
        ("TaskCreate", new_task("Map the sync layer", json!({}))),
        (
            "TaskCreate",
            new_task("Queue writes", json!({ "blockedBy": ["t1"] })),
        ),
        (
            "TaskCreate",
            new_task("Replay on reconnect", json!({ "blockedBy": ["t2"] })),
        ),
        ("TaskList", json!({})),
        (
            "TaskUpdate",
            update_task("t2", json!({ "status": "in_progress" })),
        ),
        (
            "TaskUpdate",
            update_task("t1", json!({ "addBlockedBy": ["t3"] })),
        ),
        (
            "TaskCreate",
            new_task("Orphan", json!({ "blockedBy": ["t9"] })),
        ),
        (
            "TaskUpdate",
            update_task("t1", json!({ "status": "in_progress" })),
        ),
        (
            "TaskUpdate",
            update_task("t1", json!({ "status": "completed" })),
        ),
        ("TaskList", json!({})),
    ];
    for (tool, input) in calls {
        h.model.push(Script::tool(tool, input));
    }
    h.model.push(Script::text("Planned."));
    h.run_turn("plan offline sync").await;

    let requests = h.main_requests();
    let reply = |call: usize| tool_reply(&requests[call + 1]);
    let (error, created) = reply(0);
    assert!(!error, "{created}");
    assert!(
        created.contains("Created task t1: Map the sync layer"),
        "{created}"
    );
    assert!(
        created.contains("started the work plan \"Map the sync layer\" (active)"),
        "outside planning the plan starts active: {created}"
    );
    let (_, listed) = reply(3);
    let ready = listed.split("Ready:").nth(1).unwrap();
    let ready = ready.split("Blocked or waiting:").next().unwrap();
    assert!(ready.contains("t1: Map the sync layer"), "{listed}");
    assert!(!ready.contains("t2") && !ready.contains("t3"), "{listed}");
    assert!(
        listed.contains("t2: Queue writes") && listed.contains("after t1"),
        "{listed}"
    );
    assert!(listed.contains("after t2"), "{listed}");

    let (error, early) = reply(4);
    assert!(error && early.contains("task t2 cannot start"), "{early}");
    let (error, cycle) = reply(5);
    assert!(error && cycle.contains("dependency cycle"), "{cycle}");
    let (error, orphan) = reply(6);
    assert!(error && orphan.contains("unknown task t9"), "{orphan}");
    let (error, done) = reply(8);
    assert!(!error, "{done}");
    assert!(done.contains("Next ready task: t2: Queue writes"), "{done}");
    let (_, listed) = reply(9);
    let ready = listed.split("Ready:").nth(1).unwrap();
    let ready = ready.split("Blocked or waiting:").next().unwrap();
    assert!(ready.contains("t2: Queue writes"), "{listed}");

    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Active);
    assert_eq!(plan.tasks.len(), 3, "the refused task was not created");
    assert_eq!(plan.task(&task("t1")).unwrap().status, TaskStatus::Done);
    assert!(plan.task(&task("t1")).unwrap().depends_on.is_empty());
    assert_eq!(plan.task(&task("t3")).unwrap().depends_on, vec![task("t2")]);
    let refused = rejections(&h.plan_log().records);
    assert_eq!(refused.len(), 3, "each refusal is audited: {refused:?}");
    assert!(
        refused[1]
            .iter()
            .any(|reason| reason.contains("dependency cycle"))
    );
}

#[tokio::test]
async fn a_draft_plan_starts_no_task_until_approved() {
    let settings = work_settings("[permissions]\nplanning = true\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::tool(
        "TaskCreate",
        new_task("Read the client", json!({})),
    ));
    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "status": "in_progress" })),
    ));
    h.model.push(Script::text("Drafted."));
    h.run_turn("plan the retry fix").await;

    let requests = h.main_requests();
    let (_, created) = tool_reply(&requests[1]);
    assert!(
        created.contains("(draft)"),
        "planning drafts the plan: {created}"
    );
    let (error, refused) = tool_reply(&requests[2]);
    assert!(
        error && refused.contains("task t1 cannot start"),
        "{refused}"
    );
    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Draft);
    assert_eq!(plan.task(&task("t1")).unwrap().attempts, 0);

    h.send(Command::EditWork {
        plan_id: h.plan_id(),
        edit: WorkEdit::Approve,
    });
    h.wait(
        |e| matches!(e, Event::WorkUpdated { update } if update.plan.status == PlanStatus::Active),
    )
    .await;
    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "status": "in_progress" })),
    ));
    h.model.push(Script::text("Started."));
    h.run_turn("start it").await;
    let (error, started) = tool_reply(h.main_requests().last().unwrap());
    assert!(!error, "{started}");
    assert_eq!(
        h.plan().task(&task("t1")).unwrap().status,
        TaskStatus::Active
    );
}
