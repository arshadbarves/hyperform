//! The session log becomes unwritable mid-turn (it is replaced by a
//! directory while a tool runs). The failure is an `Error` event, the turn
//! ends `Failed` instead of claiming success, the actor keeps answering
//! (new prompts are refused visibly, cancel and close work), and once the
//! log is back the session reopens with that turn interrupted.

mod support;

use std::time::Duration;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, all_text, assert_valid_request, assert_valid_transcript};
use z_engine_context::MISSING_RESULT;
use z_engine_protocol::{Command, Event, PermissionMode, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn an_unwritable_log_fails_the_turn_and_the_session_stays_responsive() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    let dir = h.paths.sessions_dir.join(h.session.as_str());
    let (log, saved) = (dir.join("log.jsonl"), dir.join("log.saved"));
    let sabotage = format!(
        "mv '{}' '{}' && mkdir '{}'",
        log.display(),
        saved.display(),
        log.display()
    );
    h.model
        .push(Script::tool("Bash", json!({ "command": sabotage })));
    h.model.push(Script::text("this must never be claimed"));
    let turn = h.run_turn("break the log").await;
    assert!(
        matches!(turn.outcome, TurnOutcome::Failed { .. }),
        "{:?}",
        turn.outcome
    );
    assert!(h.events.count(|e| matches!(e, Event::Error { .. })) >= 1);
    assert_eq!(h.main_requests().len(), 1, "no round after the lost write");

    h.submit("are you there?");
    h.wait(|e| matches!(e, Event::Error { message } if message.contains("could not save your message")))
        .await;
    h.send(Command::Cancel);
    h.send(Command::SetMode {
        mode: PermissionMode::Default,
    });
    h.send(Command::SetPlanning { planning: true });
    h.wait(|e| matches!(e, Event::Error { message } if message.contains("could not save the session log")))
        .await;
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::ModeChanged { .. } | Event::PlanningChanged { .. })),
        0,
        "a change the log lost is not published"
    );
    tokio::time::timeout(Duration::from_secs(20), h.engine.close_session(&h.session))
        .await
        .expect("close does not hang on a broken log")
        .unwrap();

    restore(&log, &saved);
    h.events.drain();
    h.engine
        .open_session(h.repo.path(), Some(h.session.clone()))
        .await
        .unwrap();
    match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => {
            assert_eq!(
                snapshot.turns.last().map(|turn| &turn.outcome),
                Some(&TurnOutcome::Interrupted)
            );
            assert!(
                snapshot
                    .turns
                    .iter()
                    .all(|t| t.outcome != TurnOutcome::Completed)
            );
        }
        other => panic!("{other:?}"),
    }
}

fn restore(log: &std::path::Path, saved: &std::path::Path) {
    std::fs::remove_dir(log).unwrap();
    std::fs::rename(saved, log).unwrap();
}

/// The log comes back while the session is still open: the next turn
/// works, and its request answers the call whose result was never saved.
#[tokio::test]
async fn a_log_that_comes_back_lets_the_live_session_continue() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    let dir = h.paths.sessions_dir.join(h.session.as_str());
    let (log, saved) = (dir.join("log.jsonl"), dir.join("log.saved"));
    let sabotage = format!(
        "mv '{}' '{}' && mkdir '{}'",
        log.display(),
        saved.display(),
        log.display()
    );
    h.model
        .push(Script::tool("Bash", json!({ "command": sabotage })));
    let turn = h.run_turn("break the log").await;
    assert!(matches!(turn.outcome, TurnOutcome::Failed { .. }));

    restore(&log, &saved);
    h.model.push(Script::text("still here"));
    assert_eq!(
        h.run_turn("back again").await.outcome,
        TurnOutcome::Completed
    );
    let request = h.main_requests().pop().unwrap();
    assert_valid_request(&request);
    assert!(all_text(&request.messages[2]).contains(MISSING_RESULT));

    h.reopen().await;
    h.model.push(Script::text("and after a reopen"));
    assert_eq!(
        h.run_turn("once more").await.outcome,
        TurnOutcome::Completed
    );
    assert_valid_request(&h.main_requests().pop().unwrap());
    assert_valid_transcript(&h.transcript()[3..]);
}
