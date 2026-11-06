//! Plans a task view at turn start. A new task begins when the model says
//! the message is `related` or `unrelated` to the last request, or by rule
//! after idling past the prompt-cache lifetime or right after a summary.
//! Then each earlier exchange no hard keep holds is judged on its digest;
//! the unneeded ones are set aside unless the cost check says the rebuilt
//! prefix would not pay for itself. Only On mode leaves a plan behind;
//! `apply` turns it into the working set once the new message is saved.

use async_trait::async_trait;
use futures::join;
use z_engine_config::FeatureId;
use z_engine_context::compaction::{
    Exchange, MIN_HISTORY_TOKENS, TaskKeep, TaskViewPlan, exchanges, named_paths, plan_task_view,
    task_keeps, worth_rebuilding,
};
use z_engine_context::estimate_messages;
use z_engine_decisions::UNCHANGED;
use z_engine_protocol::{ToolResultPart, ToolStatus};

use super::ask::{Asked, ask_needed, ask_rules, is_boundary};
use super::history::History;
use super::memory::PendingView;
use super::readback;
use crate::batch::ToolCall;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

/// Most exchanges judged per boundary, largest first.
const MAX_ASKED: usize = 32;
/// Rough size of one index line, for the cost check.
const INDEX_LINE_TOKENS: u64 = 40;

#[derive(Debug)]
pub(crate) struct TaskView;

pub(crate) static TASK_VIEW: TaskView = TaskView;

#[async_trait]
impl DecisionUse for TaskView {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsTaskView
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::TurnStart, Seam::AfterCall]
    }

    async fn turn_start(&self, cx: &UseContext, text: &str) -> Vec<String> {
        plan(cx, text).await;
        Vec::new()
    }

    async fn after_call(
        &self,
        cx: &UseContext,
        call: &ToolCall,
        status: ToolStatus,
        _output: &[ToolResultPart],
    ) -> Vec<String> {
        if status == ToolStatus::Ok {
            readback::observe(cx, call);
        }
        Vec::new()
    }
}

async fn plan(cx: &UseContext, text: &str) {
    let memory = cx.core.decisions.task_view();
    memory.set_pending(None);
    let Some(history) = History::read(cx) else {
        return;
    };
    let exchanges = exchanges(&history.messages);
    let keep_recent = cx
        .core
        .settings()
        .settings
        .decisions
        .task_view
        .keep_recent();
    let keeps = task_keeps(
        &exchanges,
        keep_recent,
        &named_paths(text),
        !history.todos.is_empty(),
    );
    let tokens: Vec<u64> = exchanges
        .iter()
        .map(|exchange| estimate_messages(&history.messages[exchange.start..exchange.end]))
        .collect();
    let open = open_exchanges(&keeps, &tokens);
    let history_tokens: u64 = tokens.iter().sum();
    let cold = history.boundary || history.uncached;
    if open.is_empty() || (!cold && history_tokens < MIN_HISTORY_TOKENS) {
        return;
    }
    if !history.boundary && !is_boundary(cx, text, &history, &exchanges).await {
        return;
    }
    let (needed, (rules, asked_rules)) = join!(
        ask_needed(cx, text, &history, &exchanges, &open),
        ask_rules(cx, &history.messages, &exchanges, &open),
    );
    let plan = plan_task_view(
        &keeps,
        |index| verdict(&needed, index),
        |index| rules.get(&index) == Some(&true),
    );
    let view_tokens = view_tokens(&plan, &history, &exchanges, &tokens, history_tokens);
    let worth = !plan.set_aside.is_empty() && worth_rebuilding(history_tokens, view_tokens, cold);
    trace(cx, &needed, &asked_rules, &plan, &tokens, worth);
    if !worth || cx.shadow {
        return;
    }
    let set_aside = plan.set_aside.iter().map(|&index| tokens[index]).sum();
    memory.set_pending(Some(PendingView {
        history: history.messages.iter().map(|m| m.id.clone()).collect(),
        exchanges,
        plan,
        tokens: set_aside,
    }));
}

/// Exchanges no hard keep holds, largest first, at most `MAX_ASKED`.
fn open_exchanges(keeps: &[Option<TaskKeep>], tokens: &[u64]) -> Vec<usize> {
    let mut open: Vec<usize> = (0..keeps.len())
        .filter(|&index| keeps[index].is_none())
        .collect();
    open.sort_by_key(|&index| std::cmp::Reverse(tokens[index]));
    open.truncate(MAX_ASKED);
    open.sort_unstable();
    open
}

fn verdict(needed: &[Asked], index: usize) -> Option<bool> {
    needed
        .iter()
        .find(|asked| asked.index == index)?
        .answer
        .yes()
}

/// Estimated tokens of the view: history minus what is set aside, plus
/// pinned rules and the index that replace it.
fn view_tokens(
    plan: &TaskViewPlan,
    history: &History,
    exchanges: &[Exchange],
    tokens: &[u64],
    history_tokens: u64,
) -> u64 {
    let set_aside: u64 = plan.set_aside.iter().map(|&index| tokens[index]).sum();
    let pinned: u64 = plan
        .pinned
        .iter()
        .map(|&index| {
            estimate_messages(&history.messages[exchanges[index].start..=exchanges[index].start])
        })
        .sum();
    let index = INDEX_LINE_TOKENS * (plan.set_aside.len() as u64 + 1);
    history_tokens - set_aside + pinned + index
}

fn trace(
    cx: &UseContext,
    needed: &[Asked],
    rules: &[Asked],
    plan: &TaskViewPlan,
    tokens: &[u64],
    worth: bool,
) {
    for asked in needed {
        let mut record = cx.record_of(&asked.answer, &asked.fingerprint);
        if plan.set_aside.contains(&asked.index) {
            record = if worth {
                record.outcome("set aside").saved(tokens[asked.index])
            } else {
                record
                    .outcome(UNCHANGED)
                    .overridden("not worth a prompt-cache miss")
            };
        } else {
            record = record.outcome(UNCHANGED);
        }
        cx.record(record);
    }
    for asked in rules {
        let pinned = worth && plan.pinned.contains(&asked.index);
        let outcome = if pinned { "pinned" } else { UNCHANGED };
        cx.record(
            cx.record_of(&asked.answer, &asked.fingerprint)
                .outcome(outcome),
        );
    }
}
