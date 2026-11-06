//! Collects engine events and waits for conditions with a timeout.

use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;
use z_engine_protocol::{Event, EventEnvelope};

#[derive(Debug)]
pub struct EventRecorder {
    rx: UnboundedReceiver<EventEnvelope>,
    seen: Vec<Event>,
}

pub const DEFAULT_WAIT: Duration = Duration::from_secs(10);

impl EventRecorder {
    pub fn new(rx: UnboundedReceiver<EventEnvelope>) -> Self {
        Self {
            rx,
            seen: Vec::new(),
        }
    }

    /// Every event received so far.
    pub fn seen(&self) -> &[Event] {
        &self.seen
    }

    /// Receive until `predicate` matches an event; returns that event.
    /// Panics with the recorded history on timeout.
    pub async fn wait_for(&mut self, predicate: impl Fn(&Event) -> bool) -> Event {
        self.wait_for_within(DEFAULT_WAIT, predicate).await
    }

    pub async fn wait_for_within(
        &mut self,
        timeout: Duration,
        predicate: impl Fn(&Event) -> bool,
    ) -> Event {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match tokio::time::timeout_at(deadline, self.rx.recv()).await {
                Ok(Some(envelope)) => {
                    let matched = predicate(&envelope.event);
                    self.seen.push(envelope.event.clone());
                    if matched {
                        return envelope.event;
                    }
                }
                Ok(None) => panic!("event channel closed; history: {:#?}", self.seen),
                Err(_) => panic!("timed out waiting for event; history: {:#?}", self.seen),
            }
        }
    }

    /// Wait for the main agent's turn to finish.
    pub async fn wait_turn_finished(&mut self) -> Event {
        self.wait_for(|event| matches!(event, Event::TurnFinished { .. }))
            .await
    }

    /// Drain whatever is immediately available.
    pub fn drain(&mut self) -> &[Event] {
        while let Ok(envelope) = self.rx.try_recv() {
            self.seen.push(envelope.event);
        }
        &self.seen
    }

    pub fn count(&self, predicate: impl Fn(&Event) -> bool) -> usize {
        self.seen.iter().filter(|event| predicate(event)).count()
    }
}
