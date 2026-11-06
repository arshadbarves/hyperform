//! Applies a planned task view once the new request is saved, and brings
//! the full history back when the user asks. Set-aside exchanges are saved
//! to artifacts and listed in one index message (where to read them back);
//! the view is journaled as kept message ids so replay and reopen rebuild
//! the same working set. Anything unexpected leaves the history whole.

use z_engine_config::FeatureId;
use z_engine_context::compaction::{
    apply_task_view, index_line, render_for_summary, task_view_index,
};
use z_engine_decisions::{AbstainReason, Answer, DecisionRecord};
use z_engine_protocol::decisions::TaskViewInfo;
use z_engine_protocol::{Event, Message, now_ms};
use z_engine_store::LogRecord;

use super::ask::NEEDED;
use crate::session::SessionCore;

/// Longest tool result kept in a saved exchange.
const SPILL_RESULT_CHARS: usize = 20_000;

/// Applies the view planned at this turn's start, if the history is still
/// the one it was planned for and ends with `message`.
pub(crate) fn apply_task_view_for(core: &SessionCore, message: &Message) {
    let Some(pending) = core.decisions.task_view().take_pending() else {
        return;
    };
    let full = core.with_state(|state| {
        let full = state.full_working.as_ref().unwrap_or(&state.working);
        let (last, before) = full.split_last()?;
        let same = last.id == message.id && before.iter().map(|m| &m.id).eq(pending.history.iter());
        same.then(|| full.clone())
    });
    let Some(full) = full else {
        tracing::debug!("history changed since the task view was planned; left whole");
        return;
    };
    let history = &full[..full.len() - 1];
    let artifacts = core.shared.store.artifacts(&core.id);
    let root = core.root.to_string_lossy();
    let mut lines = Vec::with_capacity(pending.plan.set_aside.len());
    let mut spilled = Vec::with_capacity(lines.capacity());
    for &index in &pending.plan.set_aside {
        let exchange = &pending.exchanges[index];
        let number = pending.exchanges[..=index]
            .iter()
            .filter(|e| !e.lead)
            .count();
        let text = render_for_summary(&history[exchange.start..exchange.end], SPILL_RESULT_CHARS);
        let path = match artifacts.write_text(&format!("task-view-turn-{number}"), "txt", &text) {
            Ok(path) => path.display().to_string(),
            Err(error) => {
                tracing::warn!(%error, "could not save a set-aside exchange; history left whole");
                return;
            }
        };
        lines.push(index_line(number, exchange, &root, &path));
        spilled.push(path);
    }
    let index = task_view_index(&lines);
    let mut view = apply_task_view(history, &pending.exchanges, &pending.plan, index.clone());
    view.push(message.clone());
    let info = TaskViewInfo {
        boundary: message.id.clone(),
        set_aside: u32::try_from(lines.len()).unwrap_or(u32::MAX),
        tokens: pending.tokens,
        restored: false,
        created_at: now_ms(),
    };
    let record = LogRecord::TaskView {
        view: info.clone(),
        working: view.iter().map(|m| m.id.clone()).collect(),
        index: Some(index),
    };
    if !core.journal.append_or_report(&record) {
        return;
    }
    core.with_state(|state| {
        let before = std::mem::replace(&mut state.working, view);
        state.meter.rebase(&before, &state.working);
        state.full_working = Some(full);
        state
            .task_views
            .retain(|seen| seen.boundary != info.boundary);
        state.task_views.push(info.clone());
    });
    core.decisions.task_view().spilled(spilled);
    core.events.emit(Event::TaskViewApplied { view: info });
}

/// Puts the full history back until the next task boundary (the divider's
/// "Include full history"). Does nothing without a view in place.
pub(crate) fn include_full_history(core: &SessionCore) {
    let restored = core.with_state(|state| {
        let full = state.full_working.take()?;
        let view = std::mem::replace(&mut state.working, full);
        state.meter.rebase(&view, &state.working);
        let info = state.task_views.last_mut()?;
        info.restored = true;
        Some(info.clone())
    });
    let Some(info) = restored else {
        return;
    };
    core.journal.append_or_report(&LogRecord::TaskView {
        view: info.clone(),
        working: Vec::new(),
        index: None,
    });
    let clicked = Answer {
        abstain: None,
        cached: true,
        ..Answer::abstained(NEEDED, AbstainReason::Rules, "user")
    };
    let feature = FeatureId::DecisionsTaskView.as_str();
    let record = DecisionRecord::of(feature, &clicked, info.boundary.as_str(), false);
    core.decisions
        .trace()
        .record(record.outcome("full history restored"));
    core.events.emit(Event::TaskViewApplied { view: info });
}
