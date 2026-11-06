//! One agent run: rounds of request, stream, record, and either a tool
//! batch or the stop boundary, until the run ends. Every `tool_use`
//! receives exactly one `tool_result`, so the transcript stays valid for
//! any provider whatever the outcome.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::oneshot;
use z_engine_llm::AssistantTurn;
use z_engine_protocol::{
    ContentBlock, Event, Message, MessageId, Role, TurnOutcome, Usage, VerificationOutcome, now_ms,
};

use super::meter::ContextMeter;
use super::mutation::note_mutation;
use super::reminders::{TodoNudge, collect, take_steering};
use super::repo_map::ensure_repo_map;
use super::request::{assemble, prepare};
use super::sink::TranscriptSink;
use super::spec::{RunContext, RunOutcome};
use super::stop::{StopAction, StopCounters, stop_boundary};
use super::stream::{StreamEnd, stream_response};
use super::{budget, pressure, usage};
use crate::batch::{ToolCall, ToolSet, run_batch};
use crate::decisions::seams::screen_request;
use crate::sync::lock;

pub(crate) struct AgentRun {
    ctx: RunContext,
    sink: Arc<dyn TranscriptSink>,
    rounds: u32,
    usage: Usage,
    cost: f64,
    mutated: bool,
    stop: StopCounters,
    todo_nudge: TodoNudge,
    /// Reminders for the next user message.
    pending: Vec<String>,
    final_text: String,
    verification: Option<VerificationOutcome>,
    tool_calls: u32,
    written: BTreeSet<PathBuf>,
    /// Resolves when the turn's checkpoint is taken; no tool runs before.
    before_tools: Option<oneshot::Receiver<()>>,
}

enum Round {
    Continue,
    End(TurnOutcome),
}

impl AgentRun {
    /// `mutated` seeds the run with changes made before it started (a
    /// `!cmd` since the previous turn).
    pub(crate) fn new(ctx: RunContext, sink: Arc<dyn TranscriptSink>, mutated: bool) -> Self {
        Self {
            ctx,
            sink,
            rounds: 0,
            usage: Usage::default(),
            cost: 0.0,
            mutated,
            stop: StopCounters::default(),
            todo_nudge: TodoNudge::default(),
            pending: Vec::new(),
            final_text: String::new(),
            verification: None,
            tool_calls: 0,
            written: BTreeSet::new(),
            before_tools: None,
        }
    }

    /// Holds the first tool batch until `gate` resolves (the checkpoint
    /// runs while the model thinks).
    pub(crate) fn tools_wait_for(mut self, gate: oneshot::Receiver<()>) -> Self {
        self.before_tools = Some(gate);
        self
    }

    pub(crate) async fn run(mut self) -> RunOutcome {
        let mut meter = self.sink.meter();
        let outcome = loop {
            match self.round(&mut meter).await {
                Round::Continue => {}
                Round::End(outcome) => break outcome,
            }
        };
        self.sink.save_meter(meter);
        self.absorb_children();
        RunOutcome {
            outcome,
            usage: self.usage,
            cost_usd: self.cost,
            mutated: self.mutated,
            verification: self.verification,
            final_text: self.final_text,
            tool_calls: self.tool_calls,
            written: self.written.into_iter().collect(),
        }
    }

    /// Usage of finished foreground children and changes they (or an
    /// applied worktree) made count as this run's.
    fn absorb_children(&mut self) {
        let absorbed = self.ctx.children.take();
        self.usage += absorbed.usage;
        self.cost += absorbed.cost_usd;
        self.mutated |= absorbed.mutated;
        if absorbed.mutated {
            note_mutation(&self.ctx, now_ms());
        }
    }

    async fn round(&mut self, meter: &mut ContextMeter) -> Round {
        let stopped = self
            .ctx
            .tracker
            .as_ref()
            .is_some_and(|tracker| tracker.stop_requested());
        if self.ctx.cancel.is_cancelled() || stopped {
            return Round::End(TurnOutcome::Cancelled);
        }
        if let Some(reason) = budget::exhausted(&self.ctx, self.rounds) {
            return Round::End(TurnOutcome::BudgetExhausted { reason });
        }
        let model = self.ctx.model();
        ensure_repo_map(&self.ctx).await;
        let tools = ToolSet::offered(&self.ctx);
        let prepared = prepare(&self.ctx, &model, tools.specs());
        meter.set_overhead(prepared.overhead);
        let mut forced = false;
        loop {
            let working = screen_request(&self.ctx, &*self.sink, self.sink.working()).await;
            let relieved = pressure::relieve(&self.ctx, &*self.sink, meter, working, forced).await;
            let Ok(working) = relieved else {
                return Round::End(TurnOutcome::Cancelled);
            };
            let request = assemble(&self.ctx, &prepared, &model, working.clone());
            if self.ctx.spec.is_main() {
                *lock(&self.ctx.core.last_request) = Some(Arc::new(request.clone()));
            }
            let message_id = MessageId::new();
            match stream_response(&self.ctx, request, &message_id).await {
                StreamEnd::Complete(turn) => {
                    meter.observe(turn.usage.prompt_tokens(), working.len());
                    return self
                        .after_response(message_id, turn, &tools, &model, meter)
                        .await;
                }
                StreamEnd::Failed(error, partial) => {
                    self.account(&model, partial.usage, None);
                    if error.is_context_overflow() && !forced {
                        forced = true;
                        continue;
                    }
                    let message = error.to_string();
                    return Round::End(TurnOutcome::Failed { message });
                }
                StreamEnd::Cancelled(partial) => {
                    self.account(&model, partial.usage, None);
                    self.record_partial(message_id, partial);
                    return Round::End(TurnOutcome::Cancelled);
                }
            }
        }
    }

    async fn after_response(
        &mut self,
        message_id: MessageId,
        turn: AssistantTurn,
        tools: &ToolSet,
        model: &str,
        meter: &ContextMeter,
    ) -> Round {
        self.rounds += 1;
        let message = Message {
            id: message_id,
            role: Role::Assistant,
            content: turn.content,
            created_at: now_ms(),
        };
        if !message.content.is_empty() {
            if let Err(error) = self.record_assistant(&message) {
                return Round::End(TurnOutcome::Failed {
                    message: error.to_string(),
                });
            }
        }
        let context = self
            .ctx
            .spec
            .is_main()
            .then(|| meter.measure(&self.sink.working()));
        self.account(model, turn.usage, context);
        let calls: Vec<ToolCall> = message
            .tool_uses()
            .map(|(id, name, input)| ToolCall {
                id: id.clone(),
                name: name.to_string(),
                input: input.clone(),
            })
            .collect();
        if calls.is_empty() {
            return self.at_stop().await;
        }
        if let Some(gate) = self.before_tools.take() {
            tokio::select! {
                biased;
                () = self.ctx.cancel.cancelled() => {}
                ready = gate => if ready.is_err() {
                    tracing::debug!("checkpoint gate dropped without a signal");
                },
            }
        }
        let count = u32::try_from(calls.len()).unwrap_or(u32::MAX);
        let batch = run_batch(&self.ctx, tools, calls, &turn.malformed, &mut self.pending).await;
        self.mutated |= batch.mutated;
        if let Some(at) = batch.mutated_at {
            note_mutation(&self.ctx, at);
        }
        self.tool_calls = self.tool_calls.saturating_add(count);
        self.written.extend(batch.written);
        if let Some(tracker) = &self.ctx.tracker {
            tracker.add_tool_calls(&self.ctx.core, count);
        }
        self.absorb_children();
        let nudge = self
            .todo_nudge
            .after_round(&self.ctx, batch.used_todo_write);
        self.pending.extend(nudge);
        let cancelled = batch.cancelled || self.ctx.cancel.is_cancelled();
        let mut content = batch.results;
        let mut steering = false;
        if !cancelled {
            let steer = take_steering(&self.ctx);
            steering = !steer.is_empty();
            content.extend(steer);
            content.extend(collect(&self.ctx, &mut self.pending).await);
        }
        if let Err(error) = self
            .sink
            .append(&Message::new(Role::User, content), steering)
        {
            return Round::End(TurnOutcome::Failed {
                message: error.to_string(),
            });
        }
        if cancelled {
            Round::End(TurnOutcome::Cancelled)
        } else {
            Round::Continue
        }
    }

    async fn at_stop(&mut self) -> Round {
        let written: Vec<PathBuf> = self.written.iter().cloned().collect();
        match stop_boundary(&self.ctx, &mut self.stop, &written).await {
            StopAction::End { verification } => {
                self.verification = verification;
                Round::End(TurnOutcome::Completed)
            }
            StopAction::Continue {
                mut content,
                steering,
            } => {
                content.extend(collect(&self.ctx, &mut self.pending).await);
                match self
                    .sink
                    .append(&Message::new(Role::User, content), steering)
                {
                    Ok(()) => Round::Continue,
                    Err(error) => Round::End(TurnOutcome::Failed {
                        message: error.to_string(),
                    }),
                }
            }
        }
    }

    fn record_assistant(&mut self, message: &Message) -> Result<(), crate::EngineError> {
        self.sink.append(message, false)?;
        let text = message.text();
        if !text.trim().is_empty() {
            if let Some(tracker) = &self.ctx.tracker {
                tracker.note_text(&self.ctx.core, &text);
            }
            self.final_text = text;
        }
        self.ctx.core.events.emit(Event::AssistantFinished {
            agent_id: self.ctx.spec.agent_id.clone(),
            message: message.clone(),
        });
        Ok(())
    }

    /// Keeps the text that streamed before a cancel; unfinished tool calls
    /// and reasoning are dropped so no `tool_use` lacks a result.
    fn record_partial(&mut self, message_id: MessageId, partial: AssistantTurn) {
        let content: Vec<ContentBlock> = partial
            .content
            .into_iter()
            .filter(|block| matches!(block, ContentBlock::Text { text } if !text.trim().is_empty()))
            .collect();
        if content.is_empty() {
            return;
        }
        let message = Message {
            id: message_id,
            role: Role::Assistant,
            content,
            created_at: now_ms(),
        };
        if let Err(error) = self.record_assistant(&message) {
            self.ctx
                .core
                .events
                .error(format!("could not save the interrupted response: {error}"));
        }
    }

    fn account(&mut self, model: &str, usage: Usage, context: Option<u64>) {
        if usage.is_empty() && context.is_none() {
            return;
        }
        let cost = usage::account(
            &self.ctx.core,
            &self.ctx.spec.agent_id,
            model,
            usage,
            context,
        );
        self.usage += usage;
        self.cost += cost;
        if let Some(tracker) = &self.ctx.tracker {
            tracker.add_usage(&self.ctx.core, usage, cost);
        }
    }
}
