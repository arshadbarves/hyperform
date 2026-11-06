//! `AgentPort`: the registry's agent types, spawning children of the
//! calling run, and merging a worktree agent's changes into the project
//! (a change of the caller's tree for verification).

use async_trait::async_trait;
use z_engine_protocol::AgentId;
use z_engine_tools::{AgentCard, AgentPort, SpawnOutcome, SpawnRequest, ToolCtx};

use crate::orchestration;
use crate::run::RunContext;

#[derive(Debug)]
pub(crate) struct Agents {
    /// The calling run.
    ctx: RunContext,
}

impl Agents {
    pub(crate) fn new(ctx: RunContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl AgentPort for Agents {
    fn catalog(&self) -> Vec<AgentCard> {
        self.ctx.core.agents.registry().cards()
    }

    async fn spawn(&self, ctx: &ToolCtx, req: SpawnRequest) -> Result<SpawnOutcome, String> {
        orchestration::spawn(&self.ctx, &ctx.call_id, req).await
    }

    async fn apply_changes(&self, _ctx: &ToolCtx, agent_id: &AgentId) -> Result<String, String> {
        let applied = orchestration::apply(&self.ctx.core, agent_id).await?;
        if applied.merged {
            self.ctx.children.mark_mutated();
            Ok(applied.summary)
        } else {
            Err(applied.summary)
        }
    }
}
