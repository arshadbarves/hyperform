//! Closing a session while a turn runs a long tool cancels the turn
//! promptly: the tool's process tree is killed, the turn is recorded as
//! `Cancelled` with the call answered, and the session reopens valid.

mod support;

use std::time::{Duration, Instant};

use serde_json::json;
use support::{
    BASE_SETTINGS, Harness, assert_valid_request, assert_valid_transcript, group_gone, read_pid,
    results,
};
use z_engine_protocol::{Event, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn closing_mid_turn_cancels_it_cleanly() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    let pid_file = h.repo.path().join("shell.pid");
    h.model.push(Script::tool(
        "Bash",
        json!({
            "command": format!("echo $$ > '{}'; sleep 30", pid_file.display()),
            "timeout": 60_000
        }),
    ));
    h.submit("run the long command");
    h.wait(|e| matches!(e, Event::ToolStarted { .. })).await;
    let pgid = read_pid(&pid_file).await;

    let started = Instant::now();
    h.engine.close_session(&h.session).await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "close waited {:?} for the tool",
        started.elapsed()
    );
    assert!(group_gone(pgid).await, "the tool's tree outlived the close");
    let turn = match h.expect(|e| matches!(e, Event::TurnFinished { .. })).await {
        Event::TurnFinished { turn } => turn,
        other => panic!("{other:?}"),
    };
    assert_eq!(turn.outcome, TurnOutcome::Cancelled);

    h.events.drain();
    h.engine
        .open_session(h.repo.path(), Some(h.session.clone()))
        .await
        .unwrap();
    let snapshot = match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => snapshot,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        snapshot.turns.last().unwrap().outcome,
        TurnOutcome::Cancelled
    );
    assert_valid_transcript(&snapshot.messages);
    let answered = results(snapshot.messages.last().unwrap());
    assert_eq!(answered.len(), 1);
    assert!(answered[0].1, "{answered:?}");

    h.model.push(Script::text("fresh start"));
    assert_eq!(h.run_turn("next").await.outcome, TurnOutcome::Completed);
    assert_valid_request(&h.main_requests().pop().unwrap());
}
