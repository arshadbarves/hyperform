//! MCP (stdio and streamable HTTP) and multi-server LSP clients over one
//! JSON-RPC core.
//!
//! Documented exception to "only `z-engine-host` touches the OS": this crate
//! spawns its long-lived server processes itself (piped stdio, killed on
//! drop, own process group on unix) and speaks HTTP to MCP servers; the
//! environment policy and tree teardown come from `z-engine-host`.

pub mod error;
pub mod jsonrpc;
pub mod lsp;
pub mod mcp;

mod process;
mod sync;

pub use error::IntegrationError;
pub use jsonrpc::{CancelStyle, Framing, NotificationHandler, RequestId, RpcClient, RpcOptions};
pub use lsp::{
    CallDirection, CallEdge, CallItem, Diagnostic, FileDiagnostics, FileEdits, Hover, Location,
    LspClient, LspManager, LspPosition, LspServerSpec, LspServerState, LspServerStatus, Severity,
    SymbolKind, SymbolNode, TextEditPlan, WorkspaceEditPlan, WorkspaceSymbol, merge_specs, presets,
};
pub use mcp::{
    CallToolResult, McpChange, McpChangeCallback, McpChangeKind, McpClient, McpContent,
    McpListKind, McpManager, McpPromptInfo, McpResourceInfo, McpServerSpec, McpServerState,
    McpServerStatus, McpToolInfo, McpTransport, PromptArgument, PromptMessage, ResourceContents,
    ToolAnnotations, content_to_parts, split_tool_name, tool_name,
};
