//! A plain text turn: event order, persistence across reopen, the badge of
//! a turn that changed nothing, and the generated title.

mod support;

use support::Harness;
use z_engine_engine::ExportFormat;
use z_engine_protocol::{Event, SessionStatus, TurnOutcome, VerificationOutcome};
use z_engine_testkit::{FixtureRepo, Script};

fn position(events: &[Event], predicate: impl Fn(&Event) -> bool) -> usize {
    events
        .iter()
        .position(predicate)
        .unwrap_or_else(|| panic!("event missing from {events:#?}"))
}

#[tokio::test]
async fn text_turn_streams_persists_and_gets_a_title() {
    let builder = Harness::builder(FixtureRepo::empty());
    builder.model().push(Script::text("Hello there"));
    let mut h = builder.start().await;

    let turn = h.run_turn("say hello").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);
    assert_eq!(turn.verification, VerificationOutcome::NotApplicable);
    assert_eq!(
        (turn.usage.input_tokens, turn.usage.output_tokens),
        (100, 20)
    );
    h.expect(|event| matches!(event, Event::TitleChanged { title } if title == "Test session"))
        .await;
    h.expect(|event| {
        matches!(
            event,
            Event::StatusChanged {
                status: SessionStatus::Idle
            }
        )
    })
    .await;

    let seen = h.events.seen().to_vec();
    let order = [
        position(&seen, |e| {
            matches!(
                e,
                Event::StatusChanged {
                    status: SessionStatus::Busy
                }
            )
        }),
        position(&seen, |e| {
            matches!(
                e,
                Event::UserMessage {
                    steering: false,
                    ..
                }
            )
        }),
        position(&seen, |e| matches!(e, Event::TurnStarted { .. })),
        position(&seen, |e| matches!(e, Event::AssistantStarted { .. })),
        position(
            &seen,
            |e| matches!(e, Event::TextDelta { text, .. } if text == "Hello there"),
        ),
        position(&seen, |e| matches!(e, Event::AssistantFinished { .. })),
        position(&seen, |e| matches!(e, Event::TurnFinished { .. })),
    ];
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}");
    let between = &seen[order[5]..order[6]];
    assert!(
        between
            .iter()
            .any(|e| matches!(e, Event::UsageUpdated { .. })),
        "usage is reported before the turn finishes"
    );

    let markdown = h
        .engine
        .export_session(&h.session, ExportFormat::Markdown)
        .unwrap();
    assert!(markdown.contains("say hello") && markdown.contains("Hello there"));

    h.reopen().await;
    let snapshot = match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => snapshot,
        other => panic!("{other:?}"),
    };
    assert_eq!(snapshot.messages.len(), 2);
    assert_eq!(snapshot.turns.len(), 1);
    assert_eq!(snapshot.info.title.as_deref(), Some("Test session"));
    let listed = h.engine.list_sessions().unwrap();
    let summary = listed.iter().find(|s| s.session_id == h.session).unwrap();
    assert_eq!(summary.message_count, 2);
    assert_eq!(summary.last_outcome, Some(TurnOutcome::Completed));
    let json = h
        .engine
        .export_session(&h.session, ExportFormat::Json)
        .unwrap();
    assert!(json.contains("\"kind\": \"turnFinished\""));
}
