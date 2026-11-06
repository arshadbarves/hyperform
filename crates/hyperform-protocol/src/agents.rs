//! Agent runs as seen by the GUI's agent tree.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AgentId, CallId, TaskId};
use crate::usage::Usage;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Isolation {
    /// Works in the session's project tree.
    #[default]
    Shared,
    /// Works in its own git worktree; changes merge back on request.
    Worktree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AgentStatus {
    Running,
    /// Blocked on an approval, question, or plan review.
    Waiting,
    Completed,
    Failed,
    Cancelled,
}

impl AgentStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorktreeState {
    /// Changes are committed on the agent branch, awaiting a decision.
    Pending,
    Applied,
    Discarded,
    /// Applying produced conflicts; the tree was left unchanged.
    Conflicted,
    /// The agent changed nothing.
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
    /// Commit the worktree was created from.
    pub base: String,
    pub state: WorktreeState,
    pub files_changed: u32,
    pub diffstat: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentInfo {
    pub agent_id: AgentId,
    pub parent_id: Option<AgentId>,
    /// The `Agent` tool call that started this run (None for the main agent).
    pub call_id: Option<CallId>,
    /// Definition name, e.g. "explore" or a custom agent.
    pub agent_type: String,
    /// Short task label supplied by the caller.
    pub description: String,
    pub model: String,
    pub background: bool,
    pub isolation: Isolation,
    pub worktree: Option<WorktreeInfo>,
    pub status: AgentStatus,
    pub depth: u32,
    #[ts(type = "number")]
    pub started_at: u64,
    #[ts(type = "number | null")]
    pub finished_at: Option<u64>,
    pub usage: Usage,
    pub cost_usd: f64,
    pub tool_calls: u32,
    pub result_preview: Option<String>,
    pub error: Option<String>,
    /// The agent's run read web content, so its transcript and report are
    /// untrusted; a resumed run starts from that.
    #[serde(default)]
    pub read_web: bool,
    /// The work-plan task this helper was dispatched for (`Agent { task_id }`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub task_id: Option<TaskId>,
}
