//! Local token estimates for context-pressure checks and `/context`.
//! Provider-reported usage stays authoritative; these only need to be close.

use serde_json::Value;
use z_engine_protocol::{ContentBlock, Message, ToolResultPart};

/// Flat estimate for one image or document.
pub const MEDIA_TOKENS: u64 = 1_600;

/// About four characters per token, rounded up; CJK characters count as
/// one token each because tokenizers rarely merge them.
pub fn estimate_text(text: &str) -> u64 {
    let (mut cjk, mut other) = (0u64, 0u64);
    for c in text.chars() {
        if is_cjk(c) {
            cjk += 1;
        } else {
            other += 1;
        }
    }
    cjk + other.div_ceil(4)
}

/// Thinking text counts; tool calls count their name and compact JSON input.
pub fn estimate_block(block: &ContentBlock) -> u64 {
    match block {
        ContentBlock::Text { text } | ContentBlock::Thinking { text, .. } => estimate_text(text),
        ContentBlock::RedactedThinking { data } => estimate_text(data),
        ContentBlock::Image { .. } | ContentBlock::Document { .. } => MEDIA_TOKENS,
        ContentBlock::ToolUse { name, input, .. } => estimate_text(name) + estimate_json(input),
        ContentBlock::ToolResult { content, .. } => content.iter().map(estimate_part).sum(),
    }
}

pub fn estimate_message(message: &Message) -> u64 {
    message.content.iter().map(estimate_block).sum()
}

pub fn estimate_messages(messages: &[Message]) -> u64 {
    messages.iter().map(estimate_message).sum()
}

/// Tool definitions as `(name, description, input schema)`.
pub fn estimate_tools(tools: &[(String, String, Value)]) -> u64 {
    tools
        .iter()
        .map(|(name, description, schema)| {
            estimate_text(name) + estimate_text(description) + estimate_json(schema)
        })
        .sum()
}

fn estimate_part(part: &ToolResultPart) -> u64 {
    match part {
        ToolResultPart::Text { text } => estimate_text(text),
        ToolResultPart::Image { .. } => MEDIA_TOKENS,
    }
}

fn estimate_json(value: &Value) -> u64 {
    estimate_text(&value.to_string())
}

/// Han, kana, hangul, CJK punctuation and full-width forms.
fn is_cjk(c: char) -> bool {
    matches!(
        u32::from(c),
        0x1100..=0x11FF
            | 0x2E80..=0x2FDF
            | 0x3000..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7FF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFFEF
            | 0x20000..=0x3FFFF
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_protocol::{CallId, MediaSource, Role};

    use super::*;

    fn image() -> MediaSource {
        MediaSource::Url {
            url: "https://example.com/a.png".into(),
        }
    }

    #[test]
    fn text_is_chars_over_four_rounded_up() {
        assert_eq!(estimate_text(""), 0);
        assert_eq!(estimate_text("abcd"), 1);
        assert_eq!(estimate_text("abcde"), 2);
        assert_eq!(estimate_text(&"é".repeat(8)), 2);
    }

    #[test]
    fn cjk_characters_count_one_token_each() {
        assert_eq!(estimate_text("日本語"), 3);
        assert_eq!(estimate_text("한국어"), 3);
        assert_eq!(estimate_text("かなカナ"), 4);
        assert_eq!(estimate_text("ab日本"), 3);
    }

    #[test]
    fn blocks_price_media_tool_calls_and_results() {
        assert_eq!(
            estimate_block(&ContentBlock::Image { source: image() }),
            MEDIA_TOKENS
        );
        assert_eq!(
            estimate_block(&ContentBlock::Document {
                source: image(),
                title: None,
            }),
            MEDIA_TOKENS
        );
        let input = json!({"file_path": "src/lib.rs"});
        let call = ContentBlock::ToolUse {
            id: CallId::from("c1"),
            name: "Read".into(),
            input: input.clone(),
        };
        assert_eq!(
            estimate_block(&call),
            estimate_text("Read") + estimate_text(&input.to_string())
        );
        let result = ContentBlock::ToolResult {
            tool_use_id: CallId::from("c1"),
            content: vec![
                ToolResultPart::Text {
                    text: "x".repeat(40),
                },
                ToolResultPart::Image { source: image() },
            ],
            is_error: false,
        };
        assert_eq!(estimate_block(&result), 10 + MEDIA_TOKENS);
        let thinking = ContentBlock::Thinking {
            text: "abcdefgh".into(),
            signature: Some("sig".into()),
        };
        assert_eq!(estimate_block(&thinking), 2);
    }

    #[test]
    fn messages_and_tools_sum_their_parts() {
        let messages = vec![
            Message::user_text("abcd"),
            Message::new(
                Role::Assistant,
                vec![ContentBlock::text("abcd"), ContentBlock::text("abcdefgh")],
            ),
        ];
        assert_eq!(estimate_message(&messages[1]), 3);
        assert_eq!(estimate_messages(&messages), 4);
        let schema = json!({"type": "object"});
        let tools = vec![(
            "Read".to_string(),
            "Reads a file.".to_string(),
            schema.clone(),
        )];
        assert_eq!(
            estimate_tools(&tools),
            1 + estimate_text("Reads a file.") + estimate_text(&schema.to_string())
        );
    }
}
