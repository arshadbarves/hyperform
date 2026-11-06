//! `TodoWrite` updates the GUI, is persisted, and comes back in the
//! snapshot after the session is reopened.

mod support;

use serde_json::json;
use support::Harness;
use z_engine_protocol::{Event, TodoStatus};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn todos_are_announced_persisted_and_restored() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::tool(
        "TodoWrite",
        json!({ "todos": [
            { "content": "Write tests", "status": "in_progress", "activeForm": "Writing tests" },
            { "content": "Ship", "status": "pending", "activeForm": "Shipping" }
        ]}),
    ));
    h.model.push(Script::text("Planned."));
    h.run_turn("plan the work").await;

    match h.expect(|e| matches!(e, Event::TodosUpdated { .. })).await {
        Event::TodosUpdated { agent_id, todos } => {
            assert!(agent_id.is_main());
            assert_eq!(todos.len(), 2);
            assert_eq!(todos[0].status, TodoStatus::InProgress);
        }
        other => panic!("{other:?}"),
    }

    h.reopen().await;
    match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => {
            assert_eq!(snapshot.todos.len(), 2);
            assert_eq!(snapshot.todos[1].content, "Ship");
        }
        other => panic!("{other:?}"),
    }
}
