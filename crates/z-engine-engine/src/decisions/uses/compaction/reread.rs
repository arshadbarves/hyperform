//! Rereads: the agent runs a call whose result `decisions_compaction`
//! cleared on the model's say (the same file read, or the same input). Each
//! one is a trace record with outcome `reread`, the cost of a wrong clear.

use serde_json::Value;
use z_engine_context::compaction::call_paths;
use z_engine_decisions::{AbstainReason, Answer};
use z_engine_tools::names;

use super::advise::QUESTION;
use crate::batch::ToolCall;
use crate::decisions::context::UseContext;

/// What makes two calls fetch the same thing: a read's file, else the
/// whole input.
pub(super) fn reread_key(name: &str, input: &Value) -> String {
    match call_paths(input).first() {
        Some(path) if name == names::READ => format!("{name} {path}"),
        _ => format!("{name} {input}"),
    }
}

/// Records a reread when `call` fetches a result cleared on the model's say.
pub(super) fn observe(cx: &UseContext, call: &ToolCall) {
    let key = reread_key(&call.name, &call.input);
    let memory = cx.core.decisions.compaction();
    let Some(cleared) = memory.take_reread(&key) else {
        return;
    };
    let observed = Answer {
        abstain: None,
        cached: true,
        ..Answer::abstained(QUESTION, AbstainReason::Rules, "engine")
    };
    let record = cx.record_of(&observed, cleared.as_str()).outcome("reread");
    cx.record(record);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_match_by_file_and_other_calls_by_input() {
        let read = reread_key("Read", &json!({"file_path": "/r/a.rs", "offset": 10}));
        assert_eq!(read, reread_key("Read", &json!({"file_path": "/r/a.rs"})));
        let grep = reread_key("Grep", &json!({"pattern": "x", "path": "/r"}));
        assert_ne!(
            grep,
            reread_key("Grep", &json!({"pattern": "y", "path": "/r"}))
        );
    }
}
