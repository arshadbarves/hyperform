//! Payloads of the events and commands that decision uses add (cards,
//! chips, badges and their actions). The variants themselves live in
//! `events.rs` and `commands.rs`; their data lives here.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How a finished turn went, as the decision model judged it
/// (`decisions_pet_mood`); the pet shows it beside the turn's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TurnTone {
    Smooth,
    Struggling,
    Blocked,
    DoneWell,
}

/// How soon an Activity inbox item needs the user
/// (`decisions_inbox_priority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Urgency {
    Low,
    Normal,
    High,
}

/// The urgency of one inbox item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UrgencyInfo {
    /// The request id of an approval, question or plan, or `notice:<text>`
    /// for a notice.
    pub key: String,
    pub urgency: Urgency,
}

/// A routing choice made when a task started (`decisions_routing`): the
/// main agent's new task, or a subagent whose model is `inherit`. The
/// transcript shows it as a chip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RouteInfo {
    pub agent_id: crate::ids::AgentId,
    /// The reasoning effort chosen; `None` leaves the session's.
    pub effort: Option<crate::session::Effort>,
    /// The model chosen (main or fast); `None` leaves the session's.
    pub model: Option<String>,
    /// Why, in a few words ("complex task, confidence 0.91").
    pub reason: String,
}

/// An action a decision use offers as a card (plan first, save a rule,
/// run a review). Nothing happens until the user picks it; the answer
/// comes back as `Command::ResolveSuggestion`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Suggestion {
    /// `<feature id>:<input fingerprint>`, unique within the session.
    pub suggestion_id: String,
    pub kind: SuggestionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SuggestionKind {
    /// `decisions_plan_suggest`: the request looks large; switch to Plan mode.
    PlanFirst,
    /// `decisions_memory_suggest`: the message sets a standing rule; save
    /// it to the project's instruction files.
    SaveRule { rule: String },
    /// `decisions_review_suggest`: the turn changed risky areas; run the
    /// review command. `areas` are labels ("authentication", ...),
    /// `paths` root-relative files.
    Review {
        areas: Vec<String>,
        paths: Vec<String>,
    },
}

/// What the agent's final message claimed while no check backed the turn
/// (`decisions_completion_check`). It never changes the turn's badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UncheckedClaim {
    /// The message says the work is complete.
    pub done: bool,
    /// The message says tests or checks passed.
    pub checks: bool,
}

/// `decisions_task_view`: earlier exchanges set aside when a new task
/// started. The transcript shows a divider before `boundary` whose
/// "Include full history" action sends `Command::IncludeFullHistory`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskViewInfo {
    /// The user message that started the task.
    pub boundary: crate::ids::MessageId,
    /// How many earlier exchanges were set aside.
    pub set_aside: u32,
    /// Their estimated size in tokens.
    #[ts(type = "number")]
    pub tokens: u64,
    /// The user brought the full history back; the view no longer applies.
    pub restored: bool,
    #[ts(type = "number")]
    pub created_at: u64,
}
