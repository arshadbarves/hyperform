//! Repairs what a crash left open in a stored session: the open turn is
//! closed as `Interrupted`, unanswered tool calls get "interrupted"
//! results so the working set is valid for any provider, subagents that
//! were still running are recorded as failed, and questions or plans
//! nobody can answer any more are resolved as dismissed.

use z_engine_protocol::{
    AgentStatus, CallId, ContentBlock, Message, Role, TurnOutcome, TurnRecord, Usage,
    VerificationOutcome, now_ms,
};
use z_engine_store::{LogRecord, ReplayState};

use crate::error::EngineError;
use crate::session::Journal;

const INTERRUPTED_RESULT: &str = "interrupted: the app closed before this call finished";
const INTERRUPTED_AGENT: &str = "interrupted: the app closed before this agent finished";

/// Appends the repair records; returns whether anything was repaired (the
/// caller then replays the log again).
pub(crate) fn repair(journal: &Journal, replay: &ReplayState) -> Result<bool, EngineError> {
    let mut repaired = false;
    let open_turn = replay.open_turn.clone();
    let dangling = unanswered(&replay.working);
    if !dangling.is_empty() {
        let results = dangling
            .into_iter()
            .map(|id| ContentBlock::tool_result(id, INTERRUPTED_RESULT, true))
            .collect();
        journal.append(&LogRecord::Message {
            message: Message::new(Role::User, results),
            turn_id: open_turn.as_ref().map(|(turn_id, _)| turn_id.clone()),
        })?;
        repaired = true;
    }
    if let Some((turn_id, message_id)) = open_turn {
        let start = replay
            .transcript
            .iter()
            .position(|message| message.id == message_id);
        let started_at = start.map_or_else(now_ms, |index| replay.transcript[index].created_at);
        let acted =
            start.is_some_and(|index| replay.transcript[index..].iter().any(Message::has_tool_use));
        let verification = if acted {
            VerificationOutcome::Unverified {
                reason: "the app closed before the turn finished".to_string(),
            }
        } else {
            VerificationOutcome::NotApplicable
        };
        journal.append(&LogRecord::TurnFinished {
            turn: TurnRecord {
                turn_id,
                message_id,
                outcome: TurnOutcome::Interrupted,
                verification,
                usage: Usage::default(),
                cost_usd: 0.0,
                started_at,
                finished_at: now_ms(),
            },
        })?;
        repaired = true;
    }
    for (request_id, ..) in &replay.pending_questions {
        journal.append(&LogRecord::QuestionAnswered {
            request_id: request_id.clone(),
            answers: None,
        })?;
        repaired = true;
    }
    for agent in replay
        .agents
        .iter()
        .filter(|agent| !agent.status.is_terminal())
    {
        let mut info = agent.clone();
        info.status = AgentStatus::Failed;
        info.error = Some(INTERRUPTED_AGENT.to_string());
        info.finished_at = Some(now_ms());
        journal.append(&LogRecord::AgentUpdated { info })?;
        repaired = true;
    }
    for (request_id, ..) in &replay.pending_plans {
        journal.append(&LogRecord::PlanResolved {
            request_id: request_id.clone(),
            approved: false,
            feedback: None,
            final_plan: None,
        })?;
        repaired = true;
    }
    if repaired {
        journal.sync()?;
    }
    Ok(repaired)
}

/// Calls of a trailing assistant message (the round a crash cut off). An
/// earlier call whose result write was lost cannot be answered in place
/// by appending; request assembly fills that gap instead.
fn unanswered(working: &[Message]) -> Vec<CallId> {
    match working.last() {
        Some(last) if last.role == Role::Assistant => {
            last.tool_uses().map(|(id, _, _)| id.clone()).collect()
        }
        _ => Vec::new(),
    }
}
