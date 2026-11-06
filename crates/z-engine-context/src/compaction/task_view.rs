//! The task view: when the user starts a new task, earlier exchanges it
//! does not need are set aside (the engine spills them whole to an
//! artifact) and one index message names them. Planning is pure; the
//! engine supplies the decision model's verdicts.

use z_engine_prompts::reminders::TASK_VIEW_INDEX;
use z_engine_protocol::Message;

use super::exchanges::{Exchange, relative};
use super::touch::names_path;
use crate::template::render_template;
use crate::text::single_line;

/// A view is only worth a prompt-cache miss over this much history...
pub const MIN_HISTORY_TOKENS: u64 = 30_000;
/// ...when it keeps at most this share of it.
pub const MAX_VIEW_PERCENT: u64 = 60;

const INDEX_REQUEST_CHARS: usize = 80;
const INDEX_FILES: usize = 4;

/// Why an exchange stays in the view whatever the model says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKeep {
    /// The compaction summary and its verbatim tail.
    Lead,
    Recent,
    NamedFile,
    FailingCheck,
    OpenTodos,
}

impl TaskKeep {
    /// Short reason recorded as the override in decision traces.
    pub fn label(self) -> &'static str {
        match self {
            Self::Lead => "latest summary",
            Self::Recent => "recent exchange",
            Self::NamedFile => "file named in the new request",
            Self::FailingCheck => "check still failing",
            Self::OpenTodos => "open todos",
        }
    }
}

/// The hard keep of each exchange: the lead, the last `keep_recent`
/// exchanges, exchanges touching a file `named` in the new request, the
/// exchange holding the latest run of a check that failed there, and,
/// while `todos_open`, the exchange that last wrote the todo list.
pub fn task_keeps(
    exchanges: &[Exchange],
    keep_recent: usize,
    named: &[String],
    todos_open: bool,
) -> Vec<Option<TaskKeep>> {
    let recent_from = exchanges.len().saturating_sub(keep_recent);
    let todo_writer = exchanges
        .iter()
        .rposition(|exchange| exchange.tools.iter().any(|(name, _)| name == "TodoWrite"))
        .filter(|_| todos_open);
    let failing = |index: usize| {
        exchanges[index].checks.iter().any(|(key, failed)| {
            *failed
                && !exchanges[index + 1..]
                    .iter()
                    .any(|later| later.checks.iter().any(|(seen, _)| seen == key))
        })
    };
    exchanges
        .iter()
        .enumerate()
        .map(|(index, exchange)| {
            if exchange.lead {
                Some(TaskKeep::Lead)
            } else if index >= recent_from {
                Some(TaskKeep::Recent)
            } else if exchange.files().any(|path| names_path(named, path)) {
                Some(TaskKeep::NamedFile)
            } else if failing(index) {
                Some(TaskKeep::FailingCheck)
            } else if todo_writer == Some(index) {
                Some(TaskKeep::OpenTodos)
            } else {
                None
            }
        })
        .collect()
}

/// Exchanges to set aside, by index into the exchange list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskViewPlan {
    pub set_aside: Vec<usize>,
    /// Set-aside exchanges whose opening message stays: a standing rule.
    pub pinned: Vec<usize>,
}

/// Sets aside the exchanges without a hard keep that the model judged not
/// `needed` (`Some(false)`); no verdict keeps the exchange.
pub fn plan_task_view(
    keeps: &[Option<TaskKeep>],
    needed: impl Fn(usize) -> Option<bool>,
    pinned: impl Fn(usize) -> bool,
) -> TaskViewPlan {
    let set_aside: Vec<usize> = (0..keeps.len())
        .filter(|&index| keeps[index].is_none() && needed(index) == Some(false))
        .collect();
    let pinned = set_aside
        .iter()
        .copied()
        .filter(|&index| pinned(index))
        .collect();
    TaskViewPlan { set_aside, pinned }
}

/// The view: the lead, then `index`, then every kept exchange whole and
/// the opening message of each pinned one, in order.
pub fn apply_task_view(
    messages: &[Message],
    exchanges: &[Exchange],
    plan: &TaskViewPlan,
    index: Message,
) -> Vec<Message> {
    let mut view = Vec::with_capacity(messages.len());
    let mut index = Some(index);
    for (position, exchange) in exchanges.iter().enumerate() {
        if !exchange.lead
            && let Some(index) = index.take()
        {
            view.push(index);
        }
        let range = exchange.start..exchange.end.min(messages.len());
        if !plan.set_aside.contains(&position) {
            view.extend_from_slice(&messages[range]);
        } else if plan.pinned.contains(&position) {
            view.extend(messages.get(exchange.start).cloned());
        }
    }
    view.extend(index);
    view
}

/// One index line: `Turn <number>: "<request>"; edited ...; check failed:
/// ... - full text at <artifact>`.
pub fn index_line(number: usize, exchange: &Exchange, root: &str, artifact: &str) -> String {
    let request = single_line(&exchange.request);
    let mut line = format!("Turn {number}: \"{}\"", clip(&request, INDEX_REQUEST_CHARS));
    if !exchange.files_edited.is_empty() {
        let files: Vec<String> = exchange
            .files_edited
            .iter()
            .take(INDEX_FILES)
            .map(|path| relative(path, root))
            .collect();
        line.push_str(&format!("; edited {}", files.join(", ")));
    }
    if let Some((key, _)) = exchange.checks.iter().find(|(_, failed)| *failed) {
        line.push_str(&format!(
            "; check failed: {}",
            clip(key, INDEX_REQUEST_CHARS)
        ));
    }
    line.push_str(&format!(" - full text at {artifact}"));
    line
}

/// The index message naming the set-aside exchanges, one line each.
pub fn task_view_index(lines: &[String]) -> Message {
    let text = render_template(TASK_VIEW_INDEX, &[("exchanges", &lines.join("\n"))]);
    Message::user_text(text.trim_end())
}

/// A message made by [`task_view_index`].
pub fn is_task_view_index(message: &Message) -> bool {
    let head = TASK_VIEW_INDEX
        .split("{{")
        .next()
        .unwrap_or_default()
        .trim();
    !head.is_empty() && message.text().starts_with(head)
}

/// Whether a new view pays for the prompt-cache miss it causes: always
/// when the cache is cold anyway (idle past its lifetime, right after a
/// summary, or a provider without prompt caching), else only over
/// [`MIN_HISTORY_TOKENS`] of history when the view keeps at most
/// [`MAX_VIEW_PERCENT`] of it.
pub fn worth_rebuilding(history_tokens: u64, view_tokens: u64, cache_cold: bool) -> bool {
    view_tokens < history_tokens
        && (cache_cold
            || (history_tokens >= MIN_HISTORY_TOKENS
                && view_tokens * 100 <= history_tokens * MAX_VIEW_PERCENT))
}

fn clip(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((offset, _)) => format!("{}...", &text[..offset]),
        None => text.to_string(),
    }
}

#[cfg(test)]
#[path = "task_view_tests.rs"]
mod tests;
