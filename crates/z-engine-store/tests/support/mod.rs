//! Shared fixtures for the store integration tests.
#![allow(dead_code)]

pub mod records;
pub mod v1;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use z_engine_protocol::{CallId, ContentBlock, Message, PermissionMode, SessionId};
use z_engine_store::{NewSession, SessionStore};

pub const SESSION: &str = "01J8Z3K4M5N6P7Q8R9S0T1V2W3";

pub fn temp_store() -> (tempfile::TempDir, SessionStore) {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = SessionStore::new(dir.path().join("sessions"));
    (dir, store)
}

pub fn new_session(id: &str) -> NewSession {
    NewSession {
        session_id: SessionId::from(id),
        project_root: "/work/app".into(),
        model: "claude-sonnet-4".into(),
        mode: PermissionMode::Default,
        planning: false,
    }
}

/// A ULID session id created at `ms`.
pub fn ulid_at(ms: u64) -> String {
    ulid::Ulid::from_parts(ms, 42).to_string()
}

pub fn append_raw(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open for raw append");
    file.write_all(bytes).expect("raw append");
}

pub fn set_mtime(path: &Path, ms: u64) {
    let file = fs::File::options()
        .write(true)
        .open(path)
        .expect("open for mtime");
    file.set_modified(UNIX_EPOCH + Duration::from_millis(ms))
        .expect("set mtime");
}

/// Every tool call is answered, in order, by the next message, and every
/// tool result answers the previous message: what providers require.
pub fn assert_tool_rounds_complete(messages: &[Message]) {
    for (index, message) in messages.iter().enumerate() {
        let results: Vec<&CallId> = message
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id),
                _ => None,
            })
            .collect();
        if !results.is_empty() {
            let previous = &messages[index.checked_sub(1).expect("results need a call")];
            let calls: Vec<&CallId> = previous.tool_uses().map(|(id, _, _)| id).collect();
            assert_eq!(
                results, calls,
                "results must answer the previous calls in order"
            );
        }
        if message.has_tool_use() {
            let next = messages.get(index + 1).expect("tool calls need results");
            assert!(
                next.is_tool_results(),
                "tool calls must be followed by results"
            );
        }
    }
}
