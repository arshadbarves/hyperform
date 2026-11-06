//! Convenient construction of [`ToolCtx`] with sensible defaults: a fresh
//! session, the main agent, a new call id, default mode and limits, the
//! detected shell, no ports, no progress sink, and no spill hook.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;
use z_engine_host::{FileTracker, OutputSink, PathLocks, WebClient};
use z_engine_protocol::{AgentId, CallId, PermissionMode, SessionId};

use super::ctx::ToolCtx;
use super::settings::{ShellConfig, SpillFn, ToolLimits, WebOptions};
use crate::ports::Ports;

#[derive(Debug)]
pub struct ToolCtxBuilder {
    ctx: ToolCtx,
}

impl ToolCtx {
    /// Starts a context for `root`; the Bash working directory starts there.
    pub fn builder(root: impl Into<PathBuf>, web: WebClient) -> ToolCtxBuilder {
        let root = root.into();
        ToolCtxBuilder {
            ctx: ToolCtx {
                session_id: SessionId::new(),
                agent_id: AgentId::main(),
                call_id: CallId::new(),
                cwd: Arc::new(Mutex::new(root.clone())),
                root,
                additional_dirs: Vec::new(),
                mode: PermissionMode::default(),
                planning: false,
                cancel: CancellationToken::new(),
                files: Arc::new(FileTracker::new()),
                locks: Arc::new(PathLocks::new()),
                shell: Arc::new(ShellConfig::detect()),
                web,
                web_options: WebOptions::default(),
                progress: None,
                spill: None,
                limits: ToolLimits::default(),
                vision: false,
                ports: Arc::new(Ports::default()),
            },
        }
    }

    /// A default context for tests, with vision enabled.
    ///
    /// # Panics
    /// When the HTTP client cannot be constructed; this constructor exists
    /// for tests, where that is a fixture failure.
    pub fn for_tests(root: impl Into<PathBuf>) -> Self {
        let web = match WebClient::new() {
            Ok(web) => web,
            Err(e) => panic!("test fixture: cannot build the HTTP client: {e}"),
        };
        Self::builder(root, web).vision(true).build()
    }

    /// The same context for another call (fresh call id, shared state).
    pub fn for_call(&self, call_id: CallId) -> Self {
        Self {
            call_id,
            ..self.clone()
        }
    }
}

impl ToolCtxBuilder {
    pub fn session_id(mut self, id: SessionId) -> Self {
        self.ctx.session_id = id;
        self
    }

    pub fn agent_id(mut self, id: AgentId) -> Self {
        self.ctx.agent_id = id;
        self
    }

    pub fn call_id(mut self, id: CallId) -> Self {
        self.ctx.call_id = id;
        self
    }

    pub fn additional_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.ctx.additional_dirs = dirs;
        self
    }

    pub fn mode(mut self, mode: PermissionMode) -> Self {
        self.ctx.mode = mode;
        self
    }

    pub fn planning(mut self, planning: bool) -> Self {
        self.ctx.planning = planning;
        self
    }

    /// Shares an agent's persistent working directory across calls.
    pub fn cwd(mut self, cwd: Arc<Mutex<PathBuf>>) -> Self {
        self.ctx.cwd = cwd;
        self
    }

    pub fn cancel(mut self, token: CancellationToken) -> Self {
        self.ctx.cancel = token;
        self
    }

    pub fn files(mut self, files: Arc<FileTracker>) -> Self {
        self.ctx.files = files;
        self
    }

    pub fn locks(mut self, locks: Arc<PathLocks>) -> Self {
        self.ctx.locks = locks;
        self
    }

    pub fn shell(mut self, shell: Arc<ShellConfig>) -> Self {
        self.ctx.shell = shell;
        self
    }

    pub fn web_options(mut self, options: WebOptions) -> Self {
        self.ctx.web_options = options;
        self
    }

    pub fn progress(mut self, sink: OutputSink) -> Self {
        self.ctx.progress = Some(sink);
        self
    }

    pub fn spill(mut self, spill: SpillFn) -> Self {
        self.ctx.spill = Some(spill);
        self
    }

    pub fn limits(mut self, limits: ToolLimits) -> Self {
        self.ctx.limits = limits;
        self
    }

    pub fn vision(mut self, vision: bool) -> Self {
        self.ctx.vision = vision;
        self
    }

    pub fn ports(mut self, ports: Ports) -> Self {
        self.ctx.ports = Arc::new(ports);
        self
    }

    pub fn build(self) -> ToolCtx {
        self.ctx
    }
}
