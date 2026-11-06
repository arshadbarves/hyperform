//! Application state managed by Tauri: the engine, the active project the
//! git and worktree panels act on, the workspace registry and the pet's
//! growth file.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use z_engine_engine::Engine;

use crate::ipc::IpcResult;
use crate::pet::PetStore;
use crate::workspaces::Workspaces;

#[derive(Debug)]
pub(crate) struct AppState {
    pub(crate) engine: Engine,
    pub(crate) workspaces: Workspaces,
    pub(crate) pet: PetStore,
    active: Mutex<PathBuf>,
}

impl AppState {
    pub(crate) fn new(
        engine: Engine,
        workspaces: Workspaces,
        pet: PetStore,
        active: PathBuf,
    ) -> Self {
        Self {
            engine,
            workspaces,
            pet,
            active: Mutex::new(active),
        }
    }

    pub(crate) fn active_root(&self) -> PathBuf {
        self.active
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn set_active(&self, root: PathBuf) {
        *self.active.lock().unwrap_or_else(PoisonError::into_inner) = root;
    }

    /// Project roots the app may edit files in: the active project and
    /// every registered workspace.
    pub(crate) fn known_roots(&self) -> Vec<PathBuf> {
        let mut roots = self.workspaces.load();
        let active = self.active_root();
        if !roots.contains(&active) {
            roots.push(active);
        }
        roots
    }

    /// `projectRoot` from the webview, required.
    pub(crate) fn root_arg(project_root: &str) -> IpcResult<PathBuf> {
        let root = project_root.trim();
        if root.is_empty() {
            return Err("a project root is required".to_string());
        }
        Ok(PathBuf::from(root))
    }
}
