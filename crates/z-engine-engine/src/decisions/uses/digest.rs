//! Small deterministic digests of the main agent's conversation, sent as
//! decision state: the user's latest request, the agent's latest words,
//! what the current turn ran, and one line per tool input. Never whole
//! files: the decision model reads a few hundred tokens.

use serde_json::{Value, json};
use z_engine_protocol::{ContentBlock, Message, Role};
use z_engine_tools::names;

use crate::batch::ToolCall;

const REMINDER_OPEN: &str = "<system-reminder>";
const REQUEST_CHARS: usize = 600;
const EXPLANATION_CHARS: usize = 200;

/// The text of the newest user message that is not a tool round, without
/// the reminders the engine appended.
pub(super) fn latest_request(working: &[Message]) -> String {
    working
        .iter()
        .rev()
        .filter(|message| message.role == Role::User && !has_results(message))
        .map(user_text)
        .find(|text| !text.trim().is_empty())
        .unwrap_or_default()
}

/// The text of the newest assistant message that has any.
pub(super) fn latest_assistant_text(working: &[Message]) -> String {
    working
        .iter()
        .rev()
        .filter(|message| message.role == Role::Assistant)
        .map(Message::text)
        .find(|text| !text.trim().is_empty())
        .unwrap_or_default()
}

/// One line per tool call since the latest user request, oldest first:
/// `Bash: cargo test (error)`.
pub(super) fn turn_calls(working: &[Message], limit: usize) -> Vec<String> {
    let start = working
        .iter()
        .rposition(|message| message.role == Role::User && !has_results(message))
        .map_or(0, |at| at + 1);
    let turn = &working[start..];
    let failed = |id: &str| {
        turn.iter().flat_map(|m| &m.content).any(|block| {
            matches!(block, ContentBlock::ToolResult { tool_use_id, is_error: true, .. }
                if tool_use_id.as_str() == id)
        })
    };
    let mut calls: Vec<String> = turn
        .iter()
        .flat_map(Message::tool_uses)
        .map(|(id, name, input)| {
            let status = if failed(id.as_str()) { " (error)" } else { "" };
            format!("{name}: {}{status}", input_digest(name, input))
        })
        .collect();
    let skip = calls.len().saturating_sub(limit);
    calls.drain(..skip);
    calls
}

/// A call about to run, as decision state: the user's latest request, the
/// call, and the last line the agent wrote before it (usually its own
/// explanation).
pub(super) fn call_state(working: &[Message], call: &ToolCall) -> Value {
    json!({
        "request": head(&latest_request(working), REQUEST_CHARS),
        "tool": call.name,
        "input": input_digest(&call.name, &call.input),
        "explanation": last_line(&latest_assistant_text(working), EXPLANATION_CHARS),
    })
}

/// The part of a tool input a person would read first: the command, the
/// path, the URL or query, else the compact JSON.
pub(super) fn input_digest(name: &str, input: &Value) -> String {
    let field = |key: &str| input.get(key).and_then(Value::as_str);
    let main = match name {
        names::BASH => field("command"),
        names::WEB_FETCH => field("url"),
        names::WEB_SEARCH => field("query"),
        _ => field("file_path").or_else(|| field("path")),
    };
    let text = main.map_or_else(|| input.to_string(), str::to_string);
    head(&text, 300)
}

/// The last non-empty line of `text`, at most `chars` characters.
pub(super) fn last_line(text: &str, chars: usize) -> String {
    let line = text.lines().rev().find(|line| !line.trim().is_empty());
    head(line.unwrap_or_default().trim(), chars)
}

/// The first `chars` characters of `text`, with an ellipsis when cut.
pub(super) fn head(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_string(),
    }
}

/// The last `chars` characters of `text`, with an ellipsis when cut.
pub(super) fn tail(text: &str, chars: usize) -> String {
    let count = text.chars().count();
    if count <= chars {
        return text.to_string();
    }
    let at = text
        .char_indices()
        .nth(count - chars)
        .map_or(0, |(at, _)| at);
    format!("…{}", &text[at..])
}

fn has_results(message: &Message) -> bool {
    let mut blocks = message.content.iter();
    blocks.any(|block| matches!(block, ContentBlock::ToolResult { .. }))
}

fn user_text(message: &Message) -> String {
    let texts = message.content.iter().filter_map(|block| match block {
        ContentBlock::Text { text } if !text.trim_start().starts_with(REMINDER_OPEN) => {
            Some(text.as_str())
        }
        _ => None,
    });
    texts.collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_protocol::CallId;

    use super::*;

    fn conversation() -> Vec<Message> {
        let call = ContentBlock::ToolUse {
            id: CallId::from("c1"),
            name: "Bash".into(),
            input: json!({ "command": "cargo test" }),
        };
        vec![
            Message::user_text("old request"),
            Message::assistant_text("done before"),
            Message::new(
                Role::User,
                vec![
                    ContentBlock::text("fix the parser"),
                    ContentBlock::text("<system-reminder>\ntodo\n</system-reminder>"),
                ],
            ),
            Message::new(
                Role::Assistant,
                vec![
                    ContentBlock::text("Running the tests.\nThen I fix it."),
                    call,
                ],
            ),
            Message::new(
                Role::User,
                vec![ContentBlock::tool_result(
                    CallId::from("c1"),
                    "1 failed",
                    true,
                )],
            ),
        ]
    }

    #[test]
    fn digests_find_the_request_the_agent_words_and_the_turn_calls() {
        let working = conversation();
        assert_eq!(latest_request(&working), "fix the parser");
        assert_eq!(
            latest_assistant_text(&working),
            "Running the tests.\nThen I fix it."
        );
        assert_eq!(turn_calls(&working, 8), ["Bash: cargo test (error)"]);
        assert!(turn_calls(&working, 0).is_empty());
        assert_eq!(last_line("a\nb\n\n", 10), "b");
    }

    #[test]
    fn cuts_are_char_safe_and_marked() {
        assert_eq!(head("héllo", 2), "hé…");
        assert_eq!(head("hi", 5), "hi");
        assert_eq!(tail("héllo", 2), "…lo");
        assert_eq!(
            input_digest("Edit", &json!({ "file_path": "a.rs" })),
            "a.rs"
        );
        assert_eq!(input_digest("X", &json!({ "n": 1 })), "{\"n\":1}");
    }
}
