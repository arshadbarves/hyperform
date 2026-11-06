use serde_json::json;
use z_engine_protocol::{MediaSource, Role};

use super::*;

fn call(id: &str) -> Message {
    Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse {
            id: CallId::from(id),
            name: "Bash".into(),
            input: json!({"command": "ls"}),
        }],
    )
}

fn result(id: &str, text: &str) -> Message {
    Message::new(
        Role::User,
        vec![ContentBlock::tool_result(CallId::from(id), text, false)],
    )
}

/// user, then five call/result rounds with outputs of 100 chars except
/// round `c3` (5 chars).
fn history() -> Vec<Message> {
    let mut messages = vec![Message::user_text("list things")];
    for (id, len) in [
        ("c1", 100),
        ("c2", 100),
        ("c3", 5),
        ("c4", 100),
        ("c5", 100),
    ] {
        messages.push(call(id));
        messages.push(result(id, &"x".repeat(len)));
    }
    messages
}

fn ids(targets: &[ClearTarget]) -> Vec<&str> {
    targets
        .iter()
        .map(|target| target.call_id.as_str())
        .collect()
}

fn first_result_text(message: &Message) -> String {
    match &message.content[0] {
        ContentBlock::ToolResult { content, .. } => result_text(content),
        other => panic!("tool result expected, got {other:?}"),
    }
}

#[test]
fn keeps_the_newest_results_and_skips_small_ones() {
    let messages = history();
    let targets = plan_microcompact(&messages, 2, 50);
    assert_eq!(ids(&targets), ["c1", "c2"]);
    assert_eq!(targets[0].message, 2);
    assert_eq!(targets[0].block, 0);
    assert_eq!(targets[0].chars, 100);
    assert!(plan_microcompact(&messages, 5, 0).is_empty());
    assert_eq!(
        ids(&plan_microcompact(&messages, 0, 50)),
        ["c1", "c2", "c4", "c5"]
    );
    assert_eq!(
        plan_microcompact(&messages, 0, 100).len(),
        0,
        "must exceed min"
    );
}

#[test]
fn apply_replaces_text_and_later_plans_skip_cleared_results() {
    let mut messages = history();
    let targets = plan_microcompact(&messages, 2, 50);
    let applied = apply_microcompact(&mut messages, &targets, |target, original| {
        assert_eq!(original.chars().count(), target.chars);
        format!(
            "{CLEARED_PREFIX}: {} chars saved to /artifacts/{}.txt]",
            target.chars, target.call_id
        )
    });
    assert_eq!(applied, 2);
    assert_eq!(
        messages[2].content[0],
        ContentBlock::tool_result(
            CallId::from("c1"),
            "[cleared: 100 chars saved to /artifacts/c1.txt]",
            false
        )
    );
    assert_eq!(first_result_text(&messages[8]), "x".repeat(100));
    assert!(plan_microcompact(&messages, 2, 20).is_empty());
}

#[test]
fn apply_keeps_images_and_error_flag() {
    let image = ToolResultPart::Image {
        source: MediaSource::Url {
            url: "https://example.com/shot.png".into(),
        },
    };
    let mut messages = vec![
        call("s1"),
        Message::new(
            Role::User,
            vec![
                ContentBlock::text("steering note"),
                ContentBlock::ToolResult {
                    tool_use_id: CallId::from("s1"),
                    content: vec![
                        ToolResultPart::Text {
                            text: "a".repeat(30),
                        },
                        image.clone(),
                        ToolResultPart::Text {
                            text: "b".repeat(30),
                        },
                    ],
                    is_error: true,
                },
            ],
        ),
    ];
    let targets = plan_microcompact(&messages, 0, 10);
    assert_eq!(targets.len(), 1);
    assert_eq!((targets[0].block, targets[0].chars), (1, 61));
    apply_microcompact(&mut messages, &targets, |_, _| "[cleared]".into());
    assert_eq!(
        messages[1].content[1],
        ContentBlock::ToolResult {
            tool_use_id: CallId::from("s1"),
            content: vec![
                ToolResultPart::Text {
                    text: "[cleared]".into()
                },
                image
            ],
            is_error: true,
        }
    );
}

#[test]
fn stale_targets_are_skipped() {
    let mut messages = history();
    let before = messages.clone();
    let mut targets = plan_microcompact(&messages, 2, 50);
    targets[0].call_id = CallId::from("other");
    targets[1].message = 99;
    let applied = apply_microcompact(&mut messages, &targets, |_, _| "[cleared]".into());
    assert_eq!(applied, 0);
    assert_eq!(messages, before);
}
