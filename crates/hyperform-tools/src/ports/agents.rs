//! Subagents: the catalog shown in the `Agent` description, spawning
//! (foreground, background, resume), and applying worktree changes.

use async_trait::async_trait;
use hyperform_protocol::{AgentId, Isolation, JobId};

use crate::context::ToolCtx;

/// One agent type the model may pick as `subagent_type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    /// Tool list as written in the definition (`*` for all).
    pub tools: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnRequest {
    pub agent_type: String,
    /// Short label for the agent tree.
    pub description: String,
    /// The task, or the follow-up message when resuming.
    pub prompt: String,
    pub background: bool,
    /// Continue this finished agent from its transcript.
    pub resume: Option<AgentId>,
    /// `None` uses the definition's isolation.
    pub isolation: Option<Isolation>,
    /// The work-plan task the helper is dispatched for (`work_plans`).
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnOutcome {
    pub agent_id: AgentId,
    /// Set for background agents.
    pub job_id: Option<JobId>,
    /// The final report, or a note that the agent started in the background.
    pub text: String,
    /// Usage and changed-files summary.
    pub footer: String,
}

#[async_trait]
pub trait AgentPort: Send + Sync {
    fn catalog(&self) -> Vec<AgentCard>;

    async fn spawn(&self, ctx: &ToolCtx, req: SpawnRequest) -> Result<SpawnOutcome, String>;

    /// Merges a worktree agent's changes into the tree; returns a summary.
    async fn apply_changes(&self, ctx: &ToolCtx, agent_id: &AgentId) -> Result<String, String>;
}
