//! Model Context Protocol client: stdio and streamable HTTP transports, the
//! per-server [`McpClient`], the multi-server [`McpManager`], model-facing
//! tool names, and content conversion.

mod client;
mod content;
mod entry;
mod http;
mod manager;
mod naming;
mod notifications;
mod parse;
mod spec;
mod sse;
mod status;
mod transport;
mod types;

pub use client::{McpClient, PROTOCOL_VERSION};
pub use content::content_to_parts;
pub use manager::McpManager;
pub use naming::{MAX_TOOL_NAME_LEN, split_tool_name, tool_name};
pub use notifications::ListChangedCallback;
pub use spec::{McpServerSpec, McpTransport};
pub use status::{McpChange, McpChangeCallback, McpChangeKind, McpServerState, McpServerStatus};
pub use types::{
    CallToolResult, McpContent, McpListKind, McpPromptInfo, McpResourceInfo, McpToolInfo,
    PromptArgument, PromptMessage, ResourceContents, ServerCapabilities, ServerInfo,
    ToolAnnotations,
};
