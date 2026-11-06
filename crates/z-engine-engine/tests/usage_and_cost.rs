//! Usage and cost accumulate per turn and per session (side requests such
//! as the title included), are announced with `UsageUpdated`, and survive
//! a reopen.

mod support;

use support::{BASE_SETTINGS, Harness};
use z_engine_protocol::{Event, Usage};
use z_engine_testkit::{FixtureRepo, Script};

/// USD per million tokens: 100 input + 20 output tokens cost $0.14.
const PRICING: &str = "\n[pricing.test-model]\ninput = 1000.0\noutput = 2000.0\n";

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[tokio::test]
async fn usage_and_cost_accumulate() {
    let settings = format!("{BASE_SETTINGS}{PRICING}");
    let builder = Harness::builder(FixtureRepo::empty()).settings(&settings);
    builder.model().push(Script::text("first"));
    builder.model().push(Script::text("second"));
    let mut h = builder.start().await;

    let first = h.run_turn("one").await;
    assert_eq!(
        (first.usage.input_tokens, first.usage.output_tokens),
        (100, 20)
    );
    assert!(close(first.cost_usd, 0.14), "{}", first.cost_usd);
    h.expect(|e| matches!(e, Event::TitleChanged { .. })).await;
    let second = h.run_turn("two").await;
    assert!(close(second.cost_usd, 0.14));

    let (session_usage, cost, context) = h
        .events
        .seen()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::UsageUpdated {
                session_usage,
                cost_usd,
                context_tokens,
                ..
            } => Some((*session_usage, *cost_usd, *context_tokens)),
            _ => None,
        })
        .unwrap();
    let expected = Usage {
        input_tokens: 300,
        output_tokens: 60,
        ..Usage::default()
    };
    assert_eq!(session_usage, expected, "two turns and the title request");
    assert!(close(cost, 0.42), "{cost}");
    assert!(
        context > 100,
        "context counts the last prompt and newer messages"
    );

    h.reopen().await;
    match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => {
            assert_eq!(snapshot.usage, expected);
            assert!(close(snapshot.cost_usd, 0.42));
        }
        other => panic!("{other:?}"),
    }
    let listed = h.engine.list_sessions().unwrap();
    assert!(close(listed[0].cost_usd, 0.42));
}
