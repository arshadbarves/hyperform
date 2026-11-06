//! The built-in `/review` prompt command expands `$ARGUMENTS` and reaches
//! the model as two text blocks: the visible invocation, then the body
//! wrapped in `<command name="review">`.

mod support;

use support::{Harness, user_blocks};
use z_engine_protocol::{ContentBlock, Event, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn review_expands_arguments_into_a_second_block() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::text("Looks fine."));
    h.command("review", "src/lib.rs");
    let turn = h.turn_finished().await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);

    let request = h.main_requests().pop().unwrap();
    let blocks = user_blocks(&request);
    assert_eq!(blocks[0], "/review src/lib.rs");
    assert!(
        blocks[1].starts_with("<command name=\"review\">"),
        "{}",
        blocks[1]
    );
    assert!(blocks[1].trim_end().ends_with("</command>"));
    assert!(blocks[1].contains("Target: src/lib.rs"), "{}", blocks[1]);
    assert!(!blocks[1].contains("$ARGUMENTS"));

    let message = match h.expect(|e| matches!(e, Event::UserMessage { .. })).await {
        Event::UserMessage { message, .. } => message,
        other => panic!("{other:?}"),
    };
    let texts: Vec<&str> = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts[0], "/review src/lib.rs",
        "the GUI sees the same blocks"
    );
}
