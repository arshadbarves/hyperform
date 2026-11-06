//! Token usage by prompt layer, for `/context`.

use z_engine_protocol::{ContextBreakdown, Message};

use crate::sections::PromptSection;
use crate::tokens::{estimate_messages, estimate_text};

/// `system` covers every section except the instructions share (reported
/// separately, since the instructions section sits among the system
/// sections); `total` is the sum of the four layers.
pub fn context_breakdown(
    system: &[PromptSection],
    tools_tokens: u64,
    instructions_tokens: u64,
    messages: &[Message],
    limit: u64,
) -> ContextBreakdown {
    let sections: u64 = system
        .iter()
        .map(|section| estimate_text(&section.text))
        .sum();
    let system = sections.saturating_sub(instructions_tokens);
    let messages = estimate_messages(messages);
    ContextBreakdown {
        system,
        tools: tools_tokens,
        instructions: instructions_tokens,
        messages,
        total: system
            .saturating_add(tools_tokens)
            .saturating_add(instructions_tokens)
            .saturating_add(messages),
        limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_sum_to_the_total() {
        let system = [
            PromptSection::cached("a".repeat(400)),
            PromptSection::cached("b".repeat(200)),
        ];
        let messages = [
            Message::user_text("c".repeat(40)),
            Message::assistant_text("d".repeat(8)),
        ];
        let breakdown = context_breakdown(&system, 70, 50, &messages, 200_000);
        assert_eq!(
            breakdown,
            ContextBreakdown {
                system: 100,
                tools: 70,
                instructions: 50,
                messages: 12,
                total: 232,
                limit: 200_000,
            }
        );
    }

    #[test]
    fn oversized_instruction_share_saturates() {
        let breakdown = context_breakdown(&[PromptSection::cached("abcd")], 0, 9, &[], 10);
        assert_eq!(breakdown.system, 0);
        assert_eq!(breakdown.total, 9);
    }
}
