//! Shared v2 contracts. This crate performs no I/O: it only defines the
//! values exchanged between the engine, the store, and the desktop GUI.
//!
//! Every public type derives `ts_rs::TS`; `cargo test -p z-engine-protocol`
//! regenerates the TypeScript mirror under `ui/src/lib/protocol/`.

pub mod agents;
pub mod commands;
pub mod content;
pub mod decisions;
pub mod events;
pub mod ids;
pub mod interaction;
pub mod jobs;
pub mod permission;
pub mod session;
pub mod time;
pub mod usage;
pub mod verification;
pub mod work;

pub use agents::{AgentInfo, AgentStatus, Isolation, WorktreeInfo, WorktreeState};
pub use commands::{Attachment, Command};
pub use content::{ContentBlock, MediaSource, Message, Role, ToolResultPart};
pub use events::{Event, EventEnvelope, NoticeLevel, ToolStatus};
pub use ids::{
    AgentId, CallId, CheckpointId, EntryId, JobId, MessageId, PlanId, RequestId, SessionId, TaskId,
    TurnId,
};
pub use interaction::{
    PlanDecision, Question, QuestionAnswer, QuestionOption, TodoItem, TodoStatus,
};
pub use jobs::{JobInfo, JobKind, JobStatus};
pub use permission::{ApprovalDecision, ApprovalRequest, ModeName, PermissionMode, Preview};
pub use session::{
    CheckpointInfo, CompactionMarker, CompactionTrigger, ContextBreakdown, Effort, PendingPlan,
    PendingQuestion, RewindScope, SessionInfo, SessionSnapshot, SessionStatus, SessionSummary,
    TurnOutcome, TurnRecord,
};
pub use time::now_ms;
pub use usage::Usage;
pub use verification::{CheckKind, CheckRecord, TestCounts, VerificationMode, VerificationOutcome};
