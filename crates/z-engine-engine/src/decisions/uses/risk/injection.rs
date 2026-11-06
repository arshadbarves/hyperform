//! The injection half: the head of a WebFetch, WebSearch or MCP result is
//! screened for instructions aimed at the agent. A flagged result gets a
//! warning note in front of it; nothing is blocked or removed.

use serde_json::json;
use z_engine_context::render_template;
use z_engine_decisions::{DecisionRequest, Question, UNCHANGED};
use z_engine_prompts::decisions::RISK_INJECTION;
use z_engine_prompts::reminders::INJECTION_WARNING;
use z_engine_protocol::ToolResultPart;
use z_engine_tools::names;

use crate::batch::ToolCall;
use crate::decisions::context::UseContext;
use crate::decisions::uses::digest::head;

pub(super) const QUESTION: &str = "risk_injection";
const MCP_PREFIX: &str = "mcp__";
const HEAD_CHARS: usize = 1_500;

/// Results that come from outside the project: web pages, searches, MCP.
pub(super) fn screened(tool: &str) -> bool {
    tool == names::WEB_FETCH || tool == names::WEB_SEARCH || tool.starts_with(MCP_PREFIX)
}

pub(super) async fn screen(
    cx: &UseContext,
    call: &ToolCall,
    output: &[ToolResultPart],
) -> Vec<String> {
    if !screened(&call.name) {
        return Vec::new();
    }
    let text = output
        .iter()
        .filter_map(|part| match part {
            ToolResultPart::Text { text } => Some(text.as_str()),
            ToolResultPart::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let Ok(question) = Question::yes_no(QUESTION, RISK_INJECTION) else {
        return Vec::new();
    };
    if text.trim().is_empty() {
        return Vec::new();
    }
    let state = json!({ "tool": call.name, "result": head(&text, HEAD_CHARS) });
    let request = DecisionRequest::new(state).ask(question);
    let Some(answer) = cx.ask(&request).await.into_iter().next() else {
        return Vec::new();
    };
    let flagged = answer.yes() == Some(true);
    let outcome = if flagged { "flagged" } else { UNCHANGED };
    cx.record(
        cx.record_of(&answer, &request.fingerprint())
            .outcome(outcome),
    );
    if !flagged {
        return Vec::new();
    }
    vec![render_template(INJECTION_WARNING, &[("tool", &call.name)])]
}
