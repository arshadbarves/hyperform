//! What parameterizes one agent run, so the same loop serves the main
//! agent and subagents: identity, prompt, tools, model, mode, budget,
//! root directory, depth, working resources and cancellation.

use std::path::PathBuf;
use std::sync::Arc;

use hyperform_context::GitInfo;
use hyperform_protocol::{AgentId, PermissionMode, TurnOutcome, Usage, VerificationOutcome};
use tokio_util::sync::CancellationToken;

use super::tally::ChildTally;
use super::web::WebTaint;
use crate::decisions::routed_model;
use crate::mcp::McpRunState;
use crate::orchestration::AgentTracker;
use crate::session::{AgentResources, SessionCore};

/// Which registry tools an agent is offered.
#[derive(Debug, Clone, Default)]
pub(crate) struct ToolFilter {
    /// `None` offers every tool.
    pub allow: Option<Vec<String>>,
    pub deny: Vec<String>,
}

impl ToolFilter {
    pub(crate) fn permits(&self, name: &str) -> bool {
        let allowed = self
            .allow
            .as_ref()
            .is_none_or(|allow| allow.iter().any(|tool| tool == name));
        allowed && !self.deny.iter().any(|tool| tool == name)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ModelChoice {
    /// Follows the session's main model (`SetModel` applies next round).
    Session,
    Fixed(String),
}

/// An isolated agent's view: its root is the worktree, the main project
/// stays readable.
#[derive(Debug, Clone)]
pub(crate) struct WorktreeScope {
    /// The session's project root.
    pub project: PathBuf,
    /// Git state of the worktree for the environment section.
    pub git: Option<GitInfo>,
    /// Environment note naming the worktree and its branch.
    pub note: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentSpec {
    pub agent_id: AgentId,
    pub base_prompt: String,
    pub tools: ToolFilter,
    pub model: ModelChoice,
    /// `None` follows the session's permission level; a session in bypass
    /// mode overrides it.
    pub mode: Option<PermissionMode>,
    /// Plans whatever the session does: its definition says
    /// `permissionMode: plan`, or an ancestor's does.
    pub planning: bool,
    pub max_turns: u32,
    /// Project root, or the agent's worktree.
    pub root: PathBuf,
    /// 0 for the main agent, parent depth + 1 for subagents.
    pub depth: u32,
    pub worktree: Option<WorktreeScope>,
}

impl AgentSpec {
    pub(crate) fn main(root: PathBuf, max_turns: u32) -> Self {
        Self {
            agent_id: AgentId::main(),
            base_prompt: hyperform_prompts::system::MAIN.to_string(),
            tools: ToolFilter::default(),
            model: ModelChoice::Session,
            mode: None,
            planning: false,
            max_turns: max_turns.max(1),
            root,
            depth: 0,
            worktree: None,
        }
    }

    pub(crate) fn is_main(&self) -> bool {
        self.agent_id.is_main()
    }
}

/// Everything a run and its tool batches share.
#[derive(Debug, Clone)]
pub(crate) struct RunContext {
    pub core: Arc<SessionCore>,
    pub spec: AgentSpec,
    pub resources: AgentResources,
    pub cancel: CancellationToken,
    /// A subagent's live `AgentInfo`; `None` for the main agent.
    pub tracker: Option<Arc<AgentTracker>>,
    /// What foreground children and applied worktrees add to this run.
    pub children: Arc<ChildTally>,
    /// Deferred MCP tools this run loaded and the listing it was shown.
    pub mcp: Arc<McpRunState>,
    /// Whether this run has read web content.
    pub web: Arc<WebTaint>,
}

impl RunContext {
    pub(crate) fn new(
        core: Arc<SessionCore>,
        spec: AgentSpec,
        resources: AgentResources,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            core,
            spec,
            resources,
            cancel,
            tracker: None,
            children: Arc::default(),
            mcp: Arc::default(),
            web: Arc::default(),
        }
    }

    /// The agent's own mode, except that a session in bypass mode bypasses
    /// for every agent, whatever mode its definition sets (as in Claude
    /// Code).
    pub(crate) fn mode(&self) -> PermissionMode {
        let session = self.core.mode();
        match self.spec.mode {
            Some(mode) if session != PermissionMode::Bypass => mode,
            _ => session,
        }
    }

    /// Every agent plans while the session does; no agent can stop it.
    pub(crate) fn planning(&self) -> bool {
        self.spec.planning || self.core.planning()
    }

    /// The fixed model, else the session's, or the task's routed model
    /// (`decisions_routing`) while the session still runs the one it was
    /// routed from.
    pub(crate) fn model(&self) -> String {
        match &self.spec.model {
            ModelChoice::Session => {
                let model = self.core.main_model();
                routed_model(&self.core, &model).unwrap_or(model)
            }
            ModelChoice::Fixed(model) => model.clone(),
        }
    }
}

/// How a run ended and what it cost.
#[derive(Debug, Clone)]
pub(crate) struct RunOutcome {
    pub outcome: TurnOutcome,
    /// This run's usage plus its foreground children's.
    pub usage: Usage,
    pub cost_usd: f64,
    /// Files changed during the run.
    pub mutated: bool,
    /// The badge from the stop boundary, when the run reached it.
    pub verification: Option<VerificationOutcome>,
    /// Text of the last assistant message.
    pub final_text: String,
    pub tool_calls: u32,
    /// Files the run's own tools wrote.
    pub written: Vec<PathBuf>,
}
