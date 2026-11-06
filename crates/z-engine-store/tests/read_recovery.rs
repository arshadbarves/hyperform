//! Reading tolerates crashes and corruption without touching the file, and
//! reopening for append leaves the log at a clean line boundary.

mod support;

use std::path::PathBuf;

use support::{SESSION, append_raw, new_session, temp_store};
use z_engine_protocol::SessionId;
use z_engine_store::{LogRecord, SessionStore, StoreError, read_records};

fn note(text: &str) -> LogRecord {
    LogRecord::Note { text: text.into() }
}

fn session_with_notes(texts: &[&str]) -> (tempfile::TempDir, SessionStore, PathBuf) {
    let (dir, store) = temp_store();
    let (mut log, _) = store.create(new_session(SESSION)).unwrap();
    for text in texts {
        log.append(&note(text)).unwrap();
    }
    let path = log.path().to_path_buf();
    (dir, store, path)
}

#[test]
fn torn_final_line_is_ignored_and_reported() {
    let (_dir, _store, path) = session_with_notes(&["one", "two"]);
    append_raw(&path, br#"{"kind":"note","text":"thr"#);
    let read = read_records(&path).unwrap();
    assert!(read.torn_tail);
    assert_eq!(read.corrupt_lines, 0);
    assert_eq!(read.records[1..], [note("one"), note("two")]);
}

#[test]
fn any_unterminated_invalid_final_line_is_a_torn_tail() {
    let (_dir, _store, path) = session_with_notes(&["one"]);
    append_raw(&path, br#"{"kind":"note",]"#);
    let read = read_records(&path).unwrap();
    assert!(read.torn_tail);
    assert_eq!(read.corrupt_lines, 0);
    assert_eq!(read.records[1..], [note("one")]);
}

#[test]
fn corrupt_and_unknown_lines_mid_file_are_counted_and_skipped() {
    let (_dir, store, path) = session_with_notes(&["one"]);
    append_raw(
        &path,
        b"this is not json\n\n{\"kind\":\"futureRecord\",\"x\":1}\n{\"kind\":\"note\"}\n",
    );
    let mut log = store.open_append(&SessionId::from(SESSION)).unwrap();
    log.append(&note("two")).unwrap();
    let read = read_records(&path).unwrap();
    assert!(!read.torn_tail);
    assert_eq!(read.corrupt_lines, 3);
    assert_eq!(read.records[1..], [note("one"), note("two")]);
}

#[test]
fn complete_record_without_its_newline_is_kept() {
    let (_dir, _store, path) = session_with_notes(&["one"]);
    append_raw(&path, br#"{"kind":"note","text":"two"}"#);
    let read = read_records(&path).unwrap();
    assert!(!read.torn_tail);
    assert_eq!(read.corrupt_lines, 0);
    assert_eq!(read.records[1..], [note("one"), note("two")]);
}

#[test]
fn unknown_kind_on_the_unterminated_final_line_is_corrupt_not_torn() {
    let (_dir, _store, path) = session_with_notes(&["one"]);
    append_raw(&path, br#"{"kind":"futureRecord"}"#);
    let read = read_records(&path).unwrap();
    assert!(!read.torn_tail);
    assert_eq!(read.corrupt_lines, 1);
}

#[test]
fn reading_never_modifies_the_file() {
    let (_dir, _store, path) = session_with_notes(&["one"]);
    append_raw(&path, b"garbage\n{\"kind\":\"note\",\"te");
    let before = std::fs::read(&path).unwrap();
    let read = read_records(&path).unwrap();
    assert!(read.torn_tail);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn reopening_cuts_a_torn_tail_before_appending() {
    let (_dir, store, path) = session_with_notes(&["one"]);
    append_raw(&path, br#"{"kind":"note","text":"lo"#);
    let mut log = store.open_append(&SessionId::from(SESSION)).unwrap();
    log.append(&note("two")).unwrap();
    log.sync().unwrap();
    let read = read_records(&path).unwrap();
    assert!(!read.torn_tail);
    assert_eq!(read.corrupt_lines, 0);
    assert_eq!(read.records[1..], [note("one"), note("two")]);
}

#[test]
fn reopening_terminates_a_complete_unterminated_record() {
    let (_dir, store, path) = session_with_notes(&["one"]);
    append_raw(&path, br#"{"kind":"note","text":"two"}"#);
    let mut log = store.open_append(&SessionId::from(SESSION)).unwrap();
    log.append(&note("three")).unwrap();
    let read = read_records(&path).unwrap();
    assert_eq!(read.corrupt_lines, 0);
    assert_eq!(read.records[1..], [note("one"), note("two"), note("three")]);
}

#[test]
fn load_reports_torn_tail_and_corrupt_lines() {
    let (_dir, store, path) = session_with_notes(&["one"]);
    append_raw(&path, b"not json\n{\"kind\":\"note\",\"te");
    let loaded = store.load(&SessionId::from(SESSION)).unwrap();
    assert!(loaded.torn_tail);
    assert_eq!(loaded.corrupt_lines, 1);
    assert!(loaded.state.info.is_some());
}

#[test]
fn missing_log_is_not_found() {
    let (dir, _store) = temp_store();
    let missing = dir.path().join("missing.jsonl");
    assert!(matches!(
        read_records(&missing),
        Err(StoreError::NotFound(_))
    ));
}
