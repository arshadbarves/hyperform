//! Provider failures: a context overflow forces relief and one retry; any
//! other error ends the turn as `Failed` with the message recorded.

mod support;

use support::Harness;
use z_engine_llm::LlmError;
use z_engine_protocol::{Event, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn context_overflow_is_retried_once() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::Error(LlmError::Http {
        status: 400,
        body: "prompt is too long: 300000 tokens > 200000 maximum".into(),
    }));
    h.model.push(Script::text("fits now"));
    let turn = h.run_turn("hello").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);
    assert_eq!(h.main_requests().len(), 2);
}

#[tokio::test]
async fn other_errors_fail_the_turn() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::Error(LlmError::Http {
        status: 401,
        body: "invalid api key".into(),
    }));
    let turn = h.run_turn("hello").await;
    match &turn.outcome {
        TurnOutcome::Failed { message } => {
            assert!(message.contains("invalid api key"), "{message}")
        }
        other => panic!("expected a failure, got {other:?}"),
    }
    assert_eq!(
        h.main_requests().len(),
        1,
        "non-retryable errors are not retried"
    );
    h.reopen().await;
    match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => {
            assert!(matches!(
                snapshot.turns[0].outcome,
                TurnOutcome::Failed { .. }
            ));
        }
        other => panic!("{other:?}"),
    }
}
