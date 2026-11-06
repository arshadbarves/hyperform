//! Plan mode and the permission level replay from logs written before
//! planning was its own switch (`mode: "plan"`), after it, and from logs
//! that mix both.

mod support;

use std::fs;

use support::{SESSION, new_session, temp_store};
use z_engine_protocol::{PermissionMode, SessionId};
use z_engine_store::{LogRecord, ReplayState, replay};

fn records(lines: &[&str]) -> Vec<LogRecord> {
    lines
        .iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn state(lines: &[&str]) -> (PermissionMode, bool) {
    let ReplayState { mode, planning, .. } = replay(&records(lines));
    (mode, planning)
}

const STARTED_PLAN: &str = r#"{"kind":"sessionStarted","schema":2,"sessionId":"S1","projectRoot":"/p","model":"m","mode":"plan","createdAt":1}"#;
const STARTED_BYPASS: &str = r#"{"kind":"sessionStarted","schema":2,"sessionId":"S1","projectRoot":"/p","model":"m","mode":"bypass","createdAt":1}"#;
const OLD_TO_PLAN: &str = r#"{"kind":"modeChanged","mode":"plan"}"#;
const OLD_TO_ACCEPT: &str = r#"{"kind":"modeChanged","mode":"acceptEdits"}"#;

#[test]
fn an_old_log_started_in_plan_mode_resumes_planning_at_ask() {
    assert_eq!(state(&[STARTED_PLAN]), (PermissionMode::Default, true));
}

#[test]
fn old_switches_into_and_out_of_plan_mode_toggle_planning() {
    assert_eq!(
        state(&[STARTED_BYPASS, OLD_TO_PLAN]),
        (PermissionMode::Default, true)
    );
    assert_eq!(
        state(&[STARTED_BYPASS, OLD_TO_PLAN, OLD_TO_ACCEPT]),
        (PermissionMode::AcceptEdits, false)
    );
}

#[test]
fn new_level_changes_keep_planning_and_planning_changes_keep_the_level() {
    let on = r#"{"kind":"planningChanged","planning":true}"#;
    let level = r#"{"kind":"modeChanged","mode":"acceptEdits","planning":true}"#;
    let off = r#"{"kind":"planningChanged","planning":false}"#;
    assert_eq!(state(&[STARTED_BYPASS, on]), (PermissionMode::Bypass, true));
    assert_eq!(
        state(&[STARTED_BYPASS, on, level]),
        (PermissionMode::AcceptEdits, true)
    );
    assert_eq!(
        state(&[STARTED_BYPASS, on, level, off]),
        (PermissionMode::AcceptEdits, false)
    );
}

#[test]
fn a_log_continued_after_the_update_keeps_its_old_plan_mode() {
    let level = r#"{"kind":"modeChanged","mode":"bypass","planning":true}"#;
    assert_eq!(
        state(&[STARTED_PLAN, level]),
        (PermissionMode::Bypass, true)
    );
}

#[test]
fn writers_record_a_level_and_the_planning_switch() {
    let started = serde_json::to_value(LogRecord::SessionStarted {
        schema: 2,
        session_id: SessionId::from(SESSION),
        project_root: "/p".into(),
        model: "m".into(),
        mode: PermissionMode::Bypass.into(),
        planning: true,
        created_at: 1,
    })
    .unwrap();
    assert_eq!(
        (&started["mode"], &started["planning"]),
        (&"bypass".into(), &true.into())
    );
    let changed = serde_json::to_value(LogRecord::ModeChanged {
        mode: PermissionMode::AcceptEdits.into(),
        planning: Some(false),
    })
    .unwrap();
    assert_eq!(changed["planning"], false);
}

#[test]
fn a_stored_old_log_loads_in_plan_mode() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    let (log, _) = store.create(new_session(SESSION)).unwrap();
    let text = format!(
        "{}\n{OLD_TO_PLAN}\n",
        STARTED_BYPASS.replace("\"S1\"", &format!("\"{SESSION}\""))
    );
    fs::write(log.path(), text).unwrap();
    let loaded = store.load(&id).unwrap();
    assert_eq!(loaded.corrupt_lines, 0);
    assert_eq!(loaded.state.mode, PermissionMode::Default);
    assert!(loaded.state.planning);
}
