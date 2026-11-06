//! Slash commands: the catalog of every source (engine built-ins, custom
//! and built-in prompt commands, MCP prompts, GUI commands), expansion of
//! prompt commands into a turn, `@agent-` mentions, suggestions for
//! unknown names, and the `/mcp` and `/doctor` reports. `session/slash.rs`
//! dispatches.

mod args;
mod catalog;
mod doctor;
mod expand;
mod include;
mod inline;
mod mcp_prompt;
mod mcp_report;
mod mentions;
mod suggest;

pub(crate) use catalog::{CommandEntry, Target, command_catalog, listed_commands};
pub(crate) use doctor::doctor;
pub(crate) use expand::{CommandCall, Expansion, expand};
pub(crate) use mcp_report::mcp_report;
pub(crate) use mentions::agent_mentions;
pub(crate) use suggest::suggest;
