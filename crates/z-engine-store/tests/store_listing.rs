//! Listing scans v2 directories and v1 files, newest first, and heals
//! missing or corrupt metadata from the log.

mod support;

use std::fs;

use serde_json::json;
use support::records::{turn, usage};
use support::v1::{v1_session_events, write_v1};
use support::{SESSION, new_session, set_mtime, temp_store, ulid_at};
use z_engine_protocol::{Message, SessionId, SessionSummary, TurnId, TurnOutcome};
use z_engine_store::{LogRecord, SessionStore};

fn summary<'a>(list: &'a [SessionSummary], id: &str) -> &'a SessionSummary {
    list.iter()
        .find(|summary| summary.session_id.as_str() == id)
        .unwrap_or_else(|| panic!("{id} not listed"))
}

/// A session with two messages, a finished turn costing 0.4, and a title.
fn busy_session(store: &SessionStore, id: &str) {
    let (mut log, _) = store.create(new_session(id)).unwrap();
    let prompt = Message::user_text("add logging");
    let turn_id = TurnId::new();
    for record in [
        LogRecord::Message {
            message: prompt.clone(),
            turn_id: Some(turn_id.clone()),
        },
        LogRecord::Message {
            message: Message::assistant_text("added"),
            turn_id: Some(turn_id.clone()),
        },
        LogRecord::TurnFinished {
            turn: turn(
                &turn_id,
                &prompt.id,
                TurnOutcome::Completed,
                usage(4, 2),
                0.4,
            ),
        },
        LogRecord::Title {
            title: "Add logging".into(),
        },
    ] {
        log.append(&record).unwrap();
    }
}

fn assert_busy_summary(summary: &SessionSummary) {
    assert_eq!(summary.message_count, 2);
    assert_eq!(summary.title.as_deref(), Some("Add logging"));
    assert_eq!(summary.last_outcome, Some(TurnOutcome::Completed));
    assert!((summary.cost_usd - 0.4).abs() < 1e-9);
    assert!(!summary.legacy);
}

#[test]
fn a_missing_sessions_dir_lists_nothing() {
    let (_dir, store) = temp_store();
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn lists_v2_and_unimported_v1_sessions_newest_first() {
    let (_dir, store) = temp_store();
    let (a, b) = (ulid_at(100), ulid_at(200));
    for (id, updated_at) in [(&a, 3_000), (&b, 1_000)] {
        let (_log, mut meta) = store.create(new_session(id)).unwrap();
        meta.updated_at = updated_at;
        store.write_meta(&meta).unwrap();
    }
    let c = ulid_at(300);
    let c_path = write_v1(
        store.dir(),
        &c,
        &[
            json!({"type": "meta", "model": "m", "project_root": "/work/c"}),
            json!({"type": "user_msg", "text": "fix login"}),
            json!({"type": "title", "text": "Fix login"}),
        ],
    );
    set_mtime(&c_path, 2_000);
    let d = ulid_at(400);
    let d_path = write_v1(
        store.dir(),
        &d,
        &[json!({"type": "user_msg", "text": "\n  Explain the parser\nplease"})],
    );
    set_mtime(&d_path, 4_000);
    fs::write(store.dir().join("index.json"), "{}").unwrap();
    fs::write(store.dir().join("notes.txt"), "hi").unwrap();
    fs::create_dir(store.dir().join(".import-stale")).unwrap();
    fs::create_dir(store.dir().join("not a session")).unwrap();

    let list = store.list().unwrap();
    let order: Vec<&str> = list.iter().map(|s| s.session_id.as_str()).collect();
    assert_eq!(order, [d.as_str(), a.as_str(), c.as_str(), b.as_str()]);

    let c_summary = summary(&list, &c);
    assert!(c_summary.legacy);
    assert_eq!(c_summary.title.as_deref(), Some("Fix login"));
    assert_eq!(c_summary.project_root, "/work/c");
    assert_eq!(c_summary.created_at, 300);
    assert_eq!(c_summary.updated_at, 2_000);
    let d_summary = summary(&list, &d);
    assert_eq!(d_summary.title.as_deref(), Some("Explain the parser"));
    assert_eq!(d_summary.project_root, "");
    assert!(!summary(&list, &a).legacy);
    assert!(!store.dir().join(&c).exists(), "listing must not import");
}

#[test]
fn imported_v1_sessions_are_listed_once_with_the_same_summary() {
    let (_dir, store) = temp_store();
    let id = ulid_at(1_700_000_000_000);
    let path = write_v1(store.dir(), &id, &v1_session_events());
    set_mtime(&path, 1_700_000_500_000);
    let before = store.list().unwrap();
    assert_eq!(before.len(), 1);
    assert!(before[0].legacy);
    assert_eq!(before[0].message_count, 8);

    store.load(&SessionId::from(id.as_str())).unwrap();
    let after = store.list().unwrap();
    assert_eq!(after, before);
}

#[test]
fn listing_rebuilds_and_rewrites_missing_meta() {
    let (_dir, store) = temp_store();
    busy_session(&store, SESSION);
    let meta_path = store.dir().join(SESSION).join("meta.json");
    fs::remove_file(&meta_path).unwrap();

    let list = store.list().unwrap();
    assert_busy_summary(summary(&list, SESSION));
    let healed = store.read_meta(&SessionId::from(SESSION)).unwrap();
    assert_eq!(SessionSummary::from(&healed), list[0]);
}

#[test]
fn listing_replaces_corrupt_meta() {
    let (_dir, store) = temp_store();
    busy_session(&store, SESSION);
    let meta_path = store.dir().join(SESSION).join("meta.json");
    fs::write(&meta_path, "garbage").unwrap();

    let list = store.list().unwrap();
    assert_busy_summary(summary(&list, SESSION));
    assert!(store.read_meta(&SessionId::from(SESSION)).is_ok());
}

#[test]
fn listing_summarizes_but_never_overwrites_meta_from_a_newer_schema() {
    let (_dir, store) = temp_store();
    busy_session(&store, SESSION);
    let meta_path = store.dir().join(SESSION).join("meta.json");
    let mut newer: serde_json::Value =
        serde_json::from_slice(&fs::read(&meta_path).unwrap()).unwrap();
    newer["schema"] = json!(99);
    newer["futureField"] = json!(true);
    let bytes = serde_json::to_vec(&newer).unwrap();
    fs::write(&meta_path, &bytes).unwrap();

    let list = store.list().unwrap();
    assert_busy_summary(summary(&list, SESSION));
    assert_eq!(fs::read(&meta_path).unwrap(), bytes);
}

#[test]
fn unreadable_session_dirs_are_skipped() {
    let (_dir, store) = temp_store();
    busy_session(&store, SESSION);
    let empty = ulid_at(5);
    fs::create_dir(store.dir().join(&empty)).unwrap();
    let list = store.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].session_id.as_str(), SESSION);
}
