//! A deterministic [`ModelClient`]: queued scripts for the main flow,
//! consumable routed queues (subagents), and persistent routed responders
//! for side requests (titles, summaries).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio_util::sync::CancellationToken;
use z_engine_llm::{LlmError, ModelClient, ModelEvent, ModelRequest, ModelStream, StopReason};
use z_engine_protocol::Usage;

/// One scripted response.
#[derive(Debug, Clone)]
pub enum Script {
    Events(Vec<ModelEvent>),
    Error(LlmError),
    /// Emit these events, then stall until cancelled.
    Stall(Vec<ModelEvent>),
    /// Wait before emitting (cancellable).
    Delayed(Duration, Vec<ModelEvent>),
    /// Emit these events, then fail the stream (a connection lost mid-response).
    FailAfter(Vec<ModelEvent>, LlmError),
}

impl Script {
    /// A plain text answer that ends the turn.
    pub fn text(text: &str) -> Self {
        Self::Events(vec![
            ModelEvent::TextDelta(text.to_string()),
            usage(100, 20),
            ModelEvent::Stop(StopReason::EndTurn),
        ])
    }

    /// One tool call (`name`, `input`) with an auto-generated id.
    pub fn tool(name: &str, input: Value) -> Self {
        Self::tools(&[(name, input)])
    }

    /// Several tool calls in one assistant message.
    pub fn tools(calls: &[(&str, Value)]) -> Self {
        Self::text_and_tools("", calls)
    }

    pub fn text_and_tools(text: &str, calls: &[(&str, Value)]) -> Self {
        let mut events = Vec::new();
        if !text.is_empty() {
            events.push(ModelEvent::TextDelta(text.to_string()));
        }
        for (index, (name, input)) in calls.iter().enumerate() {
            events.push(ModelEvent::ToolUseStart {
                index,
                id: format!("call_{}", ulid::Ulid::new().to_string().to_lowercase()),
                name: (*name).to_string(),
            });
            events.push(ModelEvent::ToolUseDelta {
                index,
                partial_json: input.to_string(),
            });
            events.push(ModelEvent::ToolUseEnd { index });
        }
        events.push(usage(120, 30));
        events.push(ModelEvent::Stop(StopReason::ToolUse));
        Self::Events(events)
    }
}

/// A `Usage` event with the given uncached input and output tokens.
pub fn usage(input: u64, output: u64) -> ModelEvent {
    ModelEvent::Usage(Usage {
        input_tokens: input,
        output_tokens: output,
        ..Usage::default()
    })
}

type Matcher = Box<dyn Fn(&ModelRequest) -> bool + Send + Sync>;

#[derive(Default)]
struct Inner {
    queue: VecDeque<Script>,
    /// Consumed in order; an exhausted queue stops matching.
    routed_queues: Vec<(Matcher, VecDeque<Script>)>,
    routes: Vec<(Matcher, Script)>,
    requests: Vec<ModelRequest>,
    fallback: Option<Script>,
}

#[derive(Clone, Default)]
pub struct ScriptedModel {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for ScriptedModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptedModel").finish_non_exhaustive()
    }
}

impl ScriptedModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue responses for successive main-flow requests.
    pub fn push(&self, script: Script) -> &Self {
        self.lock().queue.push_back(script);
        self
    }

    /// Answer every request matching `matcher` with `script` (not consumed).
    pub fn route(
        &self,
        matcher: impl Fn(&ModelRequest) -> bool + Send + Sync + 'static,
        script: Script,
    ) -> &Self {
        self.lock().routes.push((Box::new(matcher), script));
        self
    }

    /// Answer successive requests matching `matcher` with `scripts`, one
    /// each (e.g. the rounds of one subagent). Checked before `route`;
    /// once exhausted, matching requests fall through to routes and the
    /// main queue.
    pub fn route_queue(
        &self,
        matcher: impl Fn(&ModelRequest) -> bool + Send + Sync + 'static,
        scripts: Vec<Script>,
    ) -> &Self {
        self.lock()
            .routed_queues
            .push((Box::new(matcher), scripts.into()));
        self
    }

    /// [`ScriptedModel::route_queue`] for requests whose system prompt
    /// contains `needle`.
    pub fn route_queue_system(&self, needle: &'static str, scripts: Vec<Script>) -> &Self {
        self.route_queue(move |req| req.system_text().contains(needle), scripts)
    }

    /// Scripts left in routed queues.
    pub fn routed_remaining(&self) -> usize {
        self.lock()
            .routed_queues
            .iter()
            .map(|(_, queue)| queue.len())
            .sum()
    }

    /// Answer requests whose system prompt contains `needle`.
    pub fn route_system(&self, needle: &'static str, script: Script) -> &Self {
        self.route(move |req| req.system_text().contains(needle), script)
    }

    /// Response used when the queue is empty (default: "done").
    pub fn fallback(&self, script: Script) -> &Self {
        self.lock().fallback = Some(script);
        self
    }

    /// Every request received so far, in order.
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.lock().requests.clone()
    }

    pub fn remaining(&self) -> usize {
        self.lock().queue.len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn next_script(&self, request: &ModelRequest) -> Script {
        let mut inner = self.lock();
        inner.requests.push(request.clone());
        let routed = inner
            .routed_queues
            .iter_mut()
            .find(|(matcher, queue)| !queue.is_empty() && matcher(request))
            .and_then(|(_, queue)| queue.pop_front());
        if let Some(script) = routed {
            return script;
        }
        if let Some((_, script)) = inner.routes.iter().find(|(matcher, _)| matcher(request)) {
            return script.clone();
        }
        inner
            .queue
            .pop_front()
            .or_else(|| inner.fallback.clone())
            .unwrap_or_else(|| Script::text("done"))
    }
}

impl ModelClient for ScriptedModel {
    fn stream(&self, request: ModelRequest, cancel: CancellationToken) -> ModelStream {
        let script = self.next_script(&request);
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        tokio::spawn(async move {
            let (delay, events, stall, error) = match script {
                Script::Events(events) => (None, events, false, None),
                Script::Stall(events) => (None, events, true, None),
                Script::Delayed(delay, events) => (Some(delay), events, false, None),
                Script::Error(error) => (None, Vec::new(), false, Some(error)),
                Script::FailAfter(events, error) => (None, events, false, Some(error)),
            };
            if let Some(delay) = delay {
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(delay) => {}
                }
            }
            for event in events {
                if cancel.is_cancelled() || tx.send(Ok(event)).await.is_err() {
                    return;
                }
                tokio::task::yield_now().await;
            }
            if let Some(error) = error {
                let _ = tx.send(Err(error)).await;
                return;
            }
            if stall {
                cancel.cancelled().await;
            }
        });
        rx
    }

    fn provider(&self) -> &str {
        "scripted"
    }
}
