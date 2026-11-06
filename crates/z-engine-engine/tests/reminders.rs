//! Reminders the engine adds to user messages: instruction files of
//! directories the agent touches, and files changed behind its back.

mod support;

use serde_json::json;
use support::{Harness, last_user_text};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn nested_instructions_and_external_changes_are_announced() {
    let repo = FixtureRepo::git(&[
        ("src/api/AGENTS.md", "Handlers must return typed errors."),
        ("src/api/handler.rs", "fn handle() {}\n"),
    ]);
    let mut h = Harness::start(repo).await;
    let file = h.path("src/api/handler.rs");
    h.model
        .push(Script::tool("Read", json!({ "file_path": file })));
    h.model.push(Script::text("read it"));
    h.run_turn("look at the handler").await;
    let results_round = &h.main_requests()[1];
    let reminder = last_user_text(results_round);
    assert!(
        reminder.contains("Handlers must return typed errors."),
        "{reminder}"
    );

    std::fs::write(&file, "fn handle() { changed() }\n").unwrap();
    h.model.push(Script::text("noticed"));
    h.run_turn("anything new?").await;
    let prompt = last_user_text(&h.main_requests().pop().unwrap());
    assert!(
        prompt.contains("modified outside your own edits"),
        "{prompt}"
    );
    assert!(prompt.contains("src/api/handler.rs"), "{prompt}");
}
