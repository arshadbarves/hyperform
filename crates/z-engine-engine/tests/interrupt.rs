//! `Interrupt` with text cancels the running turn and starts a new one
//! with that text, marked as following an interruption.

mod support;

use support::{Harness, last_user_text};
use z_engine_llm::ModelEvent;
use z_engine_protocol::{Command, Event, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn interrupt_with_text_starts_a_new_turn() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model
        .push(Script::Stall(vec![ModelEvent::TextDelta("working".into())]));
    h.model.push(Script::text("switched"));
    h.submit("first task");
    h.wait(|e| matches!(e, Event::TextDelta { .. })).await;
    h.send(Command::Interrupt {
        text: Some("do this instead".into()),
    });

    let first = h.turn_finished().await;
    assert_eq!(first.outcome, TurnOutcome::Cancelled);
    let second = h.turn_finished().await;
    assert_eq!(second.outcome, TurnOutcome::Completed);
    assert_ne!(first.turn_id, second.turn_id);

    let requests = h.main_requests();
    assert_eq!(requests.len(), 2);
    let prompt = last_user_text(&requests[1]);
    assert!(prompt.contains("do this instead"), "{prompt}");
    assert!(
        prompt.contains("interrupted"),
        "the interruption is announced: {prompt}"
    );
}
