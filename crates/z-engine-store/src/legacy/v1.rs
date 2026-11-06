//! The v1 transcript wire format, mirrored here because v2 never imports a
//! v1 crate. Unknown event types decode as unrecognized lines and are
//! skipped by the reader.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum V1Event {
    Meta {
        model: String,
        project_root: String,
    },
    UserMsg {
        text: String,
        /// Data URLs (`data:image/png;base64,...`) or plain URLs.
        #[serde(default)]
        images: Vec<String>,
    },
    AssistantMsg {
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        tool_calls: Vec<V1ToolCall>,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
    },
    Note {
        text: String,
    },
    Title {
        text: String,
    },
    /// `completed`, `aborted`, or `failed`.
    TurnEnd {
        outcome: String,
    },
    TaskUpdated {
        report: V1TaskReport,
    },
    /// The user opened the session; it clears the unread marker.
    Ack,
}

#[derive(Debug, Deserialize)]
pub(super) struct V1ToolCall {
    pub(super) id: String,
    pub(super) name: String,
    /// JSON-encoded arguments exactly as the model streamed them.
    #[serde(default)]
    pub(super) arguments: String,
}

/// The parts of a v1 verification report kept as an audit note.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct V1TaskReport {
    pub(super) goal: String,
    /// Snake-case task status, e.g. `complete` or `needs_verification`.
    pub(super) status: String,
    pub(super) blockers: Vec<String>,
    pub(super) checks: Vec<V1Check>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct V1Check {
    pub(super) outcome: String,
    pub(super) summary: String,
}
