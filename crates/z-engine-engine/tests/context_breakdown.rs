//! The context card's breakdown query: the same estimate as `/context`,
//! pushed to the GUI as a `ContextReport`, without a transcript card.

mod support;

use support::Harness;
use z_engine_protocol::{Event, SessionId};
use z_engine_testkit::FixtureRepo;

#[tokio::test]
async fn breakdown_reports_layers_without_a_transcript_card() {
    let mut h = Harness::start(FixtureRepo::empty()).await;

    let breakdown = h
        .engine
        .context_breakdown(&h.session)
        .expect("the session is open");
    assert!(breakdown.system > 0 && breakdown.tools > 0);
    assert_eq!(breakdown.limit, 128_000);
    match h.expect(|e| matches!(e, Event::ContextReport { .. })).await {
        Event::ContextReport { breakdown: pushed } => assert_eq!(pushed, breakdown),
        other => panic!("{other:?}"),
    }
    h.events.drain();
    assert!(
        !h.events
            .seen()
            .iter()
            .any(|e| matches!(e, Event::CommandOutput { .. })),
        "no transcript card"
    );

    assert!(h.engine.context_breakdown(&SessionId::new()).is_none());
}
