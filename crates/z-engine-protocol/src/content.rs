//! Provider-neutral conversation model. Adapters in `z-engine-llm` map it
//! to each wire format; the store persists it verbatim.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::ids::{CallId, MessageId};
use crate::time::now_ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Message {
    pub id: MessageId,
    pub role: Role,
    pub content: Vec<ContentBlock>,
    #[ts(type = "number")]
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum MediaSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ToolResultPart {
    Text { text: String },
    Image { source: MediaSource },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        source: MediaSource,
    },
    /// A PDF or other document the model can read natively.
    Document {
        source: MediaSource,
        #[serde(default)]
        title: Option<String>,
    },
    /// Model reasoning. `signature` must round-trip for Anthropic tool use.
    Thinking {
        text: String,
        #[serde(default)]
        signature: Option<String>,
    },
    RedactedThinking {
        data: String,
    },
    ToolUse {
        id: CallId,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: CallId,
        content: Vec<ToolResultPart>,
        #[serde(default)]
        is_error: bool,
    },
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn tool_result(tool_use_id: CallId, text: impl Into<String>, is_error: bool) -> Self {
        Self::ToolResult {
            tool_use_id,
            content: vec![ToolResultPart::Text { text: text.into() }],
            is_error,
        }
    }
}

impl Message {
    pub fn new(role: Role, content: Vec<ContentBlock>) -> Self {
        Self {
            id: MessageId::new(),
            role,
            content,
            created_at: now_ms(),
        }
    }

    pub fn user_text(text: impl Into<String>) -> Self {
        Self::new(Role::User, vec![ContentBlock::text(text)])
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self::new(Role::Assistant, vec![ContentBlock::text(text)])
    }

    /// Concatenated text blocks (thinking and tool payloads excluded).
    pub fn text(&self) -> String {
        let mut out = String::new();
        for block in &self.content {
            if let ContentBlock::Text { text } = block {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
        }
        out
    }

    /// `(id, name, input)` for every tool call in this message.
    pub fn tool_uses(&self) -> impl Iterator<Item = (&CallId, &str, &Value)> {
        self.content.iter().filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => Some((id, name.as_str(), input)),
            _ => None,
        })
    }

    pub fn has_tool_use(&self) -> bool {
        self.tool_uses().next().is_some()
    }

    /// True when every block is a tool result (a tool-round user message).
    pub fn is_tool_results(&self) -> bool {
        !self.content.is_empty()
            && self
                .content
                .iter()
                .all(|block| matches!(block, ContentBlock::ToolResult { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_use_camel_case_tags_and_fields() {
        let block = ContentBlock::tool_result(CallId::from("c1"), "ok", false);
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "toolResult");
        assert_eq!(json["toolUseId"], "c1");
        assert_eq!(json["isError"], false);
    }

    #[test]
    fn message_helpers_extract_text_and_tool_uses() {
        let msg = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::text("a"),
                ContentBlock::ToolUse {
                    id: CallId::from("t"),
                    name: "Read".into(),
                    input: serde_json::json!({"file_path": "x"}),
                },
                ContentBlock::text("b"),
            ],
        );
        assert_eq!(msg.text(), "a\nb");
        assert_eq!(msg.tool_uses().count(), 1);
        assert!(!msg.is_tool_results());
    }
}
