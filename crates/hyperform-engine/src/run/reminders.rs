//! Reminder blocks attached to an agent's next user message: the run's own
//! pending notes, reminders queued from outside the run (finished jobs,
//! hook context), the deferred MCP tool listing, the work packet when it is
//! due, files changed behind the agent's back, and queued steering for the
//! main agent.

use std::path::PathBuf;

use hyperform_context::{files_changed_externally, steering, todo_nudge};
use hyperform_host::{FileTracker, relative_display};
use hyperform_protocol::{ContentBlock, Event};

use super::spec::RunContext;
use crate::work;

/// Rounds without `TodoWrite` before an empty todo list earns a nudge.
const TODO_NUDGE_ROUNDS: u32 = 10;

/// Counts tool rounds without `TodoWrite`.
#[derive(Debug, Default)]
pub(crate) struct TodoNudge {
    rounds: u32,
}

impl TodoNudge {
    /// The nudge, when the agent's list is empty after enough rounds.
    pub(crate) fn after_round(
        &mut self,
        ctx: &RunContext,
        used_todo_write: bool,
    ) -> Option<String> {
        if used_todo_write {
            self.rounds = 0;
            return None;
        }
        self.rounds += 1;
        let agent = &ctx.spec.agent_id;
        let empty = ctx
            .core
            .with_state(|state| state.todos_of(agent).is_empty());
        if self.rounds < TODO_NUDGE_ROUNDS || !empty {
            return None;
        }
        self.rounds = 0;
        Some(todo_nudge())
    }
}

/// Every pending reminder as one text block each, oldest first.
pub(crate) async fn collect(ctx: &RunContext, pending: &mut Vec<String>) -> Vec<ContentBlock> {
    let mut texts = std::mem::take(pending);
    texts.extend(ctx.core.reminders.take(&ctx.spec.agent_id));
    texts.extend(ctx.mcp.reminder(&ctx.core.mcp.catalog()));
    texts.extend(work::due_packet(ctx).await);
    if let Some(changed) = changed_files(&ctx.resources.files).await {
        let shown: Vec<String> = changed
            .iter()
            .map(|path| relative_display(&ctx.spec.root, path))
            .collect();
        texts.push(files_changed_externally(&shown));
    }
    texts.into_iter().map(ContentBlock::text).collect()
}

/// Tracked files that changed on disk since the agent last saw them. They
/// are forgotten once reported, so the agent must read them again before
/// editing and each change is reported once.
async fn changed_files(files: &FileTracker) -> Option<Vec<PathBuf>> {
    let files = files.clone();
    let changed = tokio::task::spawn_blocking(move || {
        let changed = files.changed_since_read();
        for path in &changed {
            files.forget(path);
        }
        changed
    })
    .await;
    match changed {
        Ok(changed) if !changed.is_empty() => Some(changed),
        Ok(_) => None,
        Err(error) => {
            tracing::warn!(%error, "checking for externally changed files failed");
            None
        }
    }
}

/// Drains the main agent's steering queue: a visible text block with the
/// user's words (the GUI shows it as a steering note) and the reminder
/// that tells the model how to treat them. Empty for other agents.
pub(crate) fn take_steering(ctx: &RunContext) -> Vec<ContentBlock> {
    if !ctx.spec.is_main() {
        return Vec::new();
    }
    let queued = ctx
        .core
        .with_state(|state| std::mem::take(&mut state.queue));
    if queued.is_empty() {
        return Vec::new();
    }
    ctx.core
        .events
        .emit(Event::QueueChanged { queued: Vec::new() });
    vec![
        ContentBlock::text(queued.join("\n\n")),
        ContentBlock::text(steering(&queued)),
    ]
}
