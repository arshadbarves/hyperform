//! `meta.json`: written with the session, replaced atomically, camelCase on
//! disk, validated on read, and mapped to the GUI summary.

mod support;

use std::fs;

use support::{SESSION, new_session, temp_store};
use z_engine_protocol::{SessionId, SessionSummary, TurnOutcome};
use z_engine_store::{SESSION_SCHEMA, SessionMeta, StoreError};

#[test]
fn create_writes_meta_matching_the_returned_value() {
    let (_dir, store) = temp_store();
    let (_log, meta) = store.create(new_session(SESSION)).unwrap();
    assert_eq!(store.read_meta(&SessionId::from(SESSION)).unwrap(), meta);
    assert_eq!(meta.schema, SESSION_SCHEMA);
    assert_eq!(meta.project_root, "/work/app");
    assert_eq!(meta.model, "claude-sonnet-4");
    assert_eq!(meta.message_count, 0);
    assert_eq!(meta.title, None);
    assert_eq!(meta.last_outcome, None);
    assert_eq!(meta.created_at, meta.updated_at);
    assert!(!meta.legacy);
}

#[test]
fn write_meta_replaces_the_file_without_leaving_temp_files() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    let (log, mut meta) = store.create(new_session(SESSION)).unwrap();
    for count in 1..=5 {
        meta.message_count = count;
        meta.title = Some(format!("title {count}"));
        meta.last_outcome = Some(TurnOutcome::Completed);
        store.write_meta(&meta).unwrap();
    }
    assert_eq!(store.read_meta(&id).unwrap(), meta);
    let session_dir = log.path().parent().unwrap();
    let mut names: Vec<String> = fs::read_dir(session_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["log.jsonl", "meta.json"]);
}

#[test]
fn meta_json_uses_camel_case_fields() {
    let (_dir, store) = temp_store();
    let (log, _) = store.create(new_session(SESSION)).unwrap();
    let raw = fs::read(log.path().with_file_name("meta.json")).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let mut keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "costUsd",
            "createdAt",
            "lastOutcome",
            "legacy",
            "messageCount",
            "model",
            "projectRoot",
            "schema",
            "sessionId",
            "title",
            "updatedAt"
        ]
    );
}

#[test]
fn corrupt_or_newer_meta_is_rejected_on_read() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    let (log, meta) = store.create(new_session(SESSION)).unwrap();
    let path = log.path().with_file_name("meta.json");

    fs::write(&path, "{ not json").unwrap();
    assert!(matches!(
        store.read_meta(&id),
        Err(StoreError::Json { line: 1, .. })
    ));

    let newer = SessionMeta {
        schema: SESSION_SCHEMA + 1,
        ..meta
    };
    fs::write(&path, serde_json::to_vec(&newer).unwrap()).unwrap();
    assert!(matches!(store.read_meta(&id), Err(StoreError::Invalid(_))));
}

#[test]
fn write_meta_requires_an_existing_session() {
    let (_dir, store) = temp_store();
    let (_log, meta) = store.create(new_session(SESSION)).unwrap();
    let stranger = SessionMeta {
        session_id: SessionId::new(),
        ..meta
    };
    assert!(matches!(
        store.write_meta(&stranger),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn summary_mirrors_meta_fields() {
    let meta = SessionMeta {
        schema: SESSION_SCHEMA,
        session_id: SessionId::from(SESSION),
        title: Some("Fix login".into()),
        project_root: "/work/app".into(),
        model: "claude-sonnet-4".into(),
        created_at: 10,
        updated_at: 20,
        message_count: 7,
        cost_usd: 0.25,
        last_outcome: Some(TurnOutcome::Failed {
            message: "boom".into(),
        }),
        legacy: true,
    };
    let summary = SessionSummary::from(&meta);
    assert_eq!(
        summary,
        SessionSummary {
            session_id: meta.session_id.clone(),
            title: meta.title.clone(),
            project_root: meta.project_root.clone(),
            created_at: 10,
            updated_at: 20,
            message_count: 7,
            cost_usd: 0.25,
            last_outcome: meta.last_outcome.clone(),
            legacy: true,
        }
    );
}
