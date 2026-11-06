use serde_json::json;

use super::*;

fn call(id: &str) -> Message {
    Message::new(
        Role::Assistant,
        vec![
            ContentBlock::text("checking"),
            ContentBlock::ToolUse {
                id: CallId::from(id),
                name: "Read".into(),
                input: json!({"file_path": "a.rs"}),
            },
        ],
    )
}

fn result(id: &str) -> Message {
    Message::new(
        Role::User,
        vec![ContentBlock::tool_result(
            CallId::from(id),
            "contents",
            false,
        )],
    )
}

/// Tool-round message that also carries a steering/reminder text block.
fn result_with_reminder(id: &str) -> Message {
    Message::new(
        Role::User,
        vec![
            ContentBlock::tool_result(CallId::from(id), "contents", false),
            ContentBlock::text("<system-reminder>\nsteer\n</system-reminder>"),
        ],
    )
}

/// 0 user, 1 call, 2 result+reminder, 3 answer, 4 user, 5 call, 6 result,
/// 7 answer, 8 user, 9 answer.
fn history() -> Vec<Message> {
    vec![
        Message::user_text("first task"),
        call("a"),
        result_with_reminder("a"),
        Message::assistant_text("done a"),
        Message::user_text("second task"),
        call("b"),
        result("b"),
        Message::assistant_text("done b"),
        Message::user_text("third task"),
        Message::assistant_text("done c"),
    ]
}

#[test]
fn nothing_to_summarize_without_two_messages_before_a_split() {
    assert_eq!(plan_summary(&[], 0), None);
    assert_eq!(plan_summary(&[Message::user_text("hi")], 0), None);
    let two = [Message::user_text("hi"), Message::assistant_text("hello")];
    assert_eq!(plan_summary(&two, 0), None);
    // The only later message is an open tool round's result.
    let open = [Message::user_text("task"), call("a"), result("a")];
    assert_eq!(plan_summary(&open, 0), None);
}

#[test]
fn split_is_the_latest_turn_start_or_assistant_that_keeps_enough() {
    let messages = history();
    let split = |keep| plan_summary(&messages, keep).map(|plan| plan.split);
    assert_eq!(split(0), Some(9));
    assert_eq!(split(2), Some(8));
    assert_eq!(split(3), Some(7));
    assert_eq!(split(5), Some(5), "assistant opening a closed round");
    assert_eq!(split(6), Some(4));
    assert_eq!(split(7), Some(3));
    // Index 2 closes round `a` but is a tool-result message, so keeping 8
    // cannot be honoured; fall back to the latest valid split.
    assert_eq!(split(8), Some(9));
    assert_eq!(split(messages.len() + 3), Some(9));
}

#[test]
fn tool_rounds_with_reminder_text_are_not_split_points() {
    let messages = vec![
        Message::user_text("task"),
        call("a"),
        result_with_reminder("a"),
        Message::assistant_text("done"),
    ];
    assert!(
        !messages[2].is_tool_results(),
        "precondition: mixed content"
    );
    assert_eq!(plan_summary(&messages, 0), Some(SummaryPlan { split: 3 }));
    assert_eq!(plan_summary(&messages, 2), Some(SummaryPlan { split: 3 }));
}

#[test]
fn late_results_keep_their_call_on_the_same_side() {
    // A result delivered after a later user message still pins its call,
    // so neither the user message (2) nor the result (3) can start the tail.
    let messages = vec![
        Message::user_text("task"),
        call("late"),
        Message::user_text("are you there?"),
        result("late"),
        Message::assistant_text("done"),
    ];
    assert_eq!(plan_summary(&messages, 0), Some(SummaryPlan { split: 4 }));
    assert_eq!(plan_summary(&messages, 3), Some(SummaryPlan { split: 4 }));
}

#[test]
fn apply_summary_keeps_the_tail_verbatim() {
    let messages = history();
    let plan = plan_summary(&messages, 2).unwrap();
    let summary = summary_message("Did tasks a and b.");
    let compacted = apply_summary(&messages, &plan, &summary);
    assert_eq!(compacted.len(), 3);
    assert_eq!(compacted[0], summary);
    assert_eq!(compacted[1..], messages[8..]);
    let stale = apply_summary(&messages, &SummaryPlan { split: 99 }, &summary);
    assert_eq!(stale, vec![summary]);
}

#[test]
fn summary_message_is_a_user_message_embedding_the_summary() {
    let message = summary_message("\n- Implemented the parser.\n");
    assert_eq!(message.role, Role::User);
    let text = message.text();
    assert!(text.contains("- Implemented the parser."), "{text}");
    assert!(!text.contains("{{"), "{text}");
    assert_eq!(text, text.trim());
}
