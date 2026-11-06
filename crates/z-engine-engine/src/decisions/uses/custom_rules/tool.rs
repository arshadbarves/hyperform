//! `PreToolUse` rules. `ask` and `notice` rules are asked before an allowed
//! call; `remind` rules after it ran, so their note reaches the agent that
//! made the call. Bypass mode never asks: an `ask` rule posts a notice
//! instead.

use regex::Regex;
use z_engine_config::DecisionRule;
use z_engine_context::render_template;
use z_engine_prompts::decisions::CUSTOM_RULE_TOOL;
use z_engine_prompts::reminders::CUSTOM_RULE_REMIND;
use z_engine_protocol::{NoticeLevel, PermissionMode};

use super::judge::{Fired, fired};
use crate::batch::ToolCall;
use crate::decisions::context::UseContext;
use crate::decisions::uses::digest::call_state;

const PRE_TOOL_USE: &str = "PreToolUse";

/// A reason to ask before the call, from the `ask` rules that fired;
/// fired `notice` rules post a notice.
pub(super) async fn before_call(cx: &UseContext, call: &ToolCall) -> Option<String> {
    let rules = rules_for(cx, call, &["ask", "notice"]);
    if rules.is_empty() {
        return None;
    }
    let bypass = cx.core.mode() == PermissionMode::Bypass;
    let asks = |rule: &DecisionRule| rule.action == "ask" && !bypass;
    let state = cx.core.with_state(|state| call_state(&state.working, call));
    let outcome = |rule: &DecisionRule| if asks(rule) { "asked" } else { "noticed" };
    let fired = fired(cx, rules, state, CUSTOM_RULE_TOOL, outcome).await;
    let mut reasons = Vec::new();
    for Fired { rule, answer } in &fired {
        let what = describe(rule, answer);
        if asks(rule) {
            reasons.push(what);
        } else if !cx.shadow {
            let text = format!("Decision rule on {}: {what}", call.name);
            cx.core.events.notice(NoticeLevel::Info, text);
        }
    }
    (!reasons.is_empty()).then(|| format!("Decision rule: {}", reasons.join("; ")))
}

/// Notes for the agent from the `remind` rules that fired.
pub(super) async fn remind(cx: &UseContext, call: &ToolCall) -> Vec<String> {
    let rules = rules_for(cx, call, &["remind"]);
    if rules.is_empty() {
        return Vec::new();
    }
    let state = cx.core.with_state(|state| call_state(&state.working, call));
    let fired = fired(cx, rules, state, CUSTOM_RULE_TOOL, |_| "reminded").await;
    fired
        .iter()
        .map(|Fired { rule, answer }| {
            let fields = [
                ("question", rule.question.as_str()),
                ("answer", answer.as_str()),
                ("tool", call.name.as_str()),
            ];
            render_template(CUSTOM_RULE_REMIND, &fields)
        })
        .collect()
}

/// `Does this touch production? yes`, led by the rule's name when it has one.
pub(super) fn describe(rule: &DecisionRule, answer: &str) -> String {
    match &rule.name {
        Some(name) => format!("{name} ({} {answer})", rule.question),
        None => format!("{} {answer}", rule.question),
    }
}

fn rules_for(cx: &UseContext, call: &ToolCall, actions: &[&str]) -> Vec<DecisionRule> {
    let settings = cx.core.settings();
    let rules = settings.settings.decisions.rules.iter().filter(|rule| {
        rule.event == PRE_TOOL_USE
            && actions.contains(&rule.action.as_str())
            && matches(rule.matcher.as_deref(), &call.name)
    });
    rules.cloned().collect()
}

/// Hook matcher semantics: an anchored regex over the tool name; unset,
/// empty or `*` match every tool.
pub(super) fn matches(matcher: Option<&str>, tool: &str) -> bool {
    match matcher.map(str::trim) {
        None | Some("" | "*") => true,
        Some(pattern) => Regex::new(&format!("^(?:{pattern})$")).is_ok_and(|re| re.is_match(tool)),
    }
}
