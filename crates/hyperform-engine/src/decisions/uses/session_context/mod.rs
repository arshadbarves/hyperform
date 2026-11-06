//! First-request context (`decisions_session_context`): which project
//! files rank early in the repository map and which deferred MCP tools are
//! loaded up front for a chat's first request.

mod candidates;
mod first_request;
mod preload;
#[cfg(test)]
mod tests;

pub(crate) use first_request::SESSION_CONTEXT;
pub(crate) use preload::{ToolPreload, preload_tools};
