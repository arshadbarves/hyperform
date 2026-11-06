//! [`LspWorker`]: owns one `LspManager` on a dedicated thread with a
//! current-thread runtime and a local task set, started on first use.
//! Some manager futures cannot be proven `Send`, so they run there and
//! only their results cross threads.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use z_engine_integrations::{LspManager, LspServerSpec};

use crate::sync::lock;

type Work = Box<dyn FnOnce(Arc<LspManager>) -> Pin<Box<dyn Future<Output = ()>>> + Send>;

enum Message {
    Run(Work),
    Shutdown(oneshot::Sender<()>),
}

#[derive(Debug)]
pub(crate) struct LspWorker {
    root: PathBuf,
    specs: Vec<LspServerSpec>,
    sender: Mutex<Option<mpsc::UnboundedSender<Message>>>,
}

impl LspWorker {
    pub(crate) fn new(root: PathBuf, specs: Vec<LspServerSpec>) -> Self {
        Self {
            root,
            specs,
            sender: Mutex::new(None),
        }
    }

    /// Runs `work` against the manager; `None` when the worker is gone.
    pub(crate) async fn run<T, F, Fut>(&self, work: F) -> Option<T>
    where
        T: Send + 'static,
        F: FnOnce(Arc<LspManager>) -> Fut + Send + 'static,
        Fut: Future<Output = T> + 'static,
    {
        let (reply, answer) = oneshot::channel();
        let work: Work = Box::new(move |manager| {
            Box::pin(async move {
                if reply.send(work(manager).await).is_err() {
                    tracing::debug!("language server answer arrived after its caller left");
                }
            })
        });
        self.sender()?.send(Message::Run(work)).ok()?;
        answer.await.ok()
    }

    /// Runs `work` against the manager without waiting for it.
    pub(crate) fn detach<F, Fut>(&self, work: F)
    where
        F: FnOnce(Arc<LspManager>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        let work: Work = Box::new(move |manager| Box::pin(work(manager)));
        let sent = self
            .sender()
            .is_some_and(|s| s.send(Message::Run(work)).is_ok());
        if !sent {
            tracing::debug!("language server worker gone; work dropped");
        }
    }

    /// Stops the servers and the thread; a worker never started is a no-op.
    pub(crate) async fn shutdown(&self) {
        let Some(sender) = lock(&self.sender).take() else {
            return;
        };
        let (done, stopped) = oneshot::channel();
        if sender.send(Message::Shutdown(done)).is_ok() && stopped.await.is_err() {
            tracing::debug!("language server worker ended before shutdown");
        }
    }

    fn sender(&self) -> Option<mpsc::UnboundedSender<Message>> {
        let mut slot = lock(&self.sender);
        if let Some(sender) = slot.as_ref().filter(|sender| !sender.is_closed()) {
            return Some(sender.clone());
        }
        let (sender, messages) = mpsc::unbounded_channel();
        let (root, specs) = (self.root.clone(), self.specs.clone());
        let spawned = std::thread::Builder::new()
            .name("zengine-lsp".into())
            .spawn(move || serve(root, specs, messages));
        if let Err(error) = spawned {
            tracing::warn!(%error, "could not start the language server worker");
            return None;
        }
        *slot = Some(sender.clone());
        Some(sender)
    }
}

fn serve(root: PathBuf, specs: Vec<LspServerSpec>, mut messages: mpsc::UnboundedReceiver<Message>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::warn!(%error, "language server worker has no runtime");
            return;
        }
    };
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let manager = Arc::new(LspManager::new(root, specs));
        let mut work_in_flight = JoinSet::new();
        let done = loop {
            while work_in_flight.try_join_next().is_some() {}
            match messages.recv().await {
                Some(Message::Run(work)) => {
                    work_in_flight.spawn_local(work(Arc::clone(&manager)));
                }
                Some(Message::Shutdown(done)) => break Some(done),
                None => break None,
            }
        };
        // A server start in flight holds its slot (initialize may take a
        // minute); aborting it kills that server and frees the slot.
        work_in_flight.shutdown().await;
        manager.shutdown_all().await;
        if done.is_some_and(|done| done.send(()).is_err()) {
            tracing::debug!("shutdown waiter left");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runs_work_on_its_thread_until_shut_down() {
        let dir = tempfile::tempdir().unwrap();
        let worker = LspWorker::new(dir.path().to_path_buf(), Vec::new());
        worker.shutdown().await;
        let root = worker
            .run(|manager| async move { manager.project_root().to_path_buf() })
            .await
            .unwrap();
        assert_eq!(root, dir.path().canonicalize().unwrap());
        let name = worker
            .run(|_| async { std::thread::current().name().map(str::to_string) })
            .await;
        assert_eq!(name, Some(Some("zengine-lsp".to_string())));
        worker.shutdown().await;
    }
}
