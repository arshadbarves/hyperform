//! Where a run's transcript goes. The main agent writes the session log
//! and the session state; subagents (later phases) write their own
//! transcript file behind the same trait.

use std::sync::Arc;

use z_engine_protocol::{
    CompactionMarker, CompactionTrigger, Event, Message, Role, TurnId, now_ms,
};
use z_engine_store::LogRecord;

use super::meter::ContextMeter;
use crate::error::EngineError;
use crate::session::SessionCore;

pub(crate) trait TranscriptSink: Send + Sync {
    /// The working set the model sees.
    fn working(&self) -> Vec<Message>;

    /// Persists `message`; user messages are announced (`steering` marks
    /// messages that carry the user's queued notes).
    fn append(&self, message: &Message, steering: bool) -> Result<(), EngineError>;

    /// Replaces the working set in memory only (microcompaction).
    fn set_working(&self, working: Vec<Message>);

    /// A summary compaction is about to ask the model for a summary.
    fn compacting(&self, trigger: CompactionTrigger);

    /// Persists a summary compaction and installs the new working set.
    fn compacted(
        &self,
        marker: CompactionMarker,
        summary: Message,
        working: Vec<Message>,
    ) -> Result<(), EngineError>;

    fn meter(&self) -> ContextMeter;

    fn save_meter(&self, meter: ContextMeter);
}

/// The main agent: session log, session state, and GUI events.
#[derive(Debug)]
pub(crate) struct MainSink {
    core: Arc<SessionCore>,
    turn_id: Option<TurnId>,
}

impl MainSink {
    pub(crate) fn new(core: Arc<SessionCore>, turn_id: Option<TurnId>) -> Self {
        Self { core, turn_id }
    }
}

impl TranscriptSink for MainSink {
    fn working(&self) -> Vec<Message> {
        self.core.with_state(|state| state.working.clone())
    }

    fn append(&self, message: &Message, steering: bool) -> Result<(), EngineError> {
        self.core.journal.append(&LogRecord::Message {
            message: message.clone(),
            turn_id: self.turn_id.clone(),
        })?;
        self.core.with_state(|state| {
            state.transcript.push(message.clone());
            state.working.push(message.clone());
            if let Some(full) = &mut state.full_working {
                full.push(message.clone());
            }
            state.updated_at = now_ms();
        });
        if message.role == Role::User {
            self.core.events.emit(Event::UserMessage {
                message: message.clone(),
                turn_id: self.turn_id.clone(),
                steering,
            });
        }
        Ok(())
    }

    fn set_working(&self, working: Vec<Message>) {
        self.core.with_state(|state| state.working = working);
    }

    fn compacting(&self, trigger: CompactionTrigger) {
        self.core.events.emit(Event::CompactionStarted { trigger });
    }

    fn compacted(
        &self,
        marker: CompactionMarker,
        summary: Message,
        working: Vec<Message>,
    ) -> Result<(), EngineError> {
        self.core.journal.append(&LogRecord::Compacted {
            marker: marker.clone(),
            summary,
        })?;
        self.core.with_state(|state| {
            state.working = working;
            state.full_working = None;
            state.compactions.push(marker.clone());
            state.meter.reset();
        });
        self.core.repo_map.invalidate();
        self.core.events.emit(Event::Compacted { marker });
        Ok(())
    }

    fn meter(&self) -> ContextMeter {
        self.core.with_state(|state| state.meter)
    }

    fn save_meter(&self, meter: ContextMeter) {
        self.core.with_state(|state| state.meter = meter);
    }
}
