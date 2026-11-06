//! The set of ports one agent's tools can reach, plus accessors that turn a
//! missing port into a clear `Unavailable` error.

use std::fmt;
use std::sync::Arc;

use super::{
    AgentPort, CheckPort, InteractionPort, JobPort, LspPort, McpPort, RankTarget, RelevancePort,
    SideModelPort, SkillPort, WorkPort,
};
use crate::error::ToolError;

#[derive(Clone, Default)]
pub struct Ports {
    pub agents: Option<Arc<dyn AgentPort>>,
    pub interaction: Option<Arc<dyn InteractionPort>>,
    pub jobs: Option<Arc<dyn JobPort>>,
    pub skills: Option<Arc<dyn SkillPort>>,
    pub checks: Option<Arc<dyn CheckPort>>,
    pub lsp: Option<Arc<dyn LspPort>>,
    pub mcp: Option<Arc<dyn McpPort>>,
    pub side_model: Option<Arc<dyn SideModelPort>>,
    pub relevance: Option<Arc<dyn RelevancePort>>,
    /// Set only while `work_plans` is on.
    pub work: Option<Arc<dyn WorkPort>>,
}

fn require<T: ?Sized>(port: &Option<Arc<T>>, what: &str) -> Result<Arc<T>, ToolError> {
    port.clone()
        .ok_or_else(|| ToolError::unavailable(format!("{what} are not available in this session")))
}

impl Ports {
    pub(crate) fn agents(&self) -> Result<Arc<dyn AgentPort>, ToolError> {
        require(&self.agents, "subagents")
    }

    pub(crate) fn interaction(&self) -> Result<Arc<dyn InteractionPort>, ToolError> {
        require(&self.interaction, "user interactions")
    }

    pub(crate) fn jobs(&self) -> Result<Arc<dyn JobPort>, ToolError> {
        require(&self.jobs, "background jobs")
    }

    pub(crate) fn skills(&self) -> Result<Arc<dyn SkillPort>, ToolError> {
        require(&self.skills, "skills")
    }

    pub(crate) fn checks(&self) -> Result<Arc<dyn CheckPort>, ToolError> {
        require(&self.checks, "project checks")
    }

    pub(crate) fn lsp(&self) -> Result<Arc<dyn LspPort>, ToolError> {
        require(&self.lsp, "language servers")
    }

    pub(crate) fn mcp(&self) -> Result<Arc<dyn McpPort>, ToolError> {
        require(&self.mcp, "MCP servers")
    }

    pub(crate) fn work(&self) -> Result<Arc<dyn WorkPort>, ToolError> {
        require(&self.work, "work plans")
    }

    /// The ranker, when `target` runs; `None` keeps today's output.
    pub(crate) fn ranker(&self, target: RankTarget) -> Option<Arc<dyn RelevancePort>> {
        self.relevance.clone().filter(|port| port.ranks(target))
    }
}

impl fmt::Debug for Ports {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ports")
            .field("agents", &self.agents.is_some())
            .field("interaction", &self.interaction.is_some())
            .field("jobs", &self.jobs.is_some())
            .field("skills", &self.skills.is_some())
            .field("checks", &self.checks.is_some())
            .field("lsp", &self.lsp.is_some())
            .field("mcp", &self.mcp.is_some())
            .field("side_model", &self.side_model.is_some())
            .field("relevance", &self.relevance.is_some())
            .field("work", &self.work.is_some())
            .finish()
    }
}
