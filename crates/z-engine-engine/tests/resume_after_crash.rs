//! A session whose app died mid-turn reopens with that turn `Interrupted`,
//! the dangling tool call answered, and a working set the next request can
//! send as-is.

mod support;

use serde_json::json;
use support::{Harness, assert_valid_transcript, last_user_text, results};
use z_engine_protocol::{CallId, ContentBlock, Event, Message, Role, TurnId, TurnOutcome, now_ms};
use z_engine_store::{LogRecord, SessionStore};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn interrupted_turn_is_closed_and_the_transcript_repaired() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.engine.close_session(&h.session).await.unwrap();

    let store = SessionStore::new(h.paths.sessions_dir.clone());
    let mut log = store.open_append(&h.session).unwrap();
    let turn_id = TurnId::new();
    let user = Message::user_text("read the notes");
    let call = CallId::from("call_dangling");
    let assistant = Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse {
            id: call.clone(),
            name: "Read".into(),
            input: json!({ "file_path": "notes.md" }),
        }],
    );
    log.append(&LogRecord::Message {
        message: user.clone(),
        turn_id: Some(turn_id.clone()),
    })
    .unwrap();
    log.append(&LogRecord::TurnStarted {
        turn_id: turn_id.clone(),
        message_id: user.id.clone(),
        started_at: now_ms(),
    })
    .unwrap();
    log.append(&LogRecord::Message {
        message: assistant,
        turn_id: Some(turn_id.clone()),
    })
    .unwrap();
    log.sync().unwrap();
    drop(log);

    h.engine
        .open_session(h.repo.path(), Some(h.session.clone()))
        .await
        .unwrap();
    let snapshot = match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => snapshot,
        other => panic!("{other:?}"),
    };
    let last_turn = snapshot.turns.last().unwrap();
    assert_eq!(last_turn.turn_id, turn_id);
    assert_eq!(last_turn.outcome, TurnOutcome::Interrupted);
    assert_valid_transcript(&snapshot.messages);
    let repaired = results(snapshot.messages.last().unwrap());
    assert_eq!(repaired[0].0, call);
    assert!(repaired[0].1 && repaired[0].2.contains("interrupted"));

    h.model.push(Script::text("picking up again"));
    let turn = h.run_turn("continue").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);
    let request = h.main_requests().pop().unwrap();
    assert_valid_transcript(&request.messages);
    assert!(last_user_text(&request).contains("interrupted"));
    let listed = h.engine.list_sessions().unwrap();
    assert_eq!(listed[0].last_outcome, Some(TurnOutcome::Completed));
}
