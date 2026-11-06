//! One user turn of the main agent: a prompt command's expansion and
//! turn-scoped grants, `UserPromptSubmit` hooks, the opening message and
//! `TurnStarted`, a code checkpoint, the title for a new session, the
//! agent run, and `TurnFinished` with its badge. Decision uses see the
//! message first (the turn-start seam); a new task is routed once the
//! message is saved; the ended turn goes to the turn-end seam.

use std::sync::Arc;

use hyperform_protocol::{
    Event, Message, MessageId, NoticeLevel, Role, TurnId, TurnOutcome, TurnRecord,
    VerificationOutcome, now_ms,
};
use hyperform_store::LogRecord;
use serde_json::json;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::checkpoint::take_checkpoint;
use super::command_turn::{TurnScope, command_texts};
use super::meta::write_meta;
use super::prompt::{TurnInput, compose, prompt_text};
use super::title::spawn_title;
use crate::decisions::seams::{at_turn_end, at_turn_start};
use crate::decisions::{apply_task_view_for, preload_tools, route_new_task};
use crate::hooks::{HookEvent, HookInput, run_hooks};
use crate::run::{AgentRun, AgentSpec, MainSink, RunContext, RunOutcome, TranscriptSink};
use crate::session::SessionCore;
use crate::verify::{Mutation, current_outcome};
use crate::work;

/// `None` when no turn started (nothing to send, or a hook blocked it).
pub(crate) async fn run_turn(
    core: Arc<SessionCore>,
    input: TurnInput,
    cancel: CancellationToken,
) -> Option<TurnOutcome> {
    let (texts, expansion) = match &input.command {
        Some(call) => {
            let (texts, expansion) = command_texts(&core, call, &cancel).await?;
            (texts, Some(expansion))
        }
        None => (vec![prompt_text(&core, &input)], None),
    };
    let text = texts[0].clone();
    if text.is_empty() && input.attachments.is_empty() {
        return None;
    }
    let prompt = HookInput::new().with("prompt", json!(text));
    let hooks = run_hooks(
        &core.hook_env(),
        &core.events,
        HookEvent::UserPromptSubmit,
        prompt,
        &cancel,
    )
    .await;
    if let Some(reason) = hooks.blocked.or(hooks.stop) {
        core.events.notice(
            NoticeLevel::Warn,
            format!("A UserPromptSubmit hook blocked this prompt: {reason}"),
        );
        return None;
    }
    let _scope = expansion
        .as_ref()
        .map(|expansion| TurnScope::apply(&core, expansion));
    let max_turns = core.settings().settings.agents.max_turns;
    let ctx = RunContext::new(
        Arc::clone(&core),
        AgentSpec::main(core.root.clone(), max_turns),
        core.main.clone(),
        cancel,
    );
    at_turn_start(&ctx, &text).await;
    let content = compose(&ctx, &texts, &input.attachments, hooks.context).await;
    let message = Message::new(Role::User, content);
    let turn_id = TurnId::new();
    let started_at = now_ms();
    let fresh = core.with_state(|state| state.title.is_none() && state.turns.is_empty());
    let sink = Arc::new(MainSink::new(Arc::clone(&core), Some(turn_id.clone())));
    if let Err(error) = start(&core, &*sink, &message, &turn_id, started_at) {
        core.events
            .error(format!("could not save your message: {error}"));
        return None;
    }
    apply_task_view_for(&core, &message);
    preload_tools(&ctx);
    let (ready, gate) = oneshot::channel();
    let checkpoint = tokio::spawn({
        let core = Arc::clone(&core);
        let message_id = message.id.clone();
        async move {
            take_checkpoint(&core, &message_id).await;
            if ready.send(()).is_err() {
                tracing::debug!("turn ended before its checkpoint");
            }
        }
    });
    route_new_task(&ctx, &text).await;
    if fresh {
        spawn_title(Arc::clone(&core), text);
    }
    let mutated = core.with_state(|state| {
        let mutated = std::mem::take(&mut state.external_mutation);
        state.mutation = Mutation::seeded(mutated);
        mutated
    });
    let run = AgentRun::new(ctx, sink, mutated)
        .tools_wait_for(gate)
        .run()
        .await;
    if let Err(error) = checkpoint.await {
        tracing::warn!(%error, "checkpoint task failed");
    }
    core.broker.withdraw_abandoned();
    let verification = match run.verification.clone() {
        Some(outcome) => outcome,
        None => current_outcome(&core).await,
    };
    let outcome = finish(&core, turn_id, message.id, started_at, run, verification);
    work::link_checkpoint(&core).await;
    Some(outcome)
}

fn start(
    core: &SessionCore,
    sink: &dyn TranscriptSink,
    message: &Message,
    turn_id: &TurnId,
    started_at: u64,
) -> Result<(), crate::EngineError> {
    sink.append(message, false)?;
    core.journal.append(&LogRecord::TurnStarted {
        turn_id: turn_id.clone(),
        message_id: message.id.clone(),
        started_at,
    })?;
    core.events.emit(Event::TurnStarted {
        turn_id: turn_id.clone(),
        message_id: message.id.clone(),
    });
    Ok(())
}

fn finish(
    core: &Arc<SessionCore>,
    turn_id: TurnId,
    message_id: MessageId,
    started_at: u64,
    run: RunOutcome,
    verification: VerificationOutcome,
) -> TurnOutcome {
    core.events.emit(Event::VerificationChanged {
        outcome: verification.clone(),
    });
    let turn = TurnRecord {
        turn_id,
        message_id,
        outcome: run.outcome.clone(),
        verification,
        usage: run.usage,
        cost_usd: run.cost_usd,
        started_at,
        finished_at: now_ms(),
    };
    core.journal
        .append_or_report(&LogRecord::TurnFinished { turn: turn.clone() });
    core.with_state(|state| {
        state.turns.push(turn.clone());
        state.interrupted = matches!(run.outcome, TurnOutcome::Cancelled);
        state.updated_at = turn.finished_at;
    });
    core.events.emit(Event::TurnFinished { turn: turn.clone() });
    at_turn_end(core, &turn);
    write_meta(core);
    core.journal.sync_or_report();
    run.outcome
}
