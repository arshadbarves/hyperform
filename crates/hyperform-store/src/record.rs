//! One line of a v2 log. Records are tagged by `kind` (camelCase) and are
//! decoded line by line, so an unknown kind costs one skipped line.
//!
//! Writers append a user `Message` before the `TurnStarted` that references
//! it, so a conversation rewind to that message also drops its turn.

use hyperform_protocol::{
    AgentId, AgentInfo, CheckRecord, CheckpointInfo, CompactionMarker, Effort, Message, MessageId,
    ModeName, PlanId, Question, QuestionAnswer, RequestId, SessionId, TodoItem, TurnId, TurnRecord,
    Usage, decisions::TaskViewInfo,
};
use serde::{Deserialize, Serialize};

/// Schema of `log.jsonl` and `meta.json` written by this crate.
pub const SESSION_SCHEMA: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LogRecord {
    /// First record of every session log. Writers record a level in
    /// `mode`; older logs may hold `plan`, which replays as planning on.
    SessionStarted {
        schema: u32,
        session_id: SessionId,
        project_root: String,
        model: String,
        mode: ModeName,
        #[serde(default)]
        planning: bool,
        created_at: u64,
    },
    /// A main-agent transcript message: user input, assistant output, or a
    /// user message of tool results.
    Message {
        message: Message,
        turn_id: Option<TurnId>,
    },
    TurnStarted {
        turn_id: TurnId,
        message_id: MessageId,
        started_at: u64,
    },
    TurnFinished {
        turn: TurnRecord,
    },
    /// The complete todo list of one agent; replaces the previous one.
    Todos {
        agent_id: AgentId,
        todos: Vec<TodoItem>,
    },
    PlanProposed {
        request_id: RequestId,
        agent_id: AgentId,
        plan: String,
    },
    PlanResolved {
        request_id: RequestId,
        approved: bool,
        feedback: Option<String>,
        final_plan: Option<String>,
    },
    QuestionAsked {
        request_id: RequestId,
        agent_id: AgentId,
        questions: Vec<Question>,
    },
    /// `answers` is `None` when the user dismissed the questions.
    QuestionAnswered {
        request_id: RequestId,
        answers: Option<Vec<QuestionAnswer>>,
    },
    /// Audit trail of an approval decision; replay does not act on it.
    Approval {
        request_id: RequestId,
        tool: String,
        title: String,
        allowed: bool,
        rule: Option<String>,
    },
    /// Latest state of one agent run.
    AgentUpdated {
        info: AgentInfo,
    },
    Check {
        record: CheckRecord,
    },
    /// `snapshot` is the shadow commit holding the code state.
    Checkpoint {
        info: CheckpointInfo,
        snapshot: String,
    },
    /// `summary` (a user-role message) replaces the history before
    /// `marker.keep_from` in the working set; the transcript keeps it all.
    Compacted {
        marker: CompactionMarker,
        summary: Message,
    },
    /// With `conversation`, replay drops the message and everything recorded
    /// after it up to this record. `code` alone changes nothing on replay.
    Rewound {
        message_id: MessageId,
        conversation: bool,
        code: bool,
    },
    /// The permission level changed. Writers record a level and the
    /// planning switch as it stood. Logs from before planning was its own
    /// switch lack `planning`: there `plan` turned planning on and any
    /// other mode turned it off.
    ModeChanged {
        mode: ModeName,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        planning: Option<bool>,
    },
    /// Plan mode was turned on or off.
    PlanningChanged {
        planning: bool,
    },
    ModelChanged {
        model: String,
    },
    EffortChanged {
        effort: Option<Effort>,
    },
    Title {
        title: String,
    },
    /// Usage of side requests (titles, summaries) not covered by a
    /// `TurnFinished`.
    Usage {
        agent_id: AgentId,
        usage: Usage,
        cost_usd: f64,
    },
    /// Imported v1 notes and other annotations; replay ignores it.
    Note {
        text: String,
    },
    /// The session started working on a work plan of its project.
    PlanAttached {
        plan_id: PlanId,
    },
    /// The session stopped working on its plan.
    PlanDetached,
    /// `decisions_task_view`: the working set became the messages with the
    /// `working` ids, in order (`index` is the index message, which is not
    /// in the transcript); with `view.restored`, the full history again.
    TaskView {
        view: TaskViewInfo,
        #[serde(default)]
        working: Vec<MessageId>,
        #[serde(default)]
        index: Option<Message>,
    },
}
