//! Whole exchanges for the task view: a real user turn and everything up
//! to the next one, never separating a tool call from its result. Digests
//! are built from the messages alone, so the same history always gives the
//! same digest.

use std::collections::HashMap;

use serde_json::{Value, json};
use z_engine_protocol::{CallId, ContentBlock, Message, Role};

use super::summary::{crossed_splits, is_split_point, is_summary};
use super::task_view::is_task_view_index;
use super::touch::{call_command, call_paths, is_check, is_edit, request_text};
use super::transcript::truncate;

const MAX_LIST: usize = 12;
const MAX_COMMANDS: usize = 6;
const MAX_REQUEST_CHARS: usize = 600;
const MAX_REPLY_CHARS: usize = 300;
const MAX_COMMAND_CHARS: usize = 120;

/// One exchange, `messages[start..end]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exchange {
    pub start: usize,
    pub end: usize,
    /// The messages before the first real user turn: the compaction
    /// summary and its verbatim tail, if any. Never set aside.
    pub lead: bool,
    /// The opening user text without system reminders.
    pub request: String,
    pub files_edited: Vec<String>,
    pub files_read: Vec<String>,
    /// Tool names with call counts, in first-use order.
    pub tools: Vec<(String, usize)>,
    /// Bash command lines, in order.
    pub commands: Vec<String>,
    /// The last run of each check in this exchange: (check key, failed).
    pub checks: Vec<(String, bool)>,
    /// The last sentence of the last assistant text.
    pub reply: String,
}

/// The exchanges of `messages`, in order, covering every message.
pub fn exchanges(messages: &[Message]) -> Vec<Exchange> {
    let crossed = crossed_splits(messages);
    let mut starts: Vec<usize> = (0..messages.len())
        .filter(|&index| {
            let message = &messages[index];
            message.role == Role::User
                && is_split_point(message)
                && !crossed[index]
                && !is_summary(message)
                && !is_task_view_index(message)
        })
        .collect();
    let lead = starts.first() != Some(&0) && !messages.is_empty();
    if lead {
        starts.insert(0, 0);
    }
    let names: HashMap<&CallId, (&str, &Value)> = messages
        .iter()
        .flat_map(Message::tool_uses)
        .map(|(id, name, input)| (id, (name, input)))
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(position, &start)| {
            let end = starts.get(position + 1).copied().unwrap_or(messages.len());
            let mut exchange = digest(&messages[start..end], &names);
            exchange.start = start;
            exchange.end = end;
            exchange.lead = lead && position == 0;
            exchange
        })
        .collect()
}

fn digest(messages: &[Message], names: &HashMap<&CallId, (&str, &Value)>) -> Exchange {
    let mut exchange = Exchange {
        start: 0,
        end: 0,
        lead: false,
        request: messages.first().map(request_text).unwrap_or_default(),
        files_edited: Vec::new(),
        files_read: Vec::new(),
        tools: Vec::new(),
        commands: Vec::new(),
        checks: Vec::new(),
        reply: String::new(),
    };
    for message in messages {
        for block in &message.content {
            match block {
                ContentBlock::ToolUse { name, input, .. } => exchange.call(name, input),
                ContentBlock::ToolResult {
                    tool_use_id,
                    is_error,
                    ..
                } => {
                    if let Some((name, input)) = names.get(tool_use_id) {
                        exchange.check_result(name, input, *is_error);
                    }
                }
                ContentBlock::Text { text } if message.role == Role::Assistant => {
                    if let Some(sentence) = last_sentence(text) {
                        exchange.reply = sentence;
                    }
                }
                _ => {}
            }
        }
    }
    exchange
}

impl Exchange {
    fn call(&mut self, name: &str, input: &Value) {
        match self.tools.iter_mut().find(|(seen, _)| seen == name) {
            Some((_, count)) => *count += 1,
            None => self.tools.push((name.to_string(), 1)),
        }
        let files = if is_edit(name) {
            &mut self.files_edited
        } else {
            &mut self.files_read
        };
        for path in call_paths(input) {
            if !files.contains(&path) {
                files.push(path);
            }
        }
        if let Some(command) = call_command(input) {
            self.commands.push(command.trim().to_string());
        }
    }

    fn check_result(&mut self, name: &str, input: &Value, failed: bool) {
        if !is_check(name) {
            return;
        }
        let key = check_key(name, input);
        self.checks.retain(|(seen, _)| *seen != key);
        self.checks.push((key, failed));
    }

    /// Every file this exchange read or edited.
    pub fn files(&self) -> impl Iterator<Item = &String> {
        self.files_edited.iter().chain(&self.files_read)
    }

    /// The digest the decision model reads, with paths shown relative to
    /// `root`. Lists and texts are capped, so it stays under ~1,000 tokens.
    pub fn digest(&self, root: &str) -> Value {
        let paths = |paths: &[String]| -> Vec<String> {
            paths
                .iter()
                .take(MAX_LIST)
                .map(|path| relative(path, root))
                .collect()
        };
        let tools: Vec<String> = self
            .tools
            .iter()
            .take(MAX_LIST)
            .map(|(name, count)| match count {
                1 => name.clone(),
                _ => format!("{name} x{count}"),
            })
            .collect();
        let commands: Vec<String> = self
            .commands
            .iter()
            .take(MAX_COMMANDS)
            .map(|command| truncate(command, MAX_COMMAND_CHARS))
            .collect();
        let failed: Vec<&str> = self
            .checks
            .iter()
            .filter(|(_, failed)| *failed)
            .map(|(key, _)| key.as_str())
            .take(MAX_COMMANDS)
            .collect();
        json!({
            "request": truncate(&self.request, MAX_REQUEST_CHARS),
            "files_edited": paths(&self.files_edited),
            "files_read": paths(&self.files_read),
            "tools": tools.join(", "),
            "commands": commands,
            "failed_checks": failed,
            "reply": truncate(&self.reply, MAX_REPLY_CHARS),
        })
    }
}

/// Identifies one check across runs: the Bash command line, or the
/// Verify input.
pub(super) fn check_key(name: &str, input: &Value) -> String {
    match call_command(input) {
        Some(command) => truncate(command.trim(), MAX_COMMAND_CHARS),
        None => format!("{name} {input}"),
    }
}

/// `path` without the `root` prefix.
pub fn relative(path: &str, root: &str) -> String {
    let root = root.trim_end_matches('/');
    path.strip_prefix(root)
        .filter(|_| !root.is_empty())
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(path)
        .to_string()
}

fn last_sentence(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let body = text.trim_end_matches(['.', '!', '?']);
    let start = body
        .rfind(['.', '!', '?', '\n'])
        .map_or(0, |index| index + 1);
    Some(crate::text::single_line(&text[start..]))
}

#[cfg(test)]
#[path = "exchanges_tests.rs"]
pub(super) mod tests;
