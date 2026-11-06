//! [`LspManager`]: the language servers of one project, started lazily for
//! the files asked about, with cached start failures, a bounded number of
//! restarts after crashes, status for the GUI, and shutdown.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::client::LspClient;
use super::routing::{Route, Router};
use super::spec::LspServerSpec;
use super::types::FileDiagnostics;
use super::uri::plain_path;
use crate::error::IntegrationError;
use crate::process::StderrLog;
use crate::sync::lock;

/// Restarts allowed after a server exits on its own.
const MAX_RESTARTS: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspServerState {
    Starting,
    Running,
    /// Failed to start, or exited; cached so later calls fail fast.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerStatus {
    pub name: String,
    /// The workspace root the server serves.
    pub root: PathBuf,
    pub command: PathBuf,
    pub state: LspServerState,
    pub open_documents: usize,
    pub stderr_tail: Vec<String>,
}

#[derive(Debug)]
enum SlotState {
    Idle,
    Running(Arc<LspClient>),
    Failed(String),
}

/// One (server, workspace root) pair.
#[derive(Debug)]
struct Slot {
    route: Route,
    state: tokio::sync::Mutex<SlotState>,
    restarts: AtomicU32,
    stderr: StderrLog,
}

impl Slot {
    async fn client(&self) -> Result<Arc<LspClient>, IntegrationError> {
        let name = &self.route.spec.name;
        let mut state = self.state.lock().await;
        match std::mem::replace(&mut *state, SlotState::Idle) {
            SlotState::Running(client) if !client.is_closed() => {
                *state = SlotState::Running(Arc::clone(&client));
                return Ok(client);
            }
            SlotState::Running(client) => {
                let reason = client.close_reason().unwrap_or_default();
                client.shutdown().await;
                if self.restarts.fetch_add(1, Ordering::SeqCst) >= MAX_RESTARTS {
                    let message = format!("{name} exited ({reason}) after {MAX_RESTARTS} restarts");
                    *state = SlotState::Failed(message.clone());
                    return Err(IntegrationError::Unsupported(message));
                }
                tracing::warn!(server = %name, %reason, "language server exited; restarting");
            }
            SlotState::Failed(reason) => {
                let error =
                    IntegrationError::Unsupported(format!("{name} is unavailable: {reason}"));
                *state = SlotState::Failed(reason);
                return Err(error);
            }
            SlotState::Idle => {}
        }
        let route = &self.route;
        match LspClient::start_with_log(
            &route.spec,
            &route.root,
            &route.program,
            self.stderr.clone(),
        )
        .await
        {
            Ok(client) => {
                let client = Arc::new(client);
                *state = SlotState::Running(Arc::clone(&client));
                Ok(client)
            }
            Err(error) => {
                *state = SlotState::Failed(error.to_string());
                Err(error)
            }
        }
    }

    /// The client when it is up; `None` while idle, starting (the state is
    /// locked), restarting or failed.
    fn running(&self) -> Option<Arc<LspClient>> {
        match self.state.try_lock().as_deref() {
            Ok(SlotState::Running(client)) if !client.is_closed() => Some(Arc::clone(client)),
            _ => None,
        }
    }

    fn status(&self) -> LspServerStatus {
        let (state, open_documents) = match self.state.try_lock() {
            Err(_) => (LspServerState::Starting, 0),
            Ok(guard) => match &*guard {
                SlotState::Idle => (LspServerState::Starting, 0),
                SlotState::Running(client) => match client.close_reason() {
                    Some(reason) => (LspServerState::Failed(format!("exited: {reason}")), 0),
                    None => (LspServerState::Running, client.open_documents()),
                },
                SlotState::Failed(reason) => (LspServerState::Failed(reason.clone()), 0),
            },
        };
        LspServerStatus {
            name: self.route.spec.name.clone(),
            root: self.route.root.clone(),
            command: self.route.program.clone(),
            state,
            open_documents,
            stderr_tail: self.stderr.tail(),
        }
    }

    async fn shutdown(&self) {
        let previous = std::mem::replace(&mut *self.state.lock().await, SlotState::Idle);
        if let SlotState::Running(client) = previous {
            client.shutdown().await;
        }
    }
}

#[derive(Debug)]
pub struct LspManager {
    root: PathBuf,
    router: Router,
    slots: Mutex<Vec<Arc<Slot>>>,
}

impl LspManager {
    /// Nothing starts until a file is asked about. The project root is
    /// canonicalized so server paths and ours compare equal.
    pub fn new(project_root: impl Into<PathBuf>, specs: Vec<LspServerSpec>) -> Self {
        let root: PathBuf = project_root.into();
        let root = std::fs::canonicalize(&root).map_or(root, plain_path);
        Self {
            router: Router::new(root.clone(), specs),
            root,
            slots: Mutex::new(Vec::new()),
        }
    }

    pub fn project_root(&self) -> &Path {
        &self.root
    }

    /// The client serving `file` (canonical, inside the project), started
    /// on first use.
    pub(crate) async fn client_for(&self, file: &Path) -> Result<Arc<LspClient>, IntegrationError> {
        let route = self.router.route(file).await?;
        self.slot(route).client().await
    }

    fn slot(&self, route: Route) -> Arc<Slot> {
        let mut slots = lock(&self.slots);
        let existing = slots
            .iter()
            .find(|slot| slot.route.spec.name == route.spec.name && slot.route.root == route.root);
        if let Some(slot) = existing {
            return Arc::clone(slot);
        }
        let slot = Arc::new(Slot {
            route,
            state: tokio::sync::Mutex::new(SlotState::Idle),
            restarts: AtomicU32::new(0),
            stderr: StderrLog::default(),
        });
        slots.push(Arc::clone(&slot));
        slot
    }

    fn slots(&self) -> Vec<Arc<Slot>> {
        lock(&self.slots).clone()
    }

    /// Clients that are up (for workspace-wide queries); when none is,
    /// starts the servers whose root markers sit in the project root.
    pub(crate) async fn workspace_clients(&self) -> Vec<Arc<LspClient>> {
        let running: Vec<Arc<LspClient>> = self
            .slots()
            .iter()
            .filter_map(|slot| slot.running())
            .collect();
        if !running.is_empty() {
            return running;
        }
        let mut started = Vec::new();
        for route in self.router.project_routes().await {
            match self.slot(route).client().await {
                Ok(client) => started.push(client),
                Err(error) => tracing::warn!(%error, "could not start a project language server"),
            }
        }
        started
    }

    /// `path` (relative paths are taken against the project root) as a
    /// canonical file inside the project.
    pub(crate) async fn resolve_file(&self, path: &Path) -> Result<PathBuf, IntegrationError> {
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let canonical = tokio::fs::canonicalize(&joined)
            .await
            .map(plain_path)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    IntegrationError::NotFound(joined.display().to_string())
                }
                _ => IntegrationError::io(format!("resolving {}", joined.display()), e),
            })?;
        if !canonical.starts_with(&self.root) {
            return Err(IntegrationError::Unsupported(format!(
                "{} is outside the project",
                joined.display()
            )));
        }
        Ok(canonical)
    }

    /// Diagnostics of a file the caller just wrote, only from a server that
    /// is already up and only if they come quickly (see
    /// [`LspClient::diagnostics_if_quick`]). `None` when no server for the
    /// file is running yet; [`LspManager::start_for`] brings one up.
    pub async fn diagnostics_if_running(
        &self,
        path: &Path,
        wait: Duration,
    ) -> Result<Option<FileDiagnostics>, IntegrationError> {
        let path = self.resolve_file(path).await?;
        let route = self.router.route(&path).await?;
        let Some(client) = self.slot(route).running() else {
            return Ok(None);
        };
        client.diagnostics_if_quick(&path, wait).await.map(Some)
    }

    /// Starts the server for `path` when needed and syncs the file, so the
    /// server analyzes it before it is next asked about.
    pub async fn start_for(&self, path: &Path) -> Result<(), IntegrationError> {
        let path = self.resolve_file(path).await?;
        self.client_for(&path).await?.sync(&path).await.map(|_| ())
    }

    /// Every server started so far, including failed ones.
    pub fn status(&self) -> Vec<LspServerStatus> {
        self.slots().iter().map(|slot| slot.status()).collect()
    }

    /// Shuts every server down and forgets cached failures.
    pub async fn shutdown_all(&self) {
        let slots = std::mem::take(&mut *lock(&self.slots));
        futures::future::join_all(slots.iter().map(|slot| slot.shutdown())).await;
    }
}
