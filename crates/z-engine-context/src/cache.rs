//! Prompt-cache breakpoint placement.

use z_engine_protocol::{Message, Role};

use crate::sections::PromptSection;

/// Indices of the last two user-role messages (tool-result rounds
/// included), ascending: the newest writes the cache for the next request,
/// the previous one reads what the last request wrote.
pub fn message_breakpoints(messages: &[Message]) -> Vec<usize> {
    let mut indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, message)| message.role == Role::User)
        .map(|(index, _)| index)
        .take(2)
        .collect();
    indices.reverse();
    indices
}

/// Index of the last cacheable system section, which carries the system
/// prompt's cache breakpoint.
pub fn system_breakpoint(sections: &[PromptSection]) -> Option<usize> {
    sections.iter().rposition(|section| section.cacheable)
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::{CallId, ContentBlock};

    use super::*;

    #[test]
    fn picks_the_last_two_user_messages_in_order() {
        assert!(message_breakpoints(&[]).is_empty());
        let one = [Message::user_text("hi"), Message::assistant_text("hello")];
        assert_eq!(message_breakpoints(&one), vec![0]);
        let many = [
            Message::user_text("a"),
            Message::assistant_text("b"),
            Message::user_text("c"),
            Message::assistant_text("d"),
            Message::new(
                Role::User,
                vec![ContentBlock::tool_result(CallId::from("t"), "ok", false)],
            ),
            Message::assistant_text("e"),
        ];
        assert_eq!(message_breakpoints(&many), vec![2, 4]);
    }

    #[test]
    fn system_breakpoint_is_the_last_cacheable_section() {
        assert_eq!(system_breakpoint(&[]), None);
        let sections = [
            PromptSection::cached("base"),
            PromptSection::cached("env"),
            PromptSection::uncached("extra"),
        ];
        assert_eq!(system_breakpoint(&sections), Some(1));
        assert_eq!(system_breakpoint(&sections[2..]), None);
    }
}
