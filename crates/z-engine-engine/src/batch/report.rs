//! Tool card events: `toolStarted` and `toolFinished` (with the result text
//! cut to a few KiB), including for calls refused before running.

use std::time::Duration;

use z_engine_protocol::{CallId, ContentBlock, Event, ToolResultPart, ToolStatus};

use super::gate::ToolCall;
use crate::run::RunContext;

/// Result text shown on the tool card.
const CARD_OUTPUT_BYTES: usize = 4 * 1024;

pub(super) fn started(ctx: &RunContext, call: &ToolCall, title: String) {
    ctx.core.events.emit(Event::ToolStarted {
        agent_id: ctx.spec.agent_id.clone(),
        call_id: call.id.clone(),
        tool: call.name.clone(),
        title,
        input: call.input.clone(),
    });
}

pub(super) fn finished(
    ctx: &RunContext,
    call_id: &CallId,
    status: ToolStatus,
    summary: String,
    content: &[ToolResultPart],
    duration: Duration,
) {
    let text = content
        .iter()
        .filter_map(|part| match part {
            ToolResultPart::Text { text } => Some(text.as_str()),
            ToolResultPart::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    ctx.core.events.emit(Event::ToolFinished {
        agent_id: ctx.spec.agent_id.clone(),
        call_id: call_id.clone(),
        status,
        summary,
        output: card_text(&text),
        duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
    });
}

/// A call that never ran: its card opens and closes at once, and the model
/// gets `message` as an error result.
pub(super) fn refused(
    ctx: &RunContext,
    call: &ToolCall,
    status: ToolStatus,
    message: &str,
) -> ContentBlock {
    started(ctx, call, call.name.clone());
    let content = vec![ToolResultPart::Text {
        text: message.to_string(),
    }];
    let summary = message.lines().next().unwrap_or_default().to_string();
    finished(ctx, &call.id, status, summary, &content, Duration::ZERO);
    ContentBlock::ToolResult {
        tool_use_id: call.id.clone(),
        content,
        is_error: true,
    }
}

/// Head and tail of `text` within the card budget, on char boundaries.
fn card_text(text: &str) -> String {
    if text.len() <= CARD_OUTPUT_BYTES {
        return text.to_string();
    }
    let half = CARD_OUTPUT_BYTES / 2;
    let mut head = half;
    while !text.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = text.len() - half;
    while !text.is_char_boundary(tail) {
        tail += 1;
    }
    format!(
        "{}\n… {} bytes omitted …\n{}",
        &text[..head],
        tail - head,
        &text[tail..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_text_keeps_head_and_tail() {
        assert_eq!(card_text("short"), "short");
        let long = format!("{}{}", "é".repeat(3_000), "z".repeat(3_000));
        let cut = card_text(&long);
        assert!(cut.len() < long.len());
        assert!(cut.starts_with('é') && cut.ends_with('z'));
        assert!(cut.contains("bytes omitted"));
    }
}
