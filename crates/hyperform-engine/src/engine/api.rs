//! [`Engine`]: the process-wide entry point the GUI backend calls. It owns
//! the shared services and the table of live sessions; each session's
//! actor does the work.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use futures::future::join_all;
use hyperform_config::{LoadedSettings, Paths, load_with_env};
use hyperform_host::WebClient;
use hyperform_llm::ModelCatalog;
use hyperform_protocol::{AgentId, Command, Message, SessionId, SessionSummary};
use hyperform_store::SessionStore;
use serde_json::Value;

use super::{catalog, export};
use crate::decisions::Sidecars;
use crate::error::EngineError;
use crate::options::{EngineOptions, EventSink, ExportFormat};
use crate::run::inspect_request;
use crate::session::{SessionHandle, Shared, emit_snapshot, open_session};
use crate::settings::models;
use crate::sync::{lock, read};
use crate::work::PlanHub;

/// Cheap to clone; clones share every session.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

struct Inner {
    shared: Shared,
    sink: EventSink,
    sessions: Mutex<HashMap<SessionId, Arc<SessionHandle>>>,
    /// Serializes opening and closing so a session is never opened twice.
    lifecycle: tokio::sync::Mutex<()>,
    shut_down: AtomicBool,
}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Engine")
            .field("paths", &self.inner.shared.paths)
            .field("sessions", &lock(&self.inner.sessions).len())
            .finish_non_exhaustive()
    }
}

impl Engine {
    /// Creates the directories (importing v1 user settings once) and loads
    /// the cached model catalog when one exists.
    pub fn new(options: EngineOptions) -> Result<Engine, EngineError> {
        let EngineOptions {
            paths,
            event_sink,
            client_factory,
            env,
        } = options;
        let migration = paths.ensure()?;
        for note in &migration.notes {
            tracing::warn!(%note, "user settings migration");
        }
        let catalog = catalog::load_cached(&paths).map(Arc::new);
        let shared = Shared {
            store: SessionStore::new(paths.sessions_dir.clone()),
            plans: PlanHub::new(paths.plans_dir.clone()),
            paths,
            env,
            factory: client_factory,
            catalog: Arc::new(RwLock::new(catalog)),
            web: WebClient::new()?,
            sidecars: Sidecars::default(),
        };
        Ok(Engine {
            inner: Arc::new(Inner {
                shared,
                sink: event_sink,
                sessions: Mutex::new(HashMap::new()),
                lifecycle: tokio::sync::Mutex::new(()),
                shut_down: AtomicBool::new(false),
            }),
        })
    }

    /// Opens a new session (`None`) or resumes a stored one. Opening a live
    /// session again only re-sends its `Snapshot`.
    pub async fn open_session(
        &self,
        project_root: &Path,
        session_id: Option<SessionId>,
    ) -> Result<SessionId, EngineError> {
        self.ensure_running()?;
        let _guard = self.inner.lifecycle.lock().await;
        if let Some(id) = &session_id {
            if let Some(handle) = self.handle(id) {
                if !handle.is_closed() {
                    emit_snapshot(&handle.core);
                    return Ok(id.clone());
                }
                lock(&self.inner.sessions).remove(id);
                handle.close("reopen").await;
            }
        }
        let shared = &self.inner.shared;
        let handle =
            open_session(shared, self.inner.sink.clone(), project_root, session_id).await?;
        let id = handle.core.id.clone();
        lock(&self.inner.sessions).insert(id.clone(), Arc::new(handle));
        Ok(id)
    }

    pub fn send(&self, session_id: &SessionId, command: Command) -> Result<(), EngineError> {
        self.handle(session_id)
            .ok_or_else(|| EngineError::NotOpen(session_id.clone()))?
            .send(command)
    }

    /// Cancels the running turn, runs `SessionEnd` hooks, stops background
    /// jobs, and flushes the log.
    pub async fn close_session(&self, session_id: &SessionId) -> Result<(), EngineError> {
        let _guard = self.inner.lifecycle.lock().await;
        let handle = lock(&self.inner.sessions)
            .remove(session_id)
            .ok_or_else(|| EngineError::NotOpen(session_id.clone()))?;
        handle.close("close").await;
        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>, EngineError> {
        Ok(self.inner.shared.store.list()?)
    }

    pub async fn delete_session(&self, session_id: &SessionId) -> Result<(), EngineError> {
        let _guard = self.inner.lifecycle.lock().await;
        let live = lock(&self.inner.sessions).remove(session_id);
        if let Some(handle) = live {
            handle.close("delete").await;
        }
        Ok(self.inner.shared.store.delete(session_id)?)
    }

    /// The main agent's display transcript, or a subagent's transcript.
    pub fn agent_transcript(
        &self,
        session_id: &SessionId,
        agent_id: &AgentId,
    ) -> Result<Vec<Message>, EngineError> {
        let store = &self.inner.shared.store;
        if !agent_id.is_main() {
            return Ok(store.load_agent_transcript(session_id, agent_id)?);
        }
        match self.handle(session_id) {
            Some(handle) => Ok(handle.core.with_state(|state| state.transcript.clone())),
            None => Ok(store.load(session_id)?.state.transcript),
        }
    }

    pub fn export_session(
        &self,
        session_id: &SessionId,
        format: ExportFormat,
    ) -> Result<String, EngineError> {
        let live = self.handle(session_id);
        let core = live.as_ref().map(|handle| &*handle.core);
        export::export(&self.inner.shared, core, session_id, format)
    }

    /// The last main-agent request of a live session, for the prompt
    /// inspector.
    pub fn last_request(&self, session_id: &SessionId) -> Option<Value> {
        let handle = self.handle(session_id)?;
        let request = lock(&handle.core.last_request).clone()?;
        Some(inspect_request(&request))
    }

    pub fn settings(&self, project_root: Option<&Path>) -> LoadedSettings {
        let shared = &self.inner.shared;
        load_with_env(&shared.paths, project_root, &shared.env)
    }

    /// Live sessions of `project_root` (all when `None`) reload settings,
    /// client, policy, extensions and instructions.
    pub fn reload_settings(&self, project_root: Option<&Path>) {
        let root = project_root
            .map(|root| std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()));
        for handle in self.handles() {
            if root.as_ref().is_none_or(|root| handle.core.root == *root) {
                handle.reload();
            }
        }
    }

    pub fn paths(&self) -> &Paths {
        &self.inner.shared.paths
    }

    pub fn catalog(&self) -> Option<Arc<ModelCatalog>> {
        read(&self.inner.shared.catalog).clone()
    }

    /// True when the model catalog was never downloaded or is a day old;
    /// callers refresh it in the background and keep using the cache.
    pub fn catalog_is_stale(&self) -> bool {
        catalog::is_stale(&self.inner.shared.paths)
    }

    /// Downloads models.dev, caches it, merges `models.json` overrides, and
    /// updates the context windows of live sessions.
    pub async fn refresh_catalog(&self) -> Result<Arc<ModelCatalog>, EngineError> {
        let catalog = catalog::refresh(&self.inner.shared).await?;
        for handle in self.handles() {
            let core = &handle.core;
            let settings = core.settings();
            let model = core.main_model();
            let limit = models::context_window(&settings.settings, Some(&catalog), &model);
            core.with_state(|state| state.context_limit = limit);
        }
        Ok(catalog)
    }

    /// Closes every session (turns cancelled, background jobs killed, logs
    /// flushed) and stops the decision sidecar. Later opens fail with
    /// `ShutDown`.
    pub async fn shutdown(&self) {
        self.inner.shut_down.store(true, Ordering::SeqCst);
        let _guard = self.inner.lifecycle.lock().await;
        let handles: Vec<Arc<SessionHandle>> = lock(&self.inner.sessions)
            .drain()
            .map(|(_, handle)| handle)
            .collect();
        join_all(handles.iter().map(|handle| handle.close("shutdown"))).await;
        self.inner.shared.sidecars.shutdown().await;
    }

    pub(super) fn handle(&self, id: &SessionId) -> Option<Arc<SessionHandle>> {
        lock(&self.inner.sessions).get(id).cloned()
    }

    pub(super) fn handles(&self) -> Vec<Arc<SessionHandle>> {
        lock(&self.inner.sessions).values().cloned().collect()
    }

    pub(super) fn shared(&self) -> &Shared {
        &self.inner.shared
    }

    fn ensure_running(&self) -> Result<(), EngineError> {
        if self.inner.shut_down.load(Ordering::SeqCst) {
            Err(EngineError::ShutDown)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send<T: Send>(_: &T) {}

    /// The GUI backend shares the engine across threads and awaits its
    /// futures from async commands.
    #[test]
    fn engine_and_its_futures_are_thread_safe() {
        fn shared<T: Send + Sync + Clone + 'static>() {}
        shared::<Engine>();
        let dirs = tempfile::tempdir().unwrap();
        let engine = Engine::new(EngineOptions {
            paths: Paths::with_roots(dirs.path().join("c"), dirs.path().join("d")),
            event_sink: Arc::new(|_| {}),
            client_factory: None,
            env: hyperform_config::EnvOverrides::default(),
        })
        .unwrap();
        let id = SessionId::new();
        send(&engine.open_session(dirs.path(), None));
        send(&engine.close_session(&id));
        send(&engine.delete_session(&id));
        send(&engine.refresh_catalog());
        send(&engine.shutdown());
        assert!(engine.catalog().is_none());
        assert!(engine.list_sessions().unwrap().is_empty());
        assert!(matches!(
            engine.send(&id, Command::Cancel),
            Err(EngineError::NotOpen(_))
        ));
    }
}
