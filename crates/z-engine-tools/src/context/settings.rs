//! Per-agent settings carried by [`super::ToolCtx`]: limits, shell,
//! environment policy and sandbox, web options, and the artifact spill hook.

use std::path::PathBuf;
use std::sync::Arc;

use z_engine_host::{
    DEFAULT_MAX_IMAGE_BYTES, EnvPolicy, HostError, SandboxProfile, SearchBackend, ShellSpec,
    resolve_shell, sandbox_shell,
};

/// Stores a full tool output as an artifact: `(hint, content)` -> path.
/// `hint` names the producer (`"bash"`, `"grep"`, ...). `None` means the
/// output could not be stored; the truncated text then says so.
pub type SpillFn = Arc<dyn Fn(&str, &str) -> Option<PathBuf> + Send + Sync>;

/// Size and time budgets for tool results and commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolLimits {
    /// Characters of text a result may carry before head+tail truncation.
    pub max_result_chars: usize,
    /// Lines `Read` returns without an explicit `limit`.
    pub read_default_lines: usize,
    /// Characters per line `Read` shows before cutting the line.
    pub read_max_line_chars: usize,
    pub bash_default_timeout_ms: u64,
    pub bash_max_timeout_ms: u64,
    /// Files `Glob` lists before reporting truncation.
    pub glob_limit: usize,
    /// Largest image `Read` returns as an image part.
    pub max_image_bytes: u64,
}

impl Default for ToolLimits {
    fn default() -> Self {
        Self {
            max_result_chars: 30_000,
            read_default_lines: 2_000,
            read_max_line_chars: 2_000,
            bash_default_timeout_ms: 120_000,
            bash_max_timeout_ms: 600_000,
            glob_limit: 100,
            max_image_bytes: DEFAULT_MAX_IMAGE_BYTES,
        }
    }
}

/// The shell agent commands run in, the environment they see, and the
/// sandbox that confines them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellConfig {
    pub spec: ShellSpec,
    pub env: EnvPolicy,
    /// `Some` runs every command in the OS sandbox; a sandbox that cannot
    /// start fails the command rather than running it unconfined.
    pub sandbox: Option<SandboxProfile>,
}

impl ShellConfig {
    /// The detected default shell with the default environment policy and
    /// no sandbox.
    pub fn detect() -> Self {
        Self {
            spec: resolve_shell(None),
            env: EnvPolicy::default(),
            sandbox: None,
        }
    }

    #[must_use]
    pub fn with_sandbox(mut self, sandbox: Option<SandboxProfile>) -> Self {
        self.sandbox = sandbox;
        self
    }

    /// The shell to run commands with: [`Self::spec`], wrapped in the
    /// sandbox when one is configured. Foreground and background commands
    /// both start from this.
    pub fn effective_spec(&self) -> Result<ShellSpec, HostError> {
        match &self.sandbox {
            Some(profile) => sandbox_shell(&self.spec, profile),
            None => Ok(self.spec.clone()),
        }
    }
}

/// How `WebFetch` and `WebSearch` reach the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebOptions {
    /// Allow loopback, private, and link-local fetch targets.
    pub allow_private_network: bool,
    /// Answer `WebFetch` prompts with the side model when one is available;
    /// otherwise the page itself is returned.
    pub fetch_extract: bool,
    pub search: SearchBackend,
}

impl Default for WebOptions {
    fn default() -> Self {
        Self {
            allow_private_network: false,
            fetch_extract: true,
            search: SearchBackend::None,
        }
    }
}
