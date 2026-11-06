//! The user's edits from the Plan tab (`editWork`): each applies as the
//! user, a hold stops the agent from starting the task, a refused edit
//! becomes a notice and an audit record, and the plan comes back from disk
//! when the chat reopens or the app restarts, ignoring a torn last line.

mod support;

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;

use hyperform_protocol::work::{
    Actor, PlanItemKind, PlanItemStatus, Provenance, TaskStatus, WorkCause, WorkEdit,
};
use hyperform_protocol::{Command, Event};
use hyperform_testkit::{FixtureRepo, Script};
use serde_json::json;
use support::{Harness, new_task, rejections, task, tool_reply, update_task, work_settings};

async fn planned() -> Harness {
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&work_settings(""))
        .start()
        .await;
    h.model.push(Script::tool(
        "TaskCreate",
        new_task("Write the parser", json!({})),
    ));
    h.model.push(Script::tool(
        "TaskCreate",
        new_task("Wire the parser", json!({ "blockedBy": ["t1"] })),
    ));
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan the parser").await;
    h
}

fn edit(h: &Harness, edit: WorkEdit) {
    h.send(Command::EditWork {
        plan_id: h.plan_id(),
        edit,
    });
}

async fn updated(h: &mut Harness) {
    h.wait(|e| matches!(e, Event::WorkUpdated { .. })).await;
}

#[tokio::test]
async fn edits_apply_as_the_user_and_refusals_are_noticed_and_audited() {
    let mut h = planned().await;
    let rename = WorkEdit::RenameTask {
        task_id: task("t1"),
        title: "Write the TOML parser".into(),
    };
    edit(&h, rename);
    updated(&mut h).await;
    let requirement = WorkEdit::AddItem {
        kind: PlanItemKind::Requirement,
        text: "Old files stay readable".into(),
    };
    edit(&h, requirement);
    updated(&mut h).await;
    let hold = WorkEdit::HoldTask {
        task_id: task("t1"),
        held: true,
    };
    edit(&h, hold);
    updated(&mut h).await;
    let unknown = WorkEdit::RenameTask {
        task_id: task("t9"),
        title: "Nothing".into(),
    };
    edit(&h, unknown);
    let notice = h.notice("The plan change was not applied").await;
    assert!(notice.contains("unknown task t9"), "{notice}");

    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "status": "in_progress" })),
    ));
    h.model.push(Script::text("It is on hold."));
    h.run_turn("start the parser").await;
    let (error, held) = tool_reply(h.main_requests().last().unwrap());
    assert!(error && held.contains("task t1 cannot start"), "{held}");

    let plan = h.plan();
    let t1 = plan.task(&task("t1")).unwrap();
    assert_eq!(t1.title, "Write the TOML parser");
    assert!(t1.held);
    assert_eq!(t1.status, TaskStatus::Pending);
    let item = plan.items().next().unwrap();
    assert_eq!(
        item.status,
        PlanItemStatus::Active,
        "the user's own requirement"
    );
    assert_eq!(item.provenance, Provenance::User);
    let log = h.plan_log().records;
    let refused: Vec<_> = log
        .iter()
        .filter(|record| record.actor == Actor::User && record.cause == WorkCause::Edit)
        .filter(|record| !rejections(std::slice::from_ref(record)).is_empty())
        .collect();
    assert_eq!(refused.len(), 1, "the refused edit is audited: {log:#?}");
    assert_eq!(rejections(&log).len(), 2, "and so is the refused start");
}

#[tokio::test]
async fn the_plan_comes_back_after_a_reopen_and_a_restart_with_a_torn_tail() {
    let mut h = planned().await;
    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "status": "in_progress" })),
    ));
    h.model.push(Script::tool(
        "TodoWrite",
        json!({ "todos": [{ "content": "Lex the tokens", "status": "in_progress", "activeForm": "Lexing" }] }),
    ));
    h.model.push(Script::text("Lexing."));
    h.settled_turn("start").await;
    let before = h.plan();
    assert_eq!(before.task(&task("t1")).unwrap().steps.len(), 1);

    h.reopen().await;
    let work = match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => snapshot.work.expect("the plan is attached again"),
        other => panic!("{other:?}"),
    };
    assert_eq!(work.plan.plan_id, h.plan_id());
    assert_eq!(work.tasks.len(), 2);

    let events_file = h.plan_events_file();
    let remains = h.shut_down().await;
    tear_last_line(&events_file);
    let mut h = remains.restart().await;
    let work = h
        .events
        .seen()
        .iter()
        .find_map(|e| match e {
            Event::Snapshot { snapshot } => snapshot.work.clone(),
            _ => None,
        })
        .expect("the restarted chat has its plan");
    assert_eq!(work.plan.revision, before.revision());
    let t1 = work.tasks.iter().find(|t| t.id == task("t1")).unwrap();
    assert_eq!(t1.status, TaskStatus::Active);
    assert_eq!(t1.steps[0].content, "Lex the tokens");
    assert_eq!(
        h.plan_log().records.len() as u64,
        before.seq,
        "the torn line is no record"
    );

    let rename = WorkEdit::RenameTask {
        task_id: task("t2"),
        title: "Wire the TOML parser".into(),
    };
    edit(&h, rename);
    updated(&mut h).await;
    let loaded = h.plan_log();
    assert!(!loaded.torn_tail, "the torn line was repaired");
    assert_eq!(loaded.corrupt_lines, 0);
    assert_eq!(
        h.plan().task(&task("t2")).unwrap().title,
        "Wire the TOML parser"
    );
}

/// Leaves half a record at the end of the plan log, as a crash in the
/// middle of an append would.
fn tear_last_line(events_file: &Path) {
    let mut file = OpenOptions::new().append(true).open(events_file).unwrap();
    file.write_all(br#"{"seq":99,"revision":99,"at":1,"actor":{"type":"user"},"cau"#)
        .unwrap();
}
