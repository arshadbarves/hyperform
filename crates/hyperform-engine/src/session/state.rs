//! The authoritative in-memory state of one session, rebuilt from the log
//! on open and after conversation rewinds. Only short critical sections
//! touch it; nothing here performs I/O.

use std::collections::BTreeMap;
use std::path::PathBuf;

use hyperform_protocol::decisions::TaskViewInfo;
use hyperform_protocol::{
    AgentId, AgentInfo, CheckRecord, CheckpointInfo, CompactionMarker, Effort, Message,
    PermissionMode, PlanId, TodoItem, TurnOutcome, TurnRecord, Usage,
};
use hyperform_store::ReplayState;

use crate::run::ContextMeter;
use crate::verify::Mutation;

#[derive(Debug, Clone)]
pub(crate) struct SessionState {
    pub created_at: u64,
    pub updated_at: u64,
    pub legacy: bool,
    /// Every visible main-agent message, compacted history included.
    pub transcript: Vec<Message>,
    /// What the main agent's model sees.
    pub working: Vec<Message>,
    pub compactions: Vec<CompactionMarker>,
    pub turns: Vec<TurnRecord>,
    pub todos: BTreeMap<AgentId, Vec<TodoItem>>,
    pub agents: Vec<AgentInfo>,
    pub checks: Vec<CheckRecord>,
    /// Checkpoints with their shadow commit.
    pub checkpoints: Vec<(CheckpointInfo, String)>,
    /// The permission level.
    pub mode: PermissionMode,
    pub planning: bool,
    pub model: String,
    pub effort: Option<Effort>,
    pub title: Option<String>,
    /// Session totals, side requests included.
    pub usage: Usage,
    pub cost_usd: f64,
    /// Cumulative usage per agent.
    pub agent_usage: BTreeMap<AgentId, Usage>,
    /// Steering messages waiting for the next round boundary.
    pub queue: Vec<String>,
    pub context_tokens: u64,
    pub context_limit: u64,
    /// Context occupancy of the main agent's working set.
    pub meter: ContextMeter,
    /// The previous turn was cancelled or interrupted.
    pub interrupted: bool,
    /// A `!cmd` ran since the last turn started.
    pub external_mutation: bool,
    /// What the current (or last) main-agent turn changed.
    pub mutation: Mutation,
    /// Directories added with `/add-dir` for this session.
    pub added_dirs: Vec<PathBuf>,
    /// A prompt command's `model`, for its turn only.
    pub turn_model: Option<String>,
    /// While a task view applies (`decisions_task_view`): the working set
    /// without it, which new messages extend too.
    pub full_working: Option<Vec<Message>>,
    /// Latest state of each task view, oldest first.
    pub task_views: Vec<TaskViewInfo>,
    /// The work plan the log names, until the open reattaches it.
    pub attached_plan: Option<PlanId>,
}

impl SessionState {
    /// `model`/`effort` come from the log when it recorded them, else from
    /// the current settings.
    pub(crate) fn from_replay(
        replay: ReplayState,
        legacy: bool,
        updated_at: u64,
        default_model: &str,
        default_effort: Option<Effort>,
    ) -> Self {
        let created_at = replay.info.as_ref().map_or(updated_at, |info| info.2);
        let interrupted = replay.turns.last().is_some_and(|turn| {
            matches!(
                turn.outcome,
                TurnOutcome::Cancelled | TurnOutcome::Interrupted
            )
        });
        let mut agent_usage = BTreeMap::new();
        agent_usage.insert(AgentId::main(), replay.usage);
        Self {
            created_at,
            updated_at: updated_at.max(created_at),
            legacy,
            transcript: replay.transcript,
            working: replay.working,
            compactions: replay.compactions,
            turns: replay.turns,
            todos: replay.todos,
            agents: replay.agents,
            checks: replay.checks,
            checkpoints: replay.checkpoints,
            mode: replay.mode,
            planning: replay.planning,
            model: replay
                .model
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| default_model.to_string()),
            effort: replay.effort.or(default_effort),
            title: replay.title,
            usage: replay.usage,
            cost_usd: replay.cost_usd,
            agent_usage,
            queue: Vec::new(),
            context_tokens: 0,
            context_limit: 0,
            meter: ContextMeter::default(),
            interrupted,
            external_mutation: false,
            mutation: Mutation::default(),
            added_dirs: Vec::new(),
            turn_model: None,
            full_working: replay.full_working,
            task_views: replay.task_views,
            attached_plan: replay.attached_plan,
        }
    }

    pub(crate) fn todos_of(&self, agent: &AgentId) -> &[TodoItem] {
        self.todos.get(agent).map_or(&[], Vec::as_slice)
    }
}
