//! What the manager reports about its servers, and the change events it
//! emits for the GUI and the engine's tool registry.

use std::sync::Arc;

use super::types::McpListKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerState {
    Connecting,
    Ready,
    /// Start, handshake or connection failure; the reason is user-facing.
    Failed(String),
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerStatus {
    pub name: String,
    pub state: McpServerState,
    pub tool_count: usize,
    pub resource_count: usize,
    pub prompt_count: usize,
    /// The failure reason while `Failed`.
    pub error: Option<String>,
    /// Last stderr lines of a stdio server, kept across reconnects.
    pub stderr_tail: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpChangeKind {
    /// The server's state changed (see [`McpServerStatus`]).
    Status,
    Tools,
    Resources,
    Prompts,
}

impl From<McpListKind> for McpChangeKind {
    fn from(kind: McpListKind) -> Self {
        match kind {
            McpListKind::Tools => Self::Tools,
            McpListKind::Resources => Self::Resources,
            McpListKind::Prompts => Self::Prompts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpChange {
    pub server: String,
    pub kind: McpChangeKind,
}

/// Receives manager changes; may run on a connection's reading task, so
/// keep it quick and non-blocking.
pub type McpChangeCallback = Arc<dyn Fn(McpChange) + Send + Sync>;
