//! A provider stream that fails mid-response (after some text and half a
//! tool call) fails the turn with the provider's message, persists no
//! half-assembled assistant message, and leaves a transcript the next
//! request can send as-is.

mod support;

use support::{Harness, assert_valid_request, assert_valid_transcript};
use z_engine_llm::{LlmError, ModelEvent};
use z_engine_protocol::{Role, TurnOutcome};
use z_engine_store::SessionStore;
use z_engine_testkit::{FixtureRepo, Script};

fn broken_response() -> Script {
    Script::FailAfter(
        vec![
            ModelEvent::TextDelta("Let me read the file first.".into()),
            ModelEvent::ToolUseStart {
                index: 0,
                id: "call_partial".into(),
                name: "Read".into(),
            },
            ModelEvent::ToolUseDelta {
                index: 0,
                partial_json: "{\"file_pa".into(),
            },
        ],
        LlmError::Stream("connection reset by peer".into()),
    )
}

#[tokio::test]
async fn a_stream_failing_mid_tool_call_fails_the_turn_cleanly() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(broken_response());
    let turn = h.run_turn("read notes.md").await;
    match &turn.outcome {
        TurnOutcome::Failed { message } => {
            assert!(message.contains("connection reset by peer"), "{message}")
        }
        other => panic!("expected a failure, got {other:?}"),
    }

    let live = h.transcript();
    assert!(live.iter().all(|m| m.role == Role::User), "{live:#?}");
    let store = SessionStore::new(h.paths.sessions_dir.clone());
    let stored = store.load(&h.session).unwrap().state;
    assert!(
        stored.transcript.iter().all(|m| !m.has_tool_use()),
        "no unpaired tool_use may be persisted: {:#?}",
        stored.transcript
    );
    assert_eq!(stored.transcript.len(), 1);

    h.model.push(Script::text("recovered"));
    let turn = h.run_turn("try again").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);
    let request = h.main_requests().pop().unwrap();
    assert_valid_request(&request);
    assert!(request.messages[0].text().contains("read notes.md"));
    assert!(request.messages[0].text().contains("try again"));

    h.reopen().await;
    assert_valid_transcript(&h.transcript());
    assert_eq!(h.transcript().len(), 3);
}
