//! Streamed command output reaches the tool card as `ToolProgress` before
//! the call's `ToolFinished`.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness};
use z_engine_protocol::Event;
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn command_output_streams_as_progress() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::tool(
        "Bash",
        json!({ "command": "echo first-line; sleep 0.2; echo second-line" }),
    ));
    h.model.push(Script::text("ran it"));
    h.run_turn("run a command").await;

    let seen = h.events.seen();
    let progress: String = seen
        .iter()
        .filter_map(|e| match e {
            Event::ToolProgress { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        progress.contains("first-line") && progress.contains("second-line"),
        "{progress}"
    );
    let last_progress = seen
        .iter()
        .rposition(|e| matches!(e, Event::ToolProgress { .. }))
        .unwrap();
    let finished = seen
        .iter()
        .position(|e| matches!(e, Event::ToolFinished { .. }))
        .unwrap();
    assert!(last_progress < finished);
}
