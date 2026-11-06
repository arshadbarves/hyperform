use serde_json::json;
use z_engine_protocol::MediaSource;

use super::*;

fn image() -> MediaSource {
    MediaSource::Base64 {
        media_type: "image/png".into(),
        data: "AAAA".into(),
    }
}

#[test]
fn renders_speakers_calls_and_results_and_omits_thinking() {
    let messages = vec![
        Message::new(
            Role::User,
            vec![
                ContentBlock::text("Fix the parser"),
                ContentBlock::Image { source: image() },
                ContentBlock::Document {
                    source: image(),
                    title: Some("spec.pdf".into()),
                },
            ],
        ),
        Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Thinking {
                    text: "secret reasoning".into(),
                    signature: None,
                },
                ContentBlock::RedactedThinking {
                    data: "opaque".into(),
                },
                ContentBlock::text("Reading it."),
                ContentBlock::ToolUse {
                    id: CallId::from("c1"),
                    name: "Read".into(),
                    input: json!({"file_path": "src/parser.rs"}),
                },
            ],
        ),
        Message::new(
            Role::User,
            vec![
                ContentBlock::tool_result(CallId::from("c1"), "fn parse() {}", false),
                ContentBlock::ToolResult {
                    tool_use_id: CallId::from("gone"),
                    content: vec![ToolResultPart::Image { source: image() }],
                    is_error: true,
                },
            ],
        ),
    ];
    let out = render_for_summary(&messages, 1_000);
    assert_eq!(
        out,
        "User: Fix the parser\n\n\
         User: [image]\n\n\
         User: [document: spec.pdf]\n\n\
         Assistant: Reading it.\n\n\
         Tool call Read {\"file_path\":\"src/parser.rs\"}\n\n\
         Tool result (Read): fn parse() {}\n\n\
         Tool error: [image]"
    );
    assert!(!out.contains("secret reasoning"));
    assert!(!out.contains("opaque"));
}

#[test]
fn long_results_keep_head_and_tail() {
    let body = format!("{}{}", "a".repeat(50), "z".repeat(50));
    let messages = vec![Message::new(
        Role::User,
        vec![ContentBlock::tool_result(CallId::from("c"), body, false)],
    )];
    let out = render_for_summary(&messages, 10);
    assert_eq!(out, "Tool result: aaaaa\n[… 90 chars truncated …]\nzzzzz");
}

#[test]
fn tool_input_strings_are_clipped_but_keys_survive() {
    let messages = vec![Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse {
            id: CallId::from("w"),
            name: "Write".into(),
            input: json!({"content": "x".repeat(500), "file_path": "src/big.rs"}),
        }],
    )];
    let out = render_for_summary(&messages, 20);
    assert!(out.starts_with("Tool call Write {"), "{out}");
    assert!(out.contains("\"file_path\":\"src/big.rs\""), "{out}");
    assert!(out.contains("480 chars truncated"), "{out}");
    assert!(out.chars().count() < 120, "{out}");
}

#[test]
fn truncation_respects_multibyte_characters() {
    assert_eq!(truncate("héllo wörld", 20), "héllo wörld");
    assert_eq!(truncate("ééééé", 2), "é\n[… 3 chars truncated …]\né");
    assert_eq!(truncate("abc", 0), "\n[… 3 chars truncated …]\n");
}
