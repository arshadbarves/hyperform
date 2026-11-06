//! A subagent's live `AgentInfo`. Every change is persisted as
//! `LogRecord::AgentUpdated`, mirrored into the session state and
//! announced with `AgentUpdated`; usage, tool-call and text updates are
//! throttled while the agent runs, status changes never are.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyperform_protocol::{AgentInfo, AgentStatus, Event, Usage, now_ms};
use hyperform_store::LogRecord;

use crate::run::WebTaint;
use crate::session::SessionCore;
use crate::sync::lock;

const UPDATE_INTERVAL: Duration = Duration::from_millis(500);
const PREVIEW_CHARS: usize = 400;

#[derive(Debug)]
pub(crate) struct AgentTracker {
    info: Mutex<AgentInfo>,
    /// Text of the latest assistant message (the report once finished).
    latest_text: Mutex<String>,
    last_publish: Mutex<Option<Instant>>,
    /// The agent's run's taint, shared with its `RunContext`.
    web: Arc<WebTaint>,
    /// The run ends at its next round instead of going on.
    stop: AtomicBool,
}

impl AgentTracker {
    pub(crate) fn new(info: AgentInfo, web: Arc<WebTaint>) -> Self {
        Self {
            info: Mutex::new(info),
            latest_text: Mutex::new(String::new()),
            last_publish: Mutex::new(None),
            web,
            stop: AtomicBool::new(false),
        }
    }

    pub(crate) fn info(&self) -> AgentInfo {
        lock(&self.info).clone()
    }

    /// Ends the run at its next round boundary (its calls in flight finish).
    pub(crate) fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub(crate) fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// The agent's run has read web content, so its report carries it.
    pub(crate) fn read_web(&self) -> bool {
        self.web.is_set()
    }

    pub(crate) fn latest_text(&self) -> String {
        lock(&self.latest_text).clone()
    }

    /// Records the agent and emits `AgentStarted`.
    pub(crate) fn announce(&self, core: &SessionCore) {
        let info = self.info();
        publish(core, &info, true);
        *lock(&self.last_publish) = Some(Instant::now());
    }

    pub(crate) fn add_usage(&self, core: &SessionCore, usage: Usage, cost_usd: f64) {
        self.change(core, false, |info| {
            info.usage += usage;
            info.cost_usd += cost_usd;
        });
    }

    pub(crate) fn add_tool_calls(&self, core: &SessionCore, count: u32) {
        self.change(core, false, |info| {
            info.tool_calls = info.tool_calls.saturating_add(count);
        });
    }

    pub(crate) fn note_text(&self, core: &SessionCore, text: &str) {
        *lock(&self.latest_text) = text.to_string();
        let preview = preview(text);
        self.change(core, false, |info| info.result_preview = Some(preview));
    }

    /// Blocked on (or released from) an approval.
    pub(crate) fn set_waiting(&self, core: &SessionCore, waiting: bool) {
        let status = if waiting {
            AgentStatus::Waiting
        } else {
            AgentStatus::Running
        };
        self.change(core, true, |info| {
            if !info.status.is_terminal() {
                info.status = status;
            }
        });
    }

    /// Terminal update: status, final numbers, and `finished_at`.
    pub(crate) fn finish(&self, core: &SessionCore, apply: impl FnOnce(&mut AgentInfo)) {
        self.change(core, true, |info| {
            apply(info);
            info.finished_at = Some(now_ms());
        });
    }

    fn change(&self, core: &SessionCore, force: bool, apply: impl FnOnce(&mut AgentInfo)) {
        let info = {
            let mut info = lock(&self.info);
            apply(&mut info);
            info.read_web |= self.web.is_set();
            info.clone()
        };
        let now = Instant::now();
        let due = {
            let mut last = lock(&self.last_publish);
            let due = force || last.is_none_or(|at| now.duration_since(at) >= UPDATE_INTERVAL);
            if due {
                *last = Some(now);
            }
            due
        };
        if due {
            publish(core, &info, false);
        }
    }
}

/// Persists `info`, mirrors it into the session state and announces it.
pub(crate) fn publish(core: &SessionCore, info: &AgentInfo, started: bool) {
    core.journal
        .append_or_report(&LogRecord::AgentUpdated { info: info.clone() });
    core.with_state(|state| {
        match state
            .agents
            .iter_mut()
            .find(|agent| agent.agent_id == info.agent_id)
        {
            Some(agent) => *agent = info.clone(),
            None => state.agents.push(info.clone()),
        }
    });
    let info = info.clone();
    core.events.emit(if started {
        Event::AgentStarted { info }
    } else {
        Event::AgentUpdated { info }
    });
}

fn preview(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= PREVIEW_CHARS {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(PREVIEW_CHARS).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_are_bounded() {
        assert_eq!(preview("  short  "), "short");
        let long = "x".repeat(PREVIEW_CHARS + 10);
        assert_eq!(preview(&long).chars().count(), PREVIEW_CHARS + 1);
    }
}
