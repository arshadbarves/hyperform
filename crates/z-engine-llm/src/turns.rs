//! Where the user last spoke. Tool results, and the `<system-reminder>`
//! blocks the engine appends to them, are agent traffic, not user input.

use z_engine_protocol::{ContentBlock, Message, Role};

/// A user message carrying something the user wrote or attached.
pub(crate) fn is_user_input(message: &Message) -> bool {
    message.role == Role::User
        && message.content.iter().any(|block| match block {
            ContentBlock::Text { text } => !text.trim().is_empty() && !is_reminder(text),
            ContentBlock::Image { .. } | ContentBlock::Document { .. } => true,
            _ => false,
        })
}

/// Index of the latest message that is user input.
pub(crate) fn last_user_input(messages: &[Message]) -> Option<usize> {
    messages.iter().rposition(is_user_input)
}

fn is_reminder(text: &str) -> bool {
    let text = text.trim();
    text.starts_with("<system-reminder>") && text.ends_with("</system-reminder>")
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::{CallId, MediaSource};

    use super::*;

    fn user(blocks: Vec<ContentBlock>) -> Message {
        Message::new(Role::User, blocks)
    }

    #[test]
    fn tool_rounds_and_reminders_are_not_user_input() {
        let result = ContentBlock::tool_result(CallId::from("c1"), "ok", false);
        let reminder = ContentBlock::text("<system-reminder>\nsteer\n</system-reminder>");
        assert!(!is_user_input(&user(vec![result.clone()])));
        assert!(!is_user_input(&user(vec![
            result.clone(),
            reminder.clone()
        ])));
        assert!(!is_user_input(&user(vec![ContentBlock::text("  ")])));
        assert!(!is_user_input(&Message::assistant_text("hi")));
        assert!(is_user_input(&user(vec![
            result,
            ContentBlock::text("stop")
        ])));
        assert!(is_user_input(&user(vec![
            ContentBlock::text("fix it"),
            reminder
        ])));
        let image = ContentBlock::Image {
            source: MediaSource::Url {
                url: "https://example.com/a.png".into(),
            },
        };
        assert!(is_user_input(&user(vec![image])));
    }

    #[test]
    fn last_user_input_skips_tool_rounds() {
        let messages = vec![
            Message::user_text("task"),
            Message::assistant_text("calling"),
            user(vec![ContentBlock::tool_result(
                CallId::from("c1"),
                "ok",
                false,
            )]),
        ];
        assert_eq!(last_user_input(&messages), Some(0));
        assert_eq!(last_user_input(&messages[1..]), None);
    }
}
