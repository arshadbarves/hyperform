//! Streamed tool output as `ToolProgress`, coalesced so one call emits at
//! most about twenty events per second. The remainder is flushed before
//! the call's `ToolFinished`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use z_engine_host::OutputSink;
use z_engine_protocol::{AgentId, CallId, Event};

use crate::session::Emitter;
use crate::sync::lock;

const MIN_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone)]
pub(super) struct Progress {
    events: Arc<Emitter>,
    agent_id: AgentId,
    call_id: CallId,
    state: Arc<Mutex<Buffered>>,
}

#[derive(Debug, Default)]
struct Buffered {
    text: String,
    last_emit: Option<Instant>,
}

impl Progress {
    pub(super) fn new(events: Arc<Emitter>, agent_id: AgentId, call_id: CallId) -> Self {
        Self {
            events,
            agent_id,
            call_id,
            state: Arc::default(),
        }
    }

    pub(super) fn sink(&self) -> OutputSink {
        let progress = self.clone();
        Arc::new(move |text: &str| progress.push(text))
    }

    /// Emission happens under the buffer lock so chunks keep their order.
    fn push(&self, text: &str) {
        let mut state = lock(&self.state);
        state.text.push_str(text);
        let due = state
            .last_emit
            .is_none_or(|at| at.elapsed() >= MIN_INTERVAL);
        if due {
            state.last_emit = Some(Instant::now());
            let text = std::mem::take(&mut state.text);
            self.emit(text);
        }
    }

    pub(super) fn flush(&self) {
        let mut state = lock(&self.state);
        let text = std::mem::take(&mut state.text);
        if !text.is_empty() {
            self.emit(text);
        }
    }

    fn emit(&self, text: String) {
        self.events.emit(Event::ToolProgress {
            agent_id: self.agent_id.clone(),
            call_id: self.call_id.clone(),
            text,
        });
    }
}
