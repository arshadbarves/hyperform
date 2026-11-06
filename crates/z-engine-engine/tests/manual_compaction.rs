//! `Compact` summarizes the main agent's history on request: the GUI hears
//! `CompactionStarted` (manual) before `Compacted`, and a conversation too
//! short to summarize never announces a start.

mod support;

use support::Harness;
use z_engine_protocol::{Command, CompactionTrigger, Event, SessionStatus};
use z_engine_testkit::{FixtureRepo, Script};

/// Runs a text turn and waits until the session is idle again, so the
/// actor accepts `Compact`.
async fn turn(h: &mut Harness, prompt: &str, reply: &str) {
    h.model.push(Script::text(reply));
    h.run_turn(prompt).await;
    h.wait(|e| {
        matches!(
            e,
            Event::StatusChanged {
                status: SessionStatus::Idle
            }
        )
    })
    .await;
}

fn compact(h: &Harness) {
    h.send(Command::Compact { instructions: None });
}

#[tokio::test]
async fn manual_compaction_announces_its_start() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    turn(&mut h, "first", "one").await;
    turn(&mut h, "second", "two").await;

    compact(&h);
    h.wait(|e| matches!(e, Event::Compacted { .. })).await;
    let seen = h.events.seen();
    let started = seen.iter().position(|e| {
        matches!(
            e,
            Event::CompactionStarted {
                trigger: CompactionTrigger::Manual
            }
        )
    });
    let compacted = seen
        .iter()
        .position(|e| matches!(e, Event::Compacted { .. }));
    assert!(
        started.is_some() && started < compacted,
        "CompactionStarted precedes Compacted: {seen:#?}"
    );
}

#[tokio::test]
async fn a_short_conversation_never_starts_compacting() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    turn(&mut h, "first", "one").await;

    compact(&h);
    h.notice("too short to compact").await;
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::CompactionStarted { .. })),
        0
    );
}
