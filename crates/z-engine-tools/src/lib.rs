//! Tool trait, capability ports, registry, and one file per built-in tool.
//!
//! Tools derive a policy [`Action`](z_engine_policy::Action) from their
//! input, preview their effect, and run against a [`ToolCtx`]; they reach
//! the OS only through `z-engine-host` and the engine only through
//! [`ports`].

mod context;
mod edit;
mod error;
mod input;
mod mcp_tool;
mod notebook;
mod output;
mod reading;
mod registry;
mod schema;
mod text;
mod tool;

pub mod builtin;
pub mod names;
pub mod ports;

pub use context::{ShellConfig, SpillFn, ToolCtx, ToolCtxBuilder, ToolLimits, WebOptions};
pub use error::ToolError;
pub use mcp_tool::{MAX_TOOL_NAME_CHARS, mcp_tool, mcp_tool_name};
pub use output::{Effects, ToolOutput};
pub use ports::{
    AgentCard, AgentPort, CheckPort, CheckSummary, InteractionPort, JobOutput, JobPort, LspPort,
    LspRequest, McpPort, Ports, SideModelPort, SkillContent, SkillPort, SpawnOutcome, SpawnRequest,
};
pub use ports::{CriterionInput, NewTask, PlanNote, PlanOutline, TaskChange, WorkPort, WorkReply};
pub use ports::{RankRequest, RankTarget, RelevancePort};
pub use registry::ToolRegistry;
pub use tool::Tool;
