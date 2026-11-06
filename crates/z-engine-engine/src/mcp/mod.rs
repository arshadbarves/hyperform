//! MCP servers of one session: specs from the effective settings, one
//! manager per server connected in the background, the merged tool
//! catalog registered as `mcp__` tools (deferred behind `LoadMcpTools`
//! when there are too many), and results formatted for the model.

mod catalog;
mod deferred;
mod format;
mod hub;
mod lifecycle;
mod specs;

pub(crate) use deferred::McpRunState;
pub(crate) use format::{call_parts, resource_listing, resource_parts};
pub(crate) use hub::McpHub;
pub(crate) use lifecycle::sync_servers;
pub(crate) use specs::server_spec;
