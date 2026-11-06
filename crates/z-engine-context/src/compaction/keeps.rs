//! Tool results compaction keeps whatever the decision model says: results
//! about files the user named in the current request, the latest failing
//! check output, and results about files edited after them.

use std::collections::HashMap;

use serde_json::Value;
use z_engine_protocol::{CallId, ContentBlock, Message};

use super::touch::{call_paths, is_check, is_edit, names_path};

/// Why a tool result is kept regardless of the decision model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepReason {
    NamedFile,
    FailingCheck,
    EditedSince,
}

impl KeepReason {
    /// Short reason recorded as the override in decision traces.
    pub fn label(self) -> &'static str {
        match self {
            Self::NamedFile => "file named by the user",
            Self::FailingCheck => "latest failing check output",
            Self::EditedSince => "file edited since",
        }
    }
}

struct Call<'a> {
    name: &'a str,
    input: &'a Value,
}

/// The protected tool results of `messages`, keyed by call id. `named`
/// are the paths the current request mentions (see `named_paths`).
pub fn protected_results(messages: &[Message], named: &[String]) -> HashMap<CallId, KeepReason> {
    let mut calls: HashMap<&CallId, Call> = HashMap::new();
    let mut edited_at: HashMap<String, usize> = HashMap::new();
    for (index, message) in messages.iter().enumerate() {
        for (id, name, input) in message.tool_uses() {
            calls.insert(id, Call { name, input });
            if is_edit(name) {
                for path in call_paths(input) {
                    edited_at.insert(path, index);
                }
            }
        }
    }
    let mut protected = HashMap::new();
    let mut failing: Option<&CallId> = None;
    for (index, message) in messages.iter().enumerate() {
        for block in &message.content {
            let ContentBlock::ToolResult {
                tool_use_id,
                is_error,
                ..
            } = block
            else {
                continue;
            };
            let Some(call) = calls.get(tool_use_id) else {
                continue;
            };
            if *is_error && is_check(call.name) {
                failing = Some(tool_use_id);
            }
            let paths = call_paths(call.input);
            if paths.iter().any(|path| names_path(named, path)) {
                protected.insert(tool_use_id.clone(), KeepReason::NamedFile);
            } else if paths
                .iter()
                .any(|path| edited_at.get(path).is_some_and(|at| *at > index))
            {
                protected.insert(tool_use_id.clone(), KeepReason::EditedSince);
            }
        }
    }
    if let Some(id) = failing {
        protected.insert(id.clone(), KeepReason::FailingCheck);
    }
    protected
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_protocol::Role;

    use super::*;

    fn call(id: &str, name: &str, input: Value) -> Message {
        Message::new(
            Role::Assistant,
            vec![ContentBlock::ToolUse {
                id: CallId::from(id),
                name: name.into(),
                input,
            }],
        )
    }

    fn result(id: &str, is_error: bool) -> Message {
        Message::new(
            Role::User,
            vec![ContentBlock::tool_result(CallId::from(id), "out", is_error)],
        )
    }

    fn round(id: &str, name: &str, input: Value, is_error: bool) -> [Message; 2] {
        [call(id, name, input), result(id, is_error)]
    }

    #[test]
    fn keeps_named_files_the_latest_failure_and_files_edited_since() {
        let messages: Vec<Message> = [
            round("r1", "Read", json!({"file_path": "/r/src/auth.rs"}), false),
            round("r2", "Read", json!({"file_path": "/r/src/db.rs"}), false),
            round("r3", "Read", json!({"file_path": "/r/src/ui.rs"}), false),
            round("b1", "Bash", json!({"command": "cargo test"}), true),
            round("e1", "Edit", json!({"file_path": "/r/src/db.rs"}), false),
            round("b2", "Bash", json!({"command": "cargo test"}), true),
            round("g1", "Grep", json!({"pattern": "x"}), true),
        ]
        .into_iter()
        .flatten()
        .collect();
        let protected = protected_results(&messages, &["auth.rs".to_string()]);
        assert_eq!(
            protected.get(&CallId::from("r1")),
            Some(&KeepReason::NamedFile)
        );
        assert_eq!(
            protected.get(&CallId::from("r2")),
            Some(&KeepReason::EditedSince)
        );
        assert_eq!(
            protected.get(&CallId::from("b2")),
            Some(&KeepReason::FailingCheck)
        );
        for open in ["r3", "b1", "e1", "g1"] {
            assert!(!protected.contains_key(&CallId::from(open)), "{open}");
        }
    }
}
