//! v1 transcript fixtures in the exact serde shapes the v1 app wrote.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Write a v1 transcript `<id>.jsonl`, one JSON value per line.
pub fn write_v1(dir: &Path, id: &str, events: &[Value]) -> PathBuf {
    let lines: Vec<String> = events.iter().map(Value::to_string).collect();
    write_v1_lines(dir, id, &lines)
}

pub fn write_v1_lines(dir: &Path, id: &str, lines: &[String]) -> PathBuf {
    fs::create_dir_all(dir).expect("sessions dir");
    let path = dir.join(format!("{id}.jsonl"));
    let mut text = lines.join("\n");
    text.push('\n');
    fs::write(&path, text).expect("write v1 file");
    path
}

/// A v1 `TaskReport` in its camelCase wire shape.
pub fn v1_report(status: &str) -> Value {
    json!({
        "schemaVersion": 1,
        "taskId": "task-1",
        "goal": "Fix the failing login test",
        "workspaceRoot": "/work/app",
        "status": status,
        "requirements": [{"id": "r1", "description": "login test passes"}],
        "checks": [],
        "assessment": null,
        "blockers": [],
        "changedPaths": []
    })
}

/// A v1 session as the v1 desktop app wrote it: an image prompt, a tool
/// round whose results arrive out of order around task reports, a title,
/// a note, an aborted approval (no results) followed by "continue", and a
/// trailing delegated round that never got its result.
pub fn v1_session_events() -> Vec<Value> {
    let mut complete = v1_report("complete");
    complete["checks"] = json!([{
        "id": "chk-1",
        "spec": {"kind": "cargo_test", "package": null, "filter": "login"},
        "command": ["cargo", "test", "login"],
        "cwd": "/work/app",
        "inputFingerprint": "abc",
        "toolchain": "rustc 1.80.0",
        "startedAtMs": 1,
        "durationMs": 2,
        "exitCode": 0,
        "testsRun": 3,
        "outcome": "passed",
        "summary": "3 tests passed",
        "stdout": null,
        "stderr": null
    }]);
    complete["changedPaths"] = json!(["src/login.rs"]);
    vec![
        json!({"type": "meta", "model": "anthropic/claude-sonnet-4", "project_root": "/work/app"}),
        json!({"type": "user_msg", "text": "Fix the failing login test\nIt started yesterday",
               "images": ["data:image/png;base64,iVBORw0KGgo="]}),
        json!({"type": "task_updated", "report": v1_report("running")}),
        json!({"type": "assistant_msg", "content": "Let me look.", "tool_calls": [
            {"id": "call_a", "name": "read_file", "arguments": "{\"path\":\"src/login.rs\"}"},
            {"id": "call_b", "name": "bash", "arguments": "{\"command\":\"cargo test login\"}"}
        ]}),
        json!({"type": "tool_result", "tool_call_id": "call_b", "content": "test login ... FAILED"}),
        json!({"type": "task_updated", "report": v1_report("needs_verification")}),
        json!({"type": "title", "text": "Fix failing login test"}),
        json!({"type": "tool_result", "tool_call_id": "call_a", "content": "fn login() {}"}),
        json!({"type": "assistant_msg", "content": "Fixed the assertion."}),
        json!({"type": "turn_end", "outcome": "completed"}),
        json!({"type": "task_updated", "report": complete}),
        json!({"type": "ack"}),
        json!({"type": "note", "text": "FACTS: login uses bcrypt"}),
        json!({"type": "user_msg", "text": "Now search for other callers"}),
        json!({"type": "assistant_msg", "tool_calls": [
            {"id": "call_c", "name": "grep", "arguments": "{not json"}
        ]}),
        json!({"type": "turn_end", "outcome": "aborted"}),
        json!({"type": "task_updated", "report": v1_report("stopped")}),
        json!({"type": "future_event", "payload": 1}),
        json!({"type": "user_msg", "text": "continue"}),
        json!({"type": "assistant_msg", "content": "Delegating.", "tool_calls": [
            {"id": "call_d", "name": "task", "arguments": "{\"prompt\":\"find callers\"}"}
        ]}),
        json!({"type": "turn_end", "outcome": "failed"}),
    ]
}
