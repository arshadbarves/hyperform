//! A damaged session log (garbage lines, an unknown record, a torn final
//! line) still opens: everything readable before and between the bad lines
//! is restored, a warning says how many lines were skipped, and appending
//! continues on a clean line boundary.

mod support;

use std::io::Write;

use support::{Harness, assert_valid_transcript};
use z_engine_protocol::{Event, NoticeLevel, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

fn append(path: &std::path::Path, text: &str) {
    let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

async fn damage_notice(h: &mut Harness) -> String {
    match h
        .wait(|e| matches!(e, Event::Notice { level: NoticeLevel::Warn, text } if text.contains("damaged")))
        .await
    {
        Event::Notice { text, .. } => text,
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn damaged_lines_are_skipped_and_appending_continues() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    let log = h
        .paths
        .sessions_dir
        .join(h.session.as_str())
        .join("log.jsonl");
    h.model.push(Script::text("answer one"));
    h.run_turn("first").await;
    h.engine.close_session(&h.session).await.unwrap();
    append(
        &log,
        "this is not json\n{\"kind\":\"fromTheFuture\",\"x\":1}\n",
    );

    h.events.drain();
    h.engine
        .open_session(h.repo.path(), Some(h.session.clone()))
        .await
        .unwrap();
    assert!(damage_notice(&mut h).await.contains("2 damaged"));
    h.model.push(Script::text("answer two"));
    assert_eq!(h.run_turn("second").await.outcome, TurnOutcome::Completed);
    h.engine.close_session(&h.session).await.unwrap();
    append(&log, "{\"kind\":\"message\",\"message\":{\"id\":\"torn");

    h.events.drain();
    h.engine
        .open_session(h.repo.path(), Some(h.session.clone()))
        .await
        .unwrap();
    assert!(damage_notice(&mut h).await.contains("3 damaged"));
    let texts: Vec<String> = h.transcript().iter().map(|m| m.text()).collect();
    assert_eq!(texts.len(), 4, "{texts:?}");
    assert!(texts[1].contains("answer one") && texts[3].contains("answer two"));

    h.model.push(Script::text("answer three"));
    assert_eq!(h.run_turn("third").await.outcome, TurnOutcome::Completed);
    let raw = std::fs::read_to_string(&log).unwrap();
    assert!(raw.ends_with('\n'), "appends start on a line boundary");
    assert!(!raw.contains("\"id\":\"torn"), "the torn fragment is cut");
    assert!(
        raw.contains("this is not json"),
        "unreadable data is never rewritten"
    );

    h.reopen().await;
    let transcript = h.transcript();
    assert_eq!(transcript.len(), 6);
    assert_valid_transcript(&transcript);
    assert!(damage_notice(&mut h).await.contains("2 damaged"));
}
