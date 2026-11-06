//! The questions of a task view: does the new message start another task
//! (`task_boundary`), does the new task need each earlier exchange
//! (`exchange_needed`, on its digest), and did an earlier user message set
//! a rule for the rest of the chat (`standing_rule`, kept per message).

use std::collections::HashMap;

use serde_json::json;
use z_engine_context::compaction::{Exchange, request_text};
use z_engine_decisions::{Answer, Question, UNCHANGED};
use z_engine_prompts::decisions::{EXCHANGE_NEEDED, STANDING_RULE, TASK_BOUNDARY};
use z_engine_protocol::{Message, MessageId};

use super::history::History;
use crate::decisions::context::UseContext;
use crate::decisions::uses::digest::head;
use crate::decisions::uses::guidance::ask_each;

pub(super) const BOUNDARY: &str = "task_boundary";
pub(super) const NEEDED: &str = "exchange_needed";
pub(super) const STANDING: &str = "standing_rule";
const MESSAGE_CHARS: usize = 600;
const PREVIOUS_CHARS: usize = 400;
const REPLY_CHARS: usize = 200;

/// One fresh answer about the exchange at `index`.
pub(super) struct Asked {
    pub index: usize,
    pub fingerprint: String,
    pub answer: Answer,
}

/// The model's boundary choice, recorded: true for `related` or
/// `unrelated`; `continue` or no confident answer keeps the view as is.
pub(super) async fn is_boundary(
    cx: &UseContext,
    text: &str,
    history: &History,
    exchanges: &[Exchange],
) -> bool {
    let Ok(question) = Question::choice(BOUNDARY, TASK_BOUNDARY) else {
        return false;
    };
    let previous = exchanges.iter().rev().find(|exchange| !exchange.lead);
    let state = json!({
        "previous_request": head(previous.map_or("", |e| e.request.as_str()), PREVIOUS_CHARS),
        "last_reply": head(previous.map_or("", |e| e.reply.as_str()), REPLY_CHARS),
        "new_message": head(text.trim(), MESSAGE_CHARS),
        "todos": history.todos,
    });
    let Some((fingerprint, answer)) = ask_each(cx, &question, vec![state]).await.pop() else {
        return false;
    };
    let boundary = matches!(answer.choice(), Some("related" | "unrelated"));
    let outcome = if boundary { "new task" } else { UNCHANGED };
    cx.record(cx.record_of(&answer, &fingerprint).outcome(outcome));
    boundary
}

/// `exchange_needed` about each exchange in `open`, against the new request.
pub(super) async fn ask_needed(
    cx: &UseContext,
    text: &str,
    history: &History,
    exchanges: &[Exchange],
    open: &[usize],
) -> Vec<Asked> {
    let Ok(question) = Question::yes_no(NEEDED, EXCHANGE_NEEDED) else {
        return Vec::new();
    };
    let request = head(text.trim(), MESSAGE_CHARS);
    let states = open.iter().map(|&index| {
        json!({ "new_request": request, "exchange": exchanges[index].digest(&history.root) })
    });
    let answers = ask_each(cx, &question, states.collect()).await;
    open.iter()
        .zip(answers)
        .map(|(&index, (fingerprint, answer))| Asked {
            index,
            fingerprint,
            answer,
        })
        .collect()
}

/// Whether the opening message of each exchange in `open` set a standing
/// rule, from earlier verdicts or by asking; fresh answers come back too.
pub(super) async fn ask_rules(
    cx: &UseContext,
    messages: &[Message],
    exchanges: &[Exchange],
    open: &[usize],
) -> (HashMap<usize, bool>, Vec<Asked>) {
    let revision = cx.service.revision();
    let memory = cx.core.decisions.task_view();
    let known = memory.rules(&revision);
    let opening = |index: usize| -> &Message { &messages[exchanges[index].start] };
    let mut rules: HashMap<usize, bool> = open
        .iter()
        .filter_map(|&index| Some((index, *known.get(&opening(index).id)?)))
        .collect();
    let unknown: Vec<usize> = open
        .iter()
        .copied()
        .filter(|index| !rules.contains_key(index))
        .collect();
    let Ok(question) = Question::yes_no(STANDING, STANDING_RULE) else {
        return (rules, Vec::new());
    };
    let states = unknown
        .iter()
        .map(|&index| json!({ "message": head(&request_text(opening(index)), MESSAGE_CHARS) }));
    let answers = ask_each(cx, &question, states.collect()).await;
    let mut asked = Vec::with_capacity(answers.len());
    for (&index, (fingerprint, answer)) in unknown.iter().zip(answers) {
        if let Some(standing) = answer.yes() {
            let id: MessageId = opening(index).id.clone();
            memory.remember_rule(&revision, id, standing);
            rules.insert(index, standing);
        }
        asked.push(Asked {
            index,
            fingerprint,
            answer,
        });
    }
    (rules, asked)
}
