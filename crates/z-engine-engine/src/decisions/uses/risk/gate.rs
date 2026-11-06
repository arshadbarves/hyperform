//! The gate half. Calls the tool itself marks read-only (Bash through the
//! policy's shell analysis) are never asked about. For the rest, two
//! questions about one compact state (the user's latest request, the call,
//! the agent's own explanation): a `needs_approval` or `harmful` risk, or an
//! `off_task` or `injected` intent, turns the Allow into an Ask. In bypass
//! mode nothing asks; a notice says what the review saw.

use z_engine_decisions::{Answer, DecisionRequest, Question, UNCHANGED};
use z_engine_prompts::decisions::{RISK_INTENT, RISK_LEVEL};
use z_engine_protocol::{NoticeLevel, PermissionMode};

use crate::batch::ToolCall;
use crate::decisions::context::UseContext;
use crate::decisions::uses::digest::call_state;

pub(super) const INTENT: &str = "risk_intent";
pub(super) const LEVEL: &str = "risk_level";

/// The reason an allowed call should be shown to the user, if any.
pub(super) fn concern(intent: Option<&str>, level: Option<&str>) -> Option<&'static str> {
    match (intent, level) {
        (_, Some("harmful")) => Some("looks harmful"),
        (Some("injected"), _) => {
            Some("seems driven by instructions found in a file, page or tool result")
        }
        (_, Some("needs_approval")) => Some("looks like it needs your approval"),
        (Some("off_task"), _) => Some("does not seem to serve your request"),
        _ => None,
    }
}

pub(super) async fn review(cx: &UseContext, call: &ToolCall) -> Option<String> {
    let tool = cx.core.tools().get(&call.name)?;
    if tool.is_read_only(&call.input) {
        return None;
    }
    let (Ok(intent), Ok(level)) = (
        Question::choice(INTENT, RISK_INTENT),
        Question::choice(LEVEL, RISK_LEVEL),
    ) else {
        return None;
    };
    let state = cx.core.with_state(|state| call_state(&state.working, call));
    let request = DecisionRequest::new(state).ask(intent).ask(level);
    let answers = cx.ask(&request).await;
    let choice = |name: &str| {
        let answer = answers.iter().find(|answer| answer.question == name);
        answer.and_then(Answer::choice)
    };
    let concern = concern(choice(INTENT), choice(LEVEL));
    let bypass = cx.core.mode() == PermissionMode::Bypass;
    let outcome = match (concern, bypass) {
        (None, _) => UNCHANGED,
        (Some(_), true) => "noticed",
        (Some(_), false) => "asked",
    };
    let fingerprint = request.fingerprint();
    for answer in &answers {
        cx.record(cx.record_of(answer, &fingerprint).outcome(outcome));
    }
    let reason = format!("Risk review: this {} call {}.", call.name, concern?);
    if !bypass {
        return Some(reason);
    }
    if !cx.shadow {
        let text = format!("{reason} Bypass mode runs it without asking.");
        cx.core.events.notice(NoticeLevel::Warn, text);
    }
    None
}
