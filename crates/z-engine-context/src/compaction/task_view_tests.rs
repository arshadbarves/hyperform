use serde_json::json;
use z_engine_protocol::Message;

use super::*;
use crate::compaction::exchanges::exchanges;
use crate::compaction::exchanges::tests::{call, result};
use crate::compaction::summary_message;

/// A summary lead, then five exchanges: 1 edits a.rs, 2 runs a failing
/// check, 3 writes todos, 4 reads b.rs, 5 is plain.
fn history() -> Vec<Message> {
    vec![
        summary_message("earlier"),
        Message::user_text("edit a"),
        call("e1", "Edit", json!({"file_path": "/repo/src/a.rs"})),
        result("e1", "ok", false),
        Message::user_text("run tests"),
        call("b1", "Bash", json!({"command": "cargo test"})),
        result("b1", "failed", true),
        Message::user_text("plan it"),
        call("t1", "TodoWrite", json!({"todos": []})),
        result("t1", "ok", false),
        Message::user_text("read b"),
        call("r1", "Read", json!({"file_path": "/repo/src/b.rs"})),
        result("r1", "fn b() {}", false),
        Message::user_text("thanks"),
        Message::assistant_text("You're welcome."),
    ]
}

#[test]
fn hard_keeps_cover_lead_recent_named_failing_and_todos() {
    let messages = history();
    let all = exchanges(&messages);
    let keeps = task_keeps(&all, 1, &["a.rs".to_string()], true);
    assert_eq!(
        keeps,
        [
            Some(TaskKeep::Lead),
            Some(TaskKeep::NamedFile),
            Some(TaskKeep::FailingCheck),
            Some(TaskKeep::OpenTodos),
            None,
            Some(TaskKeep::Recent),
        ]
    );
    let closed = task_keeps(&all, 1, &[], false);
    assert_eq!(closed[1], None);
    assert_eq!(closed[3], None);
}

#[test]
fn a_later_passing_run_clears_the_failing_keep() {
    let mut messages = history();
    messages.splice(
        13..13,
        [
            call("b2", "Bash", json!({"command": "cargo test"})),
            result("b2", "ok", false),
        ],
    );
    let all = exchanges(&messages);
    assert_eq!(task_keeps(&all, 0, &[], false)[2], None);
}

#[test]
fn only_unneeded_exchanges_without_keeps_are_set_aside() {
    let keeps = [
        Some(TaskKeep::Lead),
        None,
        None,
        None,
        Some(TaskKeep::Recent),
    ];
    let needed = |index: usize| match index {
        1 => Some(false),
        2 => Some(true),
        3 => None,
        _ => Some(false),
    };
    let plan = plan_task_view(&keeps, needed, |_| false);
    assert_eq!(plan.set_aside, [1]);
    let pinned = plan_task_view(&keeps, |_| Some(false), |index| index == 3);
    assert_eq!(pinned.set_aside, [1, 2, 3]);
    assert_eq!(pinned.pinned, [3]);
}

#[test]
fn the_view_keeps_the_lead_then_the_index_then_kept_exchanges() {
    let messages = history();
    let all = exchanges(&messages);
    let plan = TaskViewPlan {
        set_aside: vec![1, 2, 4],
        pinned: vec![2],
    };
    let index = task_view_index(&["Turn 1: \"edit a\"".to_string()]);
    let view = apply_task_view(&messages, &all, &plan, index.clone());
    let texts: Vec<String> = view.iter().map(Message::text).collect();
    assert_eq!(view[0].id, messages[0].id);
    assert_eq!(view[1].id, index.id);
    assert_eq!(texts[2], "run tests");
    assert_eq!(view[3].id, messages[7].id);
    assert_eq!(view.last().unwrap().id, messages[14].id);
    assert_eq!(view.len(), 2 + 1 + 3 + 2);
    assert!(is_task_view_index(&view[1]));
    assert!(!is_task_view_index(&messages[1]));
}

#[test]
fn an_index_in_history_does_not_open_an_exchange() {
    let mut messages = history();
    messages.insert(1, task_view_index(&["Turn 1".to_string()]));
    assert_eq!(exchanges(&messages).len(), exchanges(&history()).len());
}

#[test]
fn index_lines_name_request_edits_failures_and_file() {
    let messages = history();
    let all = exchanges(&messages);
    let line = index_line(1, &all[1], "/repo", "/data/a1.txt");
    assert_eq!(
        line,
        "Turn 1: \"edit a\"; edited src/a.rs - full text at /data/a1.txt"
    );
    let failing = index_line(2, &all[2], "/repo", "/data/a2.txt");
    assert!(failing.contains("; check failed: cargo test"));
}

#[test]
fn rebuilds_pay_off_only_when_cold_or_large_and_small_enough() {
    assert!(worth_rebuilding(10_000, 9_000, true));
    assert!(!worth_rebuilding(10_000, 10_000, true));
    assert!(!worth_rebuilding(20_000, 1_000, false));
    assert!(worth_rebuilding(40_000, 24_000, false));
    assert!(!worth_rebuilding(40_000, 24_001, false));
}
