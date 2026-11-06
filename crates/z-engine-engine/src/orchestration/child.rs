//! Running a prepared child to its end: wait for a concurrency slot, run
//! the shared agent loop with the subagent transcript, commit a worktree,
//! record the final `AgentInfo`, settle a work-plan task's helper (its
//! report and change), and roll usage and changes up (into the caller's
//! run for foreground agents, into the session log and the next turn's
//! badge for background ones).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use z_engine_protocol::{AgentInfo, AgentStatus, NoticeLevel, TurnOutcome, Usage, WorktreeInfo};
use z_engine_store::LogRecord;

use super::launch::Child;
use super::{task_run, worktree};
use crate::run::{AgentRun, RunOutcome, TranscriptSink};

/// How a child ended, for its caller.
#[derive(Debug, Clone)]
pub(crate) struct ChildReport {
    pub info: AgentInfo,
    pub outcome: TurnOutcome,
    /// The final (or latest) assistant text.
    pub text: String,
    pub duration: Duration,
    /// This run's usage and cost (earlier runs of a resumed agent excluded).
    pub usage: Usage,
    pub cost_usd: f64,
    pub tool_calls: u32,
    pub written: Vec<PathBuf>,
}

pub(crate) async fn run_child(child: Child) -> ChildReport {
    let clock = Instant::now();
    let core = Arc::clone(&child.ctx.core);
    let slot = if child.needs_slot {
        tokio::select! {
            biased;
            () = child.ctx.cancel.cancelled() => None,
            slot = core.agents.slot() => slot,
        }
    } else {
        None
    };
    let run = if child.needs_slot && slot.is_none() {
        cancelled_before_start()
    } else {
        let sink: Arc<dyn TranscriptSink> = child.sink.clone();
        AgentRun::new(child.ctx.clone(), sink, false).run().await
    };
    drop(slot);
    child.sink.sync();
    let worktree = match child.worktree.clone() {
        Some(info) => Some(commit(&child, info).await),
        None => None,
    };
    let (status, error) = status_of(&run.outcome);
    let (base_usage, base_cost, base_calls) = child.base;
    child.tracker.finish(&core, |info| {
        info.status = status;
        info.error = error;
        info.usage = base_usage + run.usage;
        info.cost_usd = base_cost + run.cost_usd;
        info.tool_calls = base_calls.saturating_add(run.tool_calls);
        info.worktree = worktree.clone();
    });
    let shared = worktree.is_none();
    if child.background {
        let agent_id = child.ctx.spec.agent_id.clone();
        if !run.usage.is_empty() {
            core.journal.append_or_report(&LogRecord::Usage {
                agent_id,
                usage: run.usage,
                cost_usd: run.cost_usd,
            });
        }
        if run.mutated && shared {
            core.with_state(|state| state.external_mutation = true);
        }
    } else {
        child.parent_tally.add_usage(run.usage, run.cost_usd);
        if run.mutated && shared {
            child.parent_tally.mark_mutated();
        }
    }
    let mut info = child.tracker.info();
    if info.task_id.is_some() {
        task_run::settle(&core, &info, &run.written, child.tracker.read_web()).await;
        info = task_run::latest(&core, &info.agent_id).unwrap_or(info);
    }
    ChildReport {
        info,
        outcome: run.outcome,
        text: run.final_text,
        duration: clock.elapsed(),
        usage: run.usage,
        cost_usd: run.cost_usd,
        tool_calls: run.tool_calls,
        written: run.written,
    }
}

/// Commits the worktree; a failure keeps it pending for the user.
async fn commit(child: &Child, info: WorktreeInfo) -> WorktreeInfo {
    let core = &child.ctx.core;
    let agent = &child.ctx.spec.agent_id;
    match worktree::finalize(core, agent, &child.description, info.clone()).await {
        Ok(info) => info,
        Err(error) => {
            core.events.notice(
                NoticeLevel::Warn,
                format!("Could not commit agent {agent}'s worktree: {error}"),
            );
            info
        }
    }
}

fn status_of(outcome: &TurnOutcome) -> (AgentStatus, Option<String>) {
    match outcome {
        TurnOutcome::Completed => (AgentStatus::Completed, None),
        TurnOutcome::Cancelled | TurnOutcome::Interrupted => (AgentStatus::Cancelled, None),
        TurnOutcome::Failed { message } => (AgentStatus::Failed, Some(message.clone())),
        TurnOutcome::BudgetExhausted { reason } => {
            (AgentStatus::Failed, Some(format!("stopped: {reason}")))
        }
    }
}

fn cancelled_before_start() -> RunOutcome {
    RunOutcome {
        outcome: TurnOutcome::Cancelled,
        usage: Usage::default(),
        cost_usd: 0.0,
        mutated: false,
        verification: None,
        final_text: String::new(),
        tool_calls: 0,
        written: Vec::new(),
    }
}
