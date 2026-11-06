//! Subagent transcripts under `agents/<agent>.jsonl`: messages only, in
//! order, surviving reopen and a torn tail.

mod support;

use support::{SESSION, append_raw, new_session, temp_store};
use z_engine_protocol::{AgentId, Message, SessionId, TurnId};
use z_engine_store::{LogRecord, StoreError};

#[test]
fn agent_transcripts_round_trip_messages_in_order() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    drop(store.create(new_session(SESSION)).unwrap());
    let agent = AgentId::from("agt_01j8z3k4m5");
    let messages = [
        Message::user_text("find the callers of login()"),
        Message::assistant_text("Found two callers."),
    ];
    let mut log = store.agent_log(&id, &agent).unwrap();
    assert_eq!(
        log.path(),
        store
            .dir()
            .join(SESSION)
            .join("agents")
            .join("agt_01j8z3k4m5.jsonl")
    );
    log.append_message(&messages[0]).unwrap();
    log.append(&LogRecord::Message {
        message: messages[1].clone(),
        turn_id: Some(TurnId::new()),
    })
    .unwrap();
    log.sync().unwrap();

    assert_eq!(store.load_agent_transcript(&id, &agent).unwrap(), messages);
    let main = store.load(&id).unwrap();
    assert!(
        main.state.transcript.is_empty(),
        "agent messages stay out of the main log"
    );
}

#[test]
fn reopening_an_agent_log_appends_after_a_torn_tail() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    drop(store.create(new_session(SESSION)).unwrap());
    let agent = AgentId::from("agt_1");
    let first = Message::assistant_text("first");
    let mut log = store.agent_log(&id, &agent).unwrap();
    log.append_message(&first).unwrap();
    let path = log.path().to_path_buf();
    drop(log);
    append_raw(&path, br#"{"kind":"message","message":{"id":"msg_x","ro"#);

    let second = Message::assistant_text("second");
    store
        .agent_log(&id, &agent)
        .unwrap()
        .append_message(&second)
        .unwrap();
    assert_eq!(
        store.load_agent_transcript(&id, &agent).unwrap(),
        [first, second]
    );
}

#[test]
fn agent_logs_refuse_non_message_records() {
    let (_dir, store) = temp_store();
    drop(store.create(new_session(SESSION)).unwrap());
    let mut log = store
        .agent_log(&SessionId::from(SESSION), &AgentId::from("agt_1"))
        .unwrap();
    let title = LogRecord::Title {
        title: "not here".into(),
    };
    assert!(matches!(log.append(&title), Err(StoreError::Invalid(_))));
    log.append_message(&Message::user_text("still usable"))
        .unwrap();
}

#[test]
fn missing_agent_transcripts_are_not_found() {
    let (_dir, store) = temp_store();
    drop(store.create(new_session(SESSION)).unwrap());
    assert!(matches!(
        store.load_agent_transcript(&SessionId::from(SESSION), &AgentId::from("agt_none")),
        Err(StoreError::NotFound(_))
    ));
}
