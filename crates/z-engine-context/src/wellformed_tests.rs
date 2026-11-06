use serde_json::json;
use z_engine_protocol::{CallId, ContentBlock, Message, Role};

use super::*;

fn call(id: &str) -> ContentBlock {
    ContentBlock::ToolUse {
        id: CallId::from(id),
        name: "Read".into(),
        input: json!({}),
    }
}

fn result(id: &str) -> ContentBlock {
    ContentBlock::tool_result(CallId::from(id), format!("result {id}"), false)
}

fn assistant(blocks: Vec<ContentBlock>) -> Message {
    Message::new(Role::Assistant, blocks)
}

fn user(blocks: Vec<ContentBlock>) -> Message {
    Message::new(Role::User, blocks)
}

fn results_of(message: &Message) -> Vec<(String, bool)> {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                is_error,
                ..
            } => Some((tool_use_id.0.clone(), *is_error)),
            _ => None,
        })
        .collect()
}

fn roles(messages: &[Message]) -> Vec<Role> {
    messages.iter().map(|m| m.role).collect()
}

#[test]
fn a_valid_transcript_is_unchanged() {
    let messages = vec![
        Message::user_text("go"),
        assistant(vec![call("a"), call("b")]),
        user(vec![result("a"), result("b")]),
        Message::assistant_text("done"),
    ];
    assert_eq!(well_formed(messages.clone()), messages);
}

#[test]
fn neighbours_of_one_role_merge_in_order() {
    let first = Message::user_text("first");
    let first_id = first.id.clone();
    let merged = well_formed(vec![first, user(Vec::new()), Message::user_text("second")]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].id, first_id);
    assert_eq!(merged[0].text(), "first\nsecond");
}

#[test]
fn a_call_without_its_result_gets_one_before_the_next_prompt() {
    let fixed = well_formed(vec![
        Message::user_text("go"),
        assistant(vec![call("lost")]),
        Message::user_text("are you there?"),
    ]);
    assert_eq!(roles(&fixed), [Role::User, Role::Assistant, Role::User]);
    assert_eq!(results_of(&fixed[2]), [("lost".to_string(), true)]);
    assert!(matches!(
        fixed[2].content[0],
        ContentBlock::ToolResult { .. }
    ));
    assert_eq!(fixed[2].text(), "are you there?");

    let trailing = well_formed(vec![Message::user_text("go"), assistant(vec![call("x")])]);
    assert_eq!(roles(&trailing), [Role::User, Role::Assistant, Role::User]);
    assert_eq!(results_of(&trailing[2]), [("x".to_string(), true)]);
}

#[test]
fn late_duplicate_and_orphan_results_are_dropped() {
    let fixed = well_formed(vec![
        user(vec![
            result("before"),
            ContentBlock::Text { text: "go".into() },
        ]),
        assistant(vec![call("a")]),
        user(vec![result("a"), result("a")]),
        Message::assistant_text("next"),
        user(vec![result("late")]),
        Message::assistant_text("more"),
        Message::user_text("again"),
    ]);
    assert_eq!(
        roles(&fixed),
        [
            Role::User,
            Role::Assistant,
            Role::User,
            Role::Assistant,
            Role::User
        ]
    );
    assert!(results_of(&fixed[0]).is_empty());
    assert_eq!(results_of(&fixed[2]), [("a".to_string(), false)]);
    assert_eq!(fixed[3].text(), "next\nmore");
    assert_eq!(fixed[4].text(), "again");
}
