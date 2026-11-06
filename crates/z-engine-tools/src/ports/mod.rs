//! Engine services behind traits, so tools never depend on the engine. A
//! missing port makes the tools that need it report `Unavailable`.

mod agents;
mod bundle;
mod checks;
mod interaction;
mod jobs;
mod lsp;
mod mcp;
mod relevance;
mod side_model;
mod skills;
mod work;

pub use agents::{AgentCard, AgentPort, SpawnOutcome, SpawnRequest};
pub use bundle::Ports;
pub use checks::{CheckPort, CheckSummary};
pub use interaction::InteractionPort;
pub use jobs::{JobOutput, JobPort};
pub use lsp::{LspPort, LspRequest};
pub use mcp::McpPort;
pub use relevance::{RankRequest, RankTarget, RelevancePort};
pub use side_model::SideModelPort;
pub use skills::{SkillContent, SkillPort};
pub use work::{CriterionInput, NewTask, PlanNote, PlanOutline, TaskChange, WorkPort, WorkReply};
