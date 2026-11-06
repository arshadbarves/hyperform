//! Engine -> GUI notifications. Every event is wrapped in an
//! [`EventEnvelope`] carrying the session id and a per-session sequence.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::agents::AgentInfo;
use crate::content::Message;
use crate::decisions::{TurnTone, UrgencyInfo};
use crate::ids::{AgentId, CallId, MessageId, RequestId, SessionId, TurnId};
use crate::interaction::{Question, TodoItem};
use crate::jobs::JobInfo;
use crate::permission::{ApprovalRequest, PermissionMode};
use crate::session::{
    CheckpointInfo, CompactionMarker, CompactionTrigger, ContextBreakdown, Effort, SessionSnapshot,
    SessionStatus, TurnRecord,
};
use crate::usage::Usage;
use crate::verification::{CheckRecord, VerificationOutcome};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EventEnvelope {
    pub session_id: SessionId,
    #[ts(type = "number")]
    pub seq: u64,
    pub event: Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolStatus {
    Ok,
    Error,
    Denied,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoticeLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Event {
    /// Full state; sent on open and after rewinds or reloads.
    Snapshot {
        snapshot: Box<SessionSnapshot>,
    },
    StatusChanged {
        status: SessionStatus,
    },
    /// A persisted user message (a new turn or an injected steering note).
    UserMessage {
        message: Message,
        turn_id: Option<TurnId>,
        steering: bool,
    },
    TurnStarted {
        turn_id: TurnId,
        message_id: MessageId,
    },
    TurnFinished {
        turn: TurnRecord,
    },
    AssistantStarted {
        agent_id: AgentId,
        message_id: MessageId,
    },
    TextDelta {
        agent_id: AgentId,
        message_id: MessageId,
        text: String,
    },
    ThinkingDelta {
        agent_id: AgentId,
        message_id: MessageId,
        text: String,
    },
    /// The authoritative assembled assistant message.
    AssistantFinished {
        agent_id: AgentId,
        message: Message,
    },
    ToolStarted {
        agent_id: AgentId,
        call_id: CallId,
        tool: String,
        title: String,
        input: Value,
    },
    ToolProgress {
        agent_id: AgentId,
        call_id: CallId,
        text: String,
    },
    ToolFinished {
        agent_id: AgentId,
        call_id: CallId,
        status: ToolStatus,
        summary: String,
        /// Truncated result text for the tool card.
        output: String,
        #[ts(type = "number")]
        duration_ms: u64,
    },
    ApprovalRequested {
        request: ApprovalRequest,
    },
    ApprovalResolved {
        request_id: RequestId,
        allowed: bool,
    },
    QuestionAsked {
        request_id: RequestId,
        agent_id: AgentId,
        questions: Vec<Question>,
    },
    QuestionResolved {
        request_id: RequestId,
        answered: bool,
    },
    PlanProposed {
        request_id: RequestId,
        agent_id: AgentId,
        plan: String,
    },
    PlanResolved {
        request_id: RequestId,
        approved: bool,
    },
    TodosUpdated {
        agent_id: AgentId,
        todos: Vec<TodoItem>,
    },
    AgentStarted {
        info: AgentInfo,
    },
    AgentUpdated {
        info: AgentInfo,
    },
    JobUpdated {
        job: JobInfo,
    },
    UsageUpdated {
        agent_id: AgentId,
        /// Cumulative usage of this agent.
        usage: Usage,
        session_usage: Usage,
        cost_usd: f64,
        #[ts(type = "number")]
        context_tokens: u64,
        #[ts(type = "number")]
        context_limit: u64,
    },
    /// The permission level changed; planning is unaffected.
    ModeChanged {
        mode: PermissionMode,
    },
    /// Plan mode was turned on or off; the permission level is unaffected.
    PlanningChanged {
        planning: bool,
    },
    ModelChanged {
        model: String,
    },
    EffortChanged {
        effort: Option<Effort>,
    },
    /// The main agent's older history is being summarized; `Compacted`
    /// follows on success, a notice on failure.
    CompactionStarted {
        trigger: CompactionTrigger,
    },
    Compacted {
        marker: CompactionMarker,
    },
    CheckpointCreated {
        checkpoint: CheckpointInfo,
    },
    CheckRecorded {
        record: CheckRecord,
    },
    VerificationChanged {
        outcome: VerificationOutcome,
    },
    HookRan {
        hook_event: String,
        command: String,
        blocked: bool,
        message: Option<String>,
    },
    /// Markdown output of an informational slash command.
    CommandOutput {
        name: String,
        markdown: String,
    },
    ContextReport {
        breakdown: ContextBreakdown,
    },
    Notice {
        level: NoticeLevel,
        text: String,
    },
    /// The provider asked us to back off; a retry is scheduled.
    Retrying {
        attempt: u32,
        #[ts(type = "number")]
        delay_ms: u64,
        reason: String,
    },
    TitleChanged {
        title: String,
    },
    /// Steering messages waiting for the next round boundary.
    QueueChanged {
        queued: Vec<String>,
    },
    /// The project sets things only a trusted project may (hooks, MCP
    /// servers, checks, looser permissions, shell, provider, web or
    /// language servers); they stay off until the user trusts it
    /// (`defines` names them, e.g. `hooks`).
    TrustRequired {
        project_root: String,
        defines: Vec<String>,
    },
    /// `decisions_pet_mood`: how the turn that just ended went.
    TurnToneJudged {
        turn_id: TurnId,
        tone: TurnTone,
    },
    /// `decisions_inbox_priority`: how urgent an inbox item is.
    UrgencyScored {
        urgency: UrgencyInfo,
    },
    /// `decisions_routing`: the effort or model a new task (or a subagent
    /// that inherits its model) was routed to, and why.
    RouteChosen {
        route: crate::decisions::RouteInfo,
    },
    /// A decision use offers an action as a card; it waits for the user.
    Suggested {
        suggestion: crate::decisions::Suggestion,
    },
    /// The user acted on or dismissed a suggestion.
    SuggestionResolved {
        suggestion_id: String,
        accepted: bool,
    },
    /// `decisions_completion_check`: the agent's final message claims
    /// success that no check backs; the turn's receipt says so.
    CompletionClaimUnchecked {
        claim: crate::decisions::UncheckedClaim,
    },
    /// `decisions_task_view`: earlier exchanges were set aside for a new
    /// task, or (`restored`) brought back.
    TaskViewApplied {
        view: crate::decisions::TaskViewInfo,
    },
    /// A revision of the attached work plan (upserts only).
    WorkUpdated {
        update: Box<crate::work::WorkUpdate>,
    },
    /// A task packet was put in front of an agent (`reason`: "turn",
    /// "taskChanged", "compaction", "resume" or "worker").
    TaskPacketSent {
        agent_id: AgentId,
        task_id: crate::ids::TaskId,
        #[ts(type = "number")]
        tokens: u64,
        reason: String,
    },
    Error {
        message: String,
    },
}

impl Event {
    pub fn notice(level: NoticeLevel, text: impl Into<String>) -> Self {
        Self::Notice {
            level,
            text: text.into(),
        }
    }
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
