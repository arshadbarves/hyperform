//! Cancelling mid-stream and mid-tool ends the turn as `Cancelled` and
//! leaves a transcript every provider accepts.

mod support;

use std::time::{Duration, Instant};

use serde_json::json;
use support::{BASE_SETTINGS, Harness, assert_valid_transcript, results};
use z_engine_llm::ModelEvent;
use z_engine_protocol::{Command, Event, Role, ToolStatus, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn cancel_during_streaming_keeps_the_partial_text() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model
        .push(Script::Stall(vec![ModelEvent::TextDelta("partial".into())]));
    h.submit("write an essay");
    h.wait(|e| matches!(e, Event::TextDelta { .. })).await;
    h.send(Command::Cancel);
    let turn = h.turn_finished().await;
    assert_eq!(turn.outcome, TurnOutcome::Cancelled);

    let transcript = h.transcript();
    assert_valid_transcript(&transcript);
    assert_eq!(transcript.len(), 2);
    assert_eq!(transcript[1].role, Role::Assistant);
    assert_eq!(transcript[1].text(), "partial");
}

#[tokio::test]
async fn cancel_during_a_tool_answers_every_call() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::tools(&[
        ("Bash", json!({ "command": "sleep 30" })),
        ("Bash", json!({ "command": "echo never" })),
    ]));
    h.submit("wait a while");
    h.wait(|e| matches!(e, Event::ToolStarted { .. })).await;
    let cancelled_at = Instant::now();
    h.send(Command::Cancel);
    let turn = h.turn_finished().await;
    assert!(cancelled_at.elapsed() < Duration::from_secs(10));
    assert_eq!(turn.outcome, TurnOutcome::Cancelled);

    let transcript = h.transcript();
    assert_valid_transcript(&transcript);
    let answered = results(transcript.last().unwrap());
    assert_eq!(answered.len(), 2);
    assert!(
        answered
            .iter()
            .all(|(_, is_error, text)| *is_error && text.contains("cancelled by user"))
    );
    assert_eq!(
        h.events.count(|e| matches!(
            e,
            Event::ToolFinished {
                status: ToolStatus::Cancelled,
                ..
            }
        )),
        2
    );
}
