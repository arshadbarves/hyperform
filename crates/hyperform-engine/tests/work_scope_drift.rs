//! Scope drift: a task-bound helper's write outside its task's declared
//! scope asks first in Default mode (an Allow becomes an Ask), never turns
//! a Deny into anything else, and is noted once per path as a discovery on
//! the task, whether the write then happens or not. Bypass mode does not
//! ask. An absolute path inside the scope is no drift.

mod support;

use hyperform_protocol::work::EntryBody;
use hyperform_protocol::{ApprovalDecision, Command, Event};
use hyperform_testkit::{FixtureRepo, Script};
use serde_json::json;
use support::{
    Harness, dispatch, new_task, report, results, route_task, task_requests, work_settings,
};

const RULES: &str = "[permissions]\nallow = [\"Write\"]\ndeny = [\"Write(secrets/**)\"]\n";
const BYPASS: &str = "[permissions]\nmode = \"bypass\"\n";

/// A plan with one task, `t1`, scoped to `src`.
async fn planned(extra: &str) -> Harness {
    let repo = FixtureRepo::git(&[("README.md", "fixture\n")]);
    let mut h = Harness::builder(repo)
        .settings(&work_settings(extra))
        .start()
        .await;
    let scope = json!({ "scope": ["src"] });
    h.model
        .push(Script::tool("TaskCreate", new_task("Add src", scope)));
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan it").await;
    h
}

fn write(path: &str) -> Script {
    Script::tool("Write", json!({ "file_path": path, "content": "x\n" }))
}

fn finish() -> [Script; 2] {
    let summary = json!({ "summary": "Wrote the files" });
    [
        Script::tool("TaskUpdate", report("t1", summary)),
        Script::text("Reported."),
    ]
}

/// The plan's notes about writes outside a task's scope.
fn drift_notes(h: &Harness) -> Vec<String> {
    let plan = h.plan();
    let notes = plan.entries.values().filter_map(|entry| match &entry.body {
        EntryBody::Discovery(found) if found.text.contains("outside its scope") => {
            Some(found.text.clone())
        }
        _ => None,
    });
    notes.collect()
}

fn asked(h: &Harness) -> usize {
    h.events
        .count(|e| matches!(e, Event::ApprovalRequested { .. }))
}

#[tokio::test]
async fn an_out_of_scope_write_asks_is_noted_and_never_loosens_a_deny() {
    let mut h = planned(RULES).await;
    let absolute = h.path("src/abs.rs");
    let mut rounds = vec![
        write("src/new.rs"),
        write(&absolute),
        write("docs/x.md"),
        write("secrets/key.txt"),
    ];
    rounds.extend(finish());
    route_task(&h.model, "DRIFT-ASK", rounds);
    h.model
        .push(Script::tool("Agent", dispatch("t1", "DRIFT-ASK")));
    h.model.push(Script::text("Dispatched."));
    h.submit("run t1");

    let request = h.approval().await;
    assert!(!request.agent_id.is_main());
    assert_eq!(request.input["file_path"], "docs/x.md");
    assert!(
        request
            .reason
            .contains("would write outside the task's declared scope: docs/x.md"),
        "{}",
        request.reason
    );
    h.send(Command::ResolveApproval {
        request_id: request.request_id,
        decision: ApprovalDecision::AllowOnce,
    });
    h.turn_finished().await;

    assert_eq!(
        asked(&h),
        1,
        "in-scope writes and the denied one do not ask"
    );
    for file in ["src/new.rs", "src/abs.rs", "docs/x.md"] {
        assert!(h.repo.exists(file), "{file}");
    }
    assert!(!h.repo.exists("secrets/key.txt"));
    let helper = task_requests(&h.model, "DRIFT-ASK");
    let denied = results(helper[4].messages.last().unwrap());
    assert!(denied[0].1, "the deny still holds: {denied:?}");
    assert_eq!(
        drift_notes(&h),
        [
            "helper for t1 tried to write outside its scope: docs/x.md",
            "helper for t1 tried to write outside its scope: secrets/key.txt",
        ]
    );
}

#[tokio::test]
async fn bypass_does_not_ask_and_notes_each_path_once() {
    let mut h = planned(BYPASS).await;
    let mut rounds = vec![
        write("docs/x.md"),
        Script::tool("Read", json!({ "file_path": "docs/x.md" })),
        write("docs/x.md"),
    ];
    rounds.extend(finish());
    route_task(&h.model, "DRIFT-BYPASS", rounds);
    let replies = h.dispatch_turn(&[("t1", "DRIFT-BYPASS")]).await;
    assert!(!replies[0].0, "{replies:?}");

    assert_eq!(asked(&h), 0);
    assert!(h.repo.exists("docs/x.md"));
    assert_eq!(
        drift_notes(&h),
        ["helper for t1 tried to write outside its scope: docs/x.md"]
    );
}
