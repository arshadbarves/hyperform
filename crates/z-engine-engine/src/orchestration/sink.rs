//! A subagent's transcript: messages go to `agents/<id>.jsonl`, never to
//! the main transcript, and the working set lives in memory. The agent
//! transcript stores messages only, so a summary compaction replaces the
//! in-memory working set and a resumed agent starts from the full history.

use std::sync::Mutex;

use z_engine_protocol::{AgentId, CompactionMarker, CompactionTrigger, Message};
use z_engine_store::AgentLog;

use crate::error::EngineError;
use crate::run::{ContextMeter, TranscriptSink};
use crate::session::SessionCore;
use crate::sync::lock;

#[derive(Debug)]
pub(crate) struct AgentSink {
    log: Mutex<AgentLog>,
    working: Mutex<Vec<Message>>,
    meter: Mutex<ContextMeter>,
}

impl AgentSink {
    /// Appends to the agent's transcript file; `history` seeds the working
    /// set of a resumed agent.
    pub(crate) fn open(
        core: &SessionCore,
        agent_id: &AgentId,
        history: Vec<Message>,
    ) -> Result<Self, EngineError> {
        let log = core.shared.store.agent_log(&core.id, agent_id)?;
        Ok(Self {
            log: Mutex::new(log),
            working: Mutex::new(history),
            meter: Mutex::new(ContextMeter::default()),
        })
    }

    pub(crate) fn sync(&self) {
        if let Err(error) = lock(&self.log).sync() {
            tracing::warn!(%error, "could not flush a subagent transcript");
        }
    }
}

impl TranscriptSink for AgentSink {
    fn working(&self) -> Vec<Message> {
        lock(&self.working).clone()
    }

    fn append(&self, message: &Message, _steering: bool) -> Result<(), EngineError> {
        lock(&self.log).append_message(message)?;
        lock(&self.working).push(message.clone());
        Ok(())
    }

    fn set_working(&self, working: Vec<Message>) {
        *lock(&self.working) = working;
    }

    fn compacting(&self, _trigger: CompactionTrigger) {}

    fn compacted(
        &self,
        _marker: CompactionMarker,
        _summary: Message,
        working: Vec<Message>,
    ) -> Result<(), EngineError> {
        *lock(&self.working) = working;
        lock(&self.meter).reset();
        Ok(())
    }

    fn meter(&self) -> ContextMeter {
        *lock(&self.meter)
    }

    fn save_meter(&self, meter: ContextMeter) {
        *lock(&self.meter) = meter;
    }
}
