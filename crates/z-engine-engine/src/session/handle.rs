//! The engine's handle on a live session: the command channel into its
//! actor and the actor's task.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use z_engine_protocol::Command;

use crate::error::EngineError;
use crate::session::SessionCore;
use crate::sync::lock;

/// Bounds how long closing waits for the actor to finish its shutdown.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(crate) enum ActorMsg {
    Command(Command),
    /// Settings changed on disk; apply them.
    Reload,
    /// Stop the session; `done` fires after shutdown.
    Close {
        reason: &'static str,
        done: oneshot::Sender<()>,
    },
}

#[derive(Debug)]
pub(crate) struct SessionHandle {
    pub core: Arc<SessionCore>,
    tx: mpsc::Sender<ActorMsg>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl SessionHandle {
    pub(crate) fn new(
        core: Arc<SessionCore>,
        tx: mpsc::Sender<ActorMsg>,
        task: JoinHandle<()>,
    ) -> Self {
        Self {
            core,
            tx,
            task: Mutex::new(Some(task)),
        }
    }

    pub(crate) fn send(&self, command: Command) -> Result<(), EngineError> {
        self.tx
            .try_send(ActorMsg::Command(command))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => {
                    EngineError::Busy("too many commands are waiting".into())
                }
                mpsc::error::TrySendError::Closed(_) => EngineError::Closed(self.core.id.clone()),
            })
    }

    pub(crate) fn reload(&self) {
        if self.tx.try_send(ActorMsg::Reload).is_err() {
            tracing::debug!(session = %self.core.id, "reload not delivered; session closing or busy");
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }

    /// Asks the actor to shut down and waits for it (bounded).
    pub(crate) async fn close(&self, reason: &'static str) {
        let (done, finished) = oneshot::channel();
        if self.tx.send(ActorMsg::Close { reason, done }).await.is_ok()
            && tokio::time::timeout(CLOSE_TIMEOUT, finished).await.is_err()
        {
            tracing::warn!(session = %self.core.id, "session shutdown timed out");
        }
        let task = lock(&self.task).take();
        if let Some(task) = task {
            match tokio::time::timeout(CLOSE_TIMEOUT, task).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::error!(session = %self.core.id, %error, "session actor failed");
                }
                Err(_) => tracing::warn!(session = %self.core.id, "session actor did not stop"),
            }
        }
    }
}
