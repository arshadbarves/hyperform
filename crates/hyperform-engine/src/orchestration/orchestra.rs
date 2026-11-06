//! Session-wide subagent bookkeeping: the agent registry (replaced on
//! settings reload), the concurrency slots, and the lock that serializes
//! worktree merges.

use std::sync::{Arc, RwLock};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::registry::AgentRegistry;
use crate::sync::{read, write};

#[derive(Debug)]
pub(crate) struct Orchestra {
    registry: RwLock<Arc<AgentRegistry>>,
    slots: Arc<Semaphore>,
    merge: tokio::sync::Mutex<()>,
}

impl Orchestra {
    /// `max_concurrent` is read once per session.
    pub(crate) fn new(registry: AgentRegistry, max_concurrent: u32) -> Self {
        Self {
            registry: RwLock::new(Arc::new(registry)),
            slots: Arc::new(Semaphore::new(max_concurrent.max(1) as usize)),
            merge: tokio::sync::Mutex::new(()),
        }
    }

    pub(crate) fn registry(&self) -> Arc<AgentRegistry> {
        Arc::clone(&read(&self.registry))
    }

    pub(crate) fn set_registry(&self, registry: AgentRegistry) {
        *write(&self.registry) = Arc::new(registry);
    }

    /// Waits for a free slot; `None` once the session closed the slots.
    pub(crate) async fn slot(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.slots).acquire_owned().await.ok()
    }

    pub(crate) fn close_slots(&self) {
        self.slots.close();
    }

    pub(crate) async fn merge_lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.merge.lock().await
    }
}
