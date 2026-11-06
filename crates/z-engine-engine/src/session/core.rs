//! `SessionCore`: the state and services one session shares between its
//! actor, its agent runs, and the tool ports. Locks are short-lived and
//! never held across an `.await`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use tokio_util::sync::CancellationToken;
use z_engine_config::{EnvOverrides, Paths};
use z_engine_context::GitInfo;
use z_engine_host::{FileTracker, PathLocks, WebClient};
use z_engine_llm::{ModelCatalog, ModelClient, ModelRequest};
use z_engine_policy::Policy;
use z_engine_protocol::{PermissionMode, SessionId};
use z_engine_store::SessionStore;
use z_engine_tools::ToolRegistry;

use super::checkpoint::Checkpoints;
use crate::broker::Broker;
use crate::decisions::{DecisionHub, Sidecars};
use crate::hooks::HookEnv;
use crate::lsp::LspHub;
use crate::mcp::McpHub;
use crate::options::ClientFactory;
use crate::orchestration::Orchestra;
use crate::run::RepoMapCache;
use crate::session::{Emitter, JobHub, Journal, ReminderBox, SessionState, StatusTracker};
use crate::settings::SessionSettings;
use crate::sync::{lock, read};
use crate::verify::{CheckHub, Verifier};
use crate::work::{PlanHub, WorkSession};

/// The engine-wide model catalog, replaced whole on refresh.
pub(crate) type CatalogHandle = Arc<RwLock<Option<Arc<ModelCatalog>>>>;

/// Per-agent working context that survives across turns.
#[derive(Debug, Clone)]
pub(crate) struct AgentResources {
    /// Read-before-edit tracking of this agent.
    pub files: FileTracker,
    /// The agent's persistent shell working directory.
    pub cwd: Arc<Mutex<PathBuf>>,
    /// Nested instruction files already shown to this agent.
    pub seen_instructions: Arc<Mutex<HashSet<String>>>,
}

impl AgentResources {
    pub(crate) fn new(cwd: PathBuf) -> Self {
        Self {
            files: FileTracker::new(),
            cwd: Arc::new(Mutex::new(cwd)),
            seen_instructions: Arc::default(),
        }
    }

    pub(crate) fn cwd(&self) -> PathBuf {
        lock(&self.cwd).clone()
    }
}

/// Engine-wide services every session shares.
#[derive(Debug, Clone)]
pub(crate) struct Shared {
    pub paths: Paths,
    pub store: SessionStore,
    pub env: EnvOverrides,
    pub factory: Option<Arc<dyn ClientFactory>>,
    pub catalog: CatalogHandle,
    pub web: WebClient,
    /// The decision model's sidecar, shared by every session.
    pub sidecars: Sidecars,
    /// Work plans: one live handle per plan for the whole process.
    pub plans: PlanHub,
}

#[derive(Debug)]
pub(crate) struct SessionCore {
    pub id: SessionId,
    /// Canonical project root.
    pub root: PathBuf,
    pub shared: Shared,
    pub events: Arc<Emitter>,
    pub journal: Arc<Journal>,
    pub status: Arc<StatusTracker>,
    pub broker: Broker,
    pub reminders: Arc<ReminderBox>,
    pub jobs: Arc<JobHub>,
    /// Agent types, concurrency slots and worktree merges of subagents.
    pub agents: Orchestra,
    pub settings: RwLock<Arc<SessionSettings>>,
    pub client: RwLock<Arc<dyn ModelClient>>,
    pub tools: RwLock<Arc<ToolRegistry>>,
    pub policy: Mutex<Policy>,
    pub state: Mutex<SessionState>,
    /// Git snapshot for the environment section, taken at open and reload.
    pub git: Mutex<Option<GitInfo>>,
    pub main: AgentResources,
    pub locks: PathLocks,
    pub checkpoints: Checkpoints,
    pub verifier: Arc<dyn Verifier>,
    /// Discovered and configured checks.
    pub checks: CheckHub,
    pub mcp: McpHub,
    pub lsp: LspHub,
    /// The repository map section, stable between compactions.
    pub repo_map: RepoMapCache,
    /// The last main-agent request, rendered for the prompt inspector only
    /// when the inspector asks (rendering every round is costly).
    pub last_request: Mutex<Option<Arc<ModelRequest>>>,
    /// Experimental decisions: modes, provider and trace.
    pub decisions: DecisionHub,
    /// The attached work plan (`work_plans`).
    pub work: WorkSession,
    /// Cancelled when the session closes; background work hangs off it.
    pub cancel: CancellationToken,
}

impl SessionCore {
    pub(crate) fn settings(&self) -> Arc<SessionSettings> {
        Arc::clone(&read(&self.settings))
    }

    pub(crate) fn client(&self) -> Arc<dyn ModelClient> {
        Arc::clone(&read(&self.client))
    }

    pub(crate) fn catalog(&self) -> Option<Arc<ModelCatalog>> {
        read(&self.shared.catalog).clone()
    }

    pub(crate) fn tools(&self) -> Arc<ToolRegistry> {
        Arc::clone(&read(&self.tools))
    }

    pub(crate) fn with_state<R>(&self, change: impl FnOnce(&mut SessionState) -> R) -> R {
        change(&mut lock(&self.state))
    }

    /// The permission level.
    pub(crate) fn mode(&self) -> PermissionMode {
        lock(&self.state).mode
    }

    pub(crate) fn planning(&self) -> bool {
        lock(&self.state).planning
    }

    /// The session model, or a prompt command's model during its turn.
    pub(crate) fn main_model(&self) -> String {
        let state = lock(&self.state);
        state.turn_model.as_ref().unwrap_or(&state.model).clone()
    }

    /// The settings' additional directories plus those added with `/add-dir`.
    pub(crate) fn additional_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = self.settings().additional_dirs.clone();
        for dir in &lock(&self.state).added_dirs {
            if !dirs.contains(dir) {
                dirs.push(dir.clone());
            }
        }
        dirs
    }

    pub(crate) fn git_info(&self) -> Option<GitInfo> {
        lock(&self.git).clone()
    }

    pub(crate) fn hook_env(&self) -> HookEnv {
        HookEnv {
            settings: self.settings(),
            session_id: self.id.clone(),
            transcript_path: self.journal.path().to_path_buf(),
            cwd: self.root.clone(),
            mode: self.mode(),
            planning: self.planning(),
        }
    }
}
