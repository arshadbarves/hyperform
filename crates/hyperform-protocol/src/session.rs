//! Session-level records: summaries, turns, snapshots for (re)opening.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agents::AgentInfo;
use crate::content::Message;
use crate::ids::{AgentId, CheckpointId, MessageId, RequestId, SessionId, TurnId};
use crate::interaction::{Question, TodoItem};
use crate::jobs::JobInfo;
use crate::permission::{ApprovalRequest, PermissionMode};
use crate::usage::Usage;
use crate::verification::{CheckRecord, VerificationOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Effort {
    Low,
    Medium,
    High,
    Max,
}

impl Effort {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "low" | "minimal" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "max" | "xhigh" => Some(Self::Max),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SessionStatus {
    Idle,
    Busy,
    /// Busy, but blocked on the user (approval, question, plan).
    Waiting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum TurnOutcome {
    Completed,
    Cancelled,
    Failed {
        message: String,
    },
    /// A turn, cost, or continuation budget stopped the run.
    BudgetExhausted {
        reason: String,
    },
    /// The app closed while the turn was running.
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RewindScope {
    Code,
    Conversation,
    Both,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub title: Option<String>,
    pub project_root: String,
    pub model: String,
    /// The permission level.
    pub mode: PermissionMode,
    /// Plan mode is on: nothing that changes state runs, at any level.
    pub planning: bool,
    pub effort: Option<Effort>,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
    /// Imported from a v1 session file.
    pub legacy: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub title: Option<String>,
    pub project_root: String,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
    pub message_count: u32,
    pub cost_usd: f64,
    pub last_outcome: Option<TurnOutcome>,
    /// Originated from a v1 session file (imported, or imported on open).
    pub legacy: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnRecord {
    pub turn_id: TurnId,
    /// The user message that started the turn.
    pub message_id: MessageId,
    pub outcome: TurnOutcome,
    pub verification: VerificationOutcome,
    pub usage: Usage,
    pub cost_usd: f64,
    #[ts(type = "number")]
    pub started_at: u64,
    #[ts(type = "number")]
    pub finished_at: u64,
}

/// What started a summary compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CompactionTrigger {
    /// Context pressure: past `context.compact_at_percent`, or a context
    /// overflow from the provider.
    Auto,
    /// The user asked (`Compact`, `/compact`).
    Manual,
}

impl CompactionTrigger {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompactionMarker {
    /// First message kept verbatim after the summary.
    pub keep_from: Option<MessageId>,
    pub summary: String,
    #[ts(type = "number")]
    pub tokens_before: u64,
    #[ts(type = "number")]
    pub tokens_after: u64,
    #[ts(type = "number")]
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CheckpointInfo {
    pub checkpoint_id: CheckpointId,
    /// Restoring returns the code to its state before this user message.
    pub message_id: MessageId,
    #[ts(type = "number")]
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PendingQuestion {
    pub request_id: RequestId,
    pub agent_id: AgentId,
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PendingPlan {
    pub request_id: RequestId,
    pub agent_id: AgentId,
    pub plan: String,
}

/// Token usage by prompt layer, for `/context`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContextBreakdown {
    #[ts(type = "number")]
    pub system: u64,
    #[ts(type = "number")]
    pub tools: u64,
    #[ts(type = "number")]
    pub instructions: u64,
    #[ts(type = "number")]
    pub messages: u64,
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub limit: u64,
}

/// Everything the GUI needs to render a session it just opened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionSnapshot {
    pub info: SessionInfo,
    pub status: SessionStatus,
    /// Full main-agent transcript, including compacted history.
    pub messages: Vec<Message>,
    pub compactions: Vec<CompactionMarker>,
    pub turns: Vec<TurnRecord>,
    pub todos: Vec<TodoItem>,
    pub agents: Vec<AgentInfo>,
    pub jobs: Vec<JobInfo>,
    pub checks: Vec<CheckRecord>,
    pub checkpoints: Vec<CheckpointInfo>,
    pub pending_approvals: Vec<ApprovalRequest>,
    pub pending_questions: Vec<PendingQuestion>,
    pub pending_plans: Vec<PendingPlan>,
    pub queued: Vec<String>,
    pub usage: Usage,
    pub cost_usd: f64,
    #[ts(type = "number")]
    pub context_tokens: u64,
    #[ts(type = "number")]
    pub context_limit: u64,
    /// Task views of `decisions_task_view`, oldest first.
    #[serde(default)]
    pub task_views: Vec<crate::decisions::TaskViewInfo>,
    /// The work plan attached to the session, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub work: Option<crate::work::WorkSnapshot>,
}
