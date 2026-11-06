//! The per-call capability bundle. Everything a tool may touch arrives here;
//! tools hold no global state and never reach the engine except via ports.

use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use tokio_util::sync::CancellationToken;
use z_engine_host::{
    FileTracker, OutputSink, PathLocks, WebClient, expand_tilde, is_within, relative_display,
    resolve,
};
use z_engine_protocol::{AgentId, CallId, PermissionMode, SessionId};

use super::settings::{ShellConfig, SpillFn, ToolLimits, WebOptions};
use crate::error::ToolError;
use crate::ports::Ports;

#[derive(Clone)]
pub struct ToolCtx {
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub call_id: CallId,
    /// Project root, or the agent's worktree.
    pub root: PathBuf,
    /// Extra directories treated like the project.
    pub additional_dirs: Vec<PathBuf>,
    /// The permission level.
    pub mode: PermissionMode,
    /// Plan mode: the agent researches and proposes, and nothing that
    /// changes files or runs commands is allowed.
    pub planning: bool,
    /// The agent's persistent Bash working directory.
    pub cwd: Arc<Mutex<PathBuf>>,
    pub cancel: CancellationToken,
    /// The agent's read-before-edit tracker.
    pub files: Arc<FileTracker>,
    /// Session-wide per-path edit locks.
    pub locks: Arc<PathLocks>,
    pub shell: Arc<ShellConfig>,
    pub web: WebClient,
    pub web_options: WebOptions,
    /// Receives streamed output lines (Bash) for `toolProgress` events.
    pub progress: Option<OutputSink>,
    /// Stores full outputs that were truncated.
    pub spill: Option<SpillFn>,
    pub limits: ToolLimits,
    /// The model accepts images in tool results.
    pub vision: bool,
    pub ports: Arc<Ports>,
}

impl ToolCtx {
    /// `input` with `~` expanded, taken against the root when relative,
    /// and normalized lexically.
    pub fn resolve(&self, input: &str) -> PathBuf {
        resolve(&self.root, expand_tilde(input.trim(), None))
    }

    /// `path` relative to the root for display; outside paths in full.
    pub fn display(&self, path: &Path) -> String {
        relative_display(&self.root, path)
    }

    /// The root followed by the additional directories.
    pub fn allowed_dirs(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.root.as_path())
            .chain(self.additional_dirs.iter().map(PathBuf::as_path))
    }

    /// Whether `path` physically lies inside an allowed directory.
    pub fn is_allowed(&self, path: &Path) -> bool {
        self.allowed_dirs().any(|dir| is_within(dir, path))
    }

    /// The persistent Bash working directory. The value is replaced whole,
    /// so a poisoned lock still holds a consistent path.
    pub fn current_cwd(&self) -> PathBuf {
        self.cwd
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_cwd(&self, dir: PathBuf) {
        *self.cwd.lock().unwrap_or_else(PoisonError::into_inner) = dir;
    }

    pub fn check_cancelled(&self) -> Result<(), ToolError> {
        if self.cancel.is_cancelled() {
            Err(ToolError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Runs `work` until it finishes or the call is cancelled. Dropping the
    /// future on cancellation abandons the wait, not necessarily the remote
    /// operation, so only use it for waits that are safe to abandon.
    pub async fn until_cancelled<F: Future>(&self, work: F) -> Result<F::Output, ToolError> {
        tokio::select! {
            biased;
            () = self.cancel.cancelled() => Err(ToolError::Cancelled),
            output = work => Ok(output),
        }
    }

    /// Stores `content` through the spill hook, when there is one.
    pub fn spill_output(&self, hint: &str, content: &str) -> Option<PathBuf> {
        self.spill.as_ref().and_then(|spill| spill(hint, content))
    }
}

impl fmt::Debug for ToolCtx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolCtx")
            .field("session_id", &self.session_id)
            .field("agent_id", &self.agent_id)
            .field("call_id", &self.call_id)
            .field("root", &self.root)
            .field("additional_dirs", &self.additional_dirs)
            .field("mode", &self.mode)
            .field("planning", &self.planning)
            .field("cwd", &self.current_cwd())
            .field("cancelled", &self.cancel.is_cancelled())
            .field("shell", &self.shell)
            .field("web_options", &self.web_options)
            .field("progress", &self.progress.is_some())
            .field("spill", &self.spill.is_some())
            .field("limits", &self.limits)
            .field("vision", &self.vision)
            .field("ports", &self.ports)
            .finish_non_exhaustive()
    }
}
