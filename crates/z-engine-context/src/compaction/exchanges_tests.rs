use serde_json::json;
use z_engine_protocol::{CallId, ContentBlock, Message, Role};

use super::*;
use crate::compaction::summary_message;

pub(crate) fn call(id: &str, name: &str, input: Value) -> Message {
    Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse {
            id: CallId::from(id),
            name: name.into(),
            input,
        }],
    )
}

pub(crate) fn result(id: &str, text: &str, is_error: bool) -> Message {
    Message::new(
        Role::User,
        vec![ContentBlock::tool_result(CallId::from(id), text, is_error)],
    )
}

fn history() -> Vec<Message> {
    vec![
        Message::user_text("Fix src/a.rs\n<system-reminder>\nnote\n</system-reminder>"),
        call("r1", "Read", json!({"file_path": "/repo/src/a.rs"})),
        result("r1", "fn a() {}", false),
        call("e1", "Edit", json!({"file_path": "/repo/src/a.rs"})),
        result("e1", "ok", false),
        call("b1", "Bash", json!({"command": "cargo test"})),
        result("b1", "failed", true),
        call("b2", "Bash", json!({"command": "cargo test"})),
        result("b2", "ok", false),
        Message::assistant_text("Fixed it. All tests pass."),
        Message::user_text("Now run the linter"),
        call("b3", "Bash", json!({"command": "cargo clippy"})),
        result("b3", "warning", true),
        Message::assistant_text("Clippy reports one warning"),
    ]
}

#[test]
fn splits_at_real_user_turns() {
    let messages = history();
    let all = exchanges(&messages);
    let spans: Vec<(usize, usize, bool)> = all.iter().map(|e| (e.start, e.end, e.lead)).collect();
    assert_eq!(spans, [(0, 10, false), (10, 14, false)]);
    assert_eq!(all[0].request, "Fix src/a.rs");
    assert_eq!(all[0].files_edited, ["/repo/src/a.rs"]);
    assert_eq!(all[0].files_read, ["/repo/src/a.rs"]);
    assert_eq!(all[0].tools[2], ("Bash".to_string(), 2));
    assert_eq!(all[0].checks, [("cargo test".to_string(), false)]);
    assert_eq!(all[0].reply, "All tests pass.");
    assert_eq!(all[1].checks, [("cargo clippy".to_string(), true)]);
}

#[test]
fn the_summary_and_its_tail_form_the_lead() {
    let messages = vec![
        summary_message("earlier work"),
        Message::assistant_text("Continuing."),
        Message::user_text("next"),
    ];
    let all = exchanges(&messages);
    assert!(all[0].lead && all[0].start == 0 && all[0].end == 2);
    assert!(!all[1].lead && all[1].request == "next");
}

#[test]
fn messages_before_the_first_user_turn_are_the_lead() {
    let messages = vec![Message::assistant_text("tail"), Message::user_text("next")];
    let all = exchanges(&messages);
    assert_eq!(all.len(), 2);
    assert!(all[0].lead);
    assert!(exchanges(&[]).is_empty());
}

#[test]
fn digests_are_relative_and_stable() {
    let messages = history();
    let first = exchanges(&messages)[0].digest("/repo/");
    assert_eq!(first, exchanges(&messages)[0].digest("/repo/"));
    assert_eq!(first["files_edited"], json!(["src/a.rs"]));
    assert_eq!(first["tools"], "Read, Edit, Bash x2");
    assert_eq!(first["failed_checks"], json!([]));
    let long = Exchange {
        request: "x".repeat(5_000),
        ..exchanges(&messages)[1].clone()
    };
    assert!(long.digest("/repo").to_string().len() < 4_000);
}

#[test]
fn relative_keeps_paths_outside_the_root() {
    assert_eq!(relative("/repo/a.rs", "/repo"), "a.rs");
    assert_eq!(relative("/repository/a.rs", "/repo"), "/repository/a.rs");
    assert_eq!(relative("a.rs", ""), "a.rs");
}
