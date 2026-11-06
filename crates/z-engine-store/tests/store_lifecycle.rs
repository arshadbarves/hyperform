//! Creating, reopening, loading, and deleting sessions, and refusing ids
//! that are not safe path segments.

mod support;

use std::fs;

use support::records::{turn, usage};
use support::v1::{v1_session_events, write_v1};
use support::{SESSION, new_session, temp_store, ulid_at};
use z_engine_protocol::{AgentId, Message, SessionId, TurnId, TurnOutcome};
use z_engine_store::{LogRecord, SESSION_SCHEMA, StoreError};

#[test]
fn created_session_exists_and_loads_its_start_record() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    assert!(!store.exists(&id));
    let (log, meta) = store.create(new_session(SESSION)).unwrap();
    assert!(store.exists(&id));
    assert_eq!(log.path(), store.dir().join(SESSION).join("log.jsonl"));

    let loaded = store.load(&id).unwrap();
    assert_eq!(loaded.meta, meta);
    assert_eq!(
        loaded.state.info,
        Some((id, "/work/app".to_string(), meta.created_at))
    );
    assert_eq!(loaded.state.model.as_deref(), Some("claude-sonnet-4"));
    assert!(!loaded.torn_tail);
    assert_eq!(loaded.corrupt_lines, 0);
}

#[test]
fn creating_a_taken_id_is_invalid() {
    let (_dir, store) = temp_store();
    drop(store.create(new_session(SESSION)).unwrap());
    assert!(matches!(
        store.create(new_session(SESSION)),
        Err(StoreError::Invalid(_))
    ));

    let v1_id = ulid_at(1_700_000_000_000);
    write_v1(store.dir(), &v1_id, &v1_session_events());
    assert!(matches!(
        store.create(new_session(&v1_id)),
        Err(StoreError::Invalid(_))
    ));
    assert!(!store.dir().join(&v1_id).exists());
}

#[test]
fn appended_records_are_replayed_by_load() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    let (mut log, _) = store.create(new_session(SESSION)).unwrap();
    let prompt = Message::user_text("add a test");
    let turn_id = TurnId::new();
    for record in [
        LogRecord::Message {
            message: prompt.clone(),
            turn_id: Some(turn_id.clone()),
        },
        LogRecord::TurnStarted {
            turn_id: turn_id.clone(),
            message_id: prompt.id.clone(),
            started_at: 1,
        },
        LogRecord::Message {
            message: Message::assistant_text("done"),
            turn_id: Some(turn_id.clone()),
        },
        LogRecord::TurnFinished {
            turn: turn(
                &turn_id,
                &prompt.id,
                TurnOutcome::Completed,
                usage(5, 5),
                0.3,
            ),
        },
        LogRecord::Title {
            title: "Add a test".into(),
        },
    ] {
        log.append(&record).unwrap();
    }
    log.sync().unwrap();

    let state = store.load(&id).unwrap().state;
    assert_eq!(state.transcript.len(), 2);
    assert_eq!(state.turns.len(), 1);
    assert_eq!(state.open_turn, None);
    assert_eq!(state.title.as_deref(), Some("Add a test"));
}

#[test]
fn load_rebuilds_missing_meta_from_the_log() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    let (mut log, _) = store.create(new_session(SESSION)).unwrap();
    let prompt = Message::user_text("hello");
    let turn_id = TurnId::new();
    log.append(&LogRecord::Message {
        message: prompt.clone(),
        turn_id: None,
    })
    .unwrap();
    log.append(&LogRecord::TurnStarted {
        turn_id,
        message_id: prompt.id.clone(),
        started_at: 2,
    })
    .unwrap();
    fs::remove_file(store.dir().join(SESSION).join("meta.json")).unwrap();

    let meta = store.load(&id).unwrap().meta;
    assert_eq!(meta.message_count, 1);
    assert_eq!(meta.last_outcome, Some(TurnOutcome::Interrupted));
    assert_eq!(meta.project_root, "/work/app");
    assert_eq!(meta.updated_at, prompt.created_at.max(meta.created_at));
    assert!(!meta.legacy);
    assert_eq!(store.read_meta(&id).unwrap(), meta);
}

#[test]
fn load_refuses_a_log_from_a_newer_schema() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    let (log, _) = store.create(new_session(SESSION)).unwrap();
    let newer = LogRecord::SessionStarted {
        schema: SESSION_SCHEMA + 1,
        session_id: id.clone(),
        project_root: "/work/app".into(),
        model: "m".into(),
        mode: z_engine_protocol::PermissionMode::Default.into(),
        planning: false,
        created_at: 1,
    };
    let mut line = serde_json::to_vec(&newer).unwrap();
    line.push(b'\n');
    fs::write(log.path(), line).unwrap();
    assert!(matches!(store.load(&id), Err(StoreError::Invalid(_))));
}

#[test]
fn unknown_sessions_are_not_found() {
    let (_dir, store) = temp_store();
    let id = SessionId::new();
    assert!(!store.exists(&id));
    assert!(matches!(store.load(&id), Err(StoreError::NotFound(_))));
    assert!(matches!(
        store.open_append(&id),
        Err(StoreError::NotFound(_))
    ));
    assert!(matches!(store.delete(&id), Err(StoreError::NotFound(_))));
    assert!(matches!(store.read_meta(&id), Err(StoreError::NotFound(_))));
    assert!(matches!(
        store.agent_log(&id, &AgentId::from("agt_1")),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn delete_removes_the_directory_and_the_v1_file() {
    let (_dir, store) = temp_store();
    let id = ulid_at(1_700_000_000_000);
    let session = SessionId::from(id.as_str());
    let v1 = write_v1(store.dir(), &id, &v1_session_events());
    store.load(&session).unwrap();
    assert!(store.dir().join(&id).is_dir());

    store.delete(&session).unwrap();
    assert!(!store.dir().join(&id).exists());
    assert!(!v1.exists());
    assert!(!store.exists(&session));
    assert!(store.list().unwrap().is_empty());
    assert!(matches!(
        store.delete(&session),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn writes_through_a_deleted_session_log_fail() {
    let (_dir, store) = temp_store();
    let (mut log, _) = store.create(new_session(SESSION)).unwrap();
    store.delete(&SessionId::from(SESSION)).unwrap();
    let note = LogRecord::Note {
        text: "lost?".into(),
    };
    assert!(log.append(&note).is_err());
    assert!(log.sync().is_err());
}

#[test]
fn ids_that_are_not_path_segments_are_rejected() {
    let (dir, store) = temp_store();
    for bad in ["../escape", "a/b", "", "."] {
        let id = SessionId::from(bad);
        let invalid =
            |result: Result<(), StoreError>| matches!(result, Err(StoreError::Invalid(_)));
        assert!(invalid(store.create(new_session(bad)).map(drop)), "{bad:?}");
        assert!(invalid(store.load(&id).map(drop)));
        assert!(invalid(store.open_append(&id).map(drop)));
        assert!(invalid(store.delete(&id)));
        assert!(invalid(store.read_meta(&id).map(drop)));
        assert!(invalid(store.agent_log(&id, &AgentId::main()).map(drop)));
        assert!(invalid(
            store
                .artifacts(&id)
                .write_text("x", "txt", "data")
                .map(drop)
        ));
        assert!(!store.exists(&id));
    }
    assert!(!dir.path().join("escape").exists());
    let (_log, _) = store.create(new_session(SESSION)).unwrap();
    assert!(matches!(
        store.agent_log(&SessionId::from(SESSION), &AgentId::from("../x")),
        Err(StoreError::Invalid(_))
    ));
}
