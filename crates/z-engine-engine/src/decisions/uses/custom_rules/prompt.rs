//! `UserPromptSubmit` rules, asked about the message that opens a turn. The
//! message always reaches the agent: `ask` tells the agent to ask the user
//! before it changes anything, `notice` posts a notice, and `remind` adds a
//! note for the agent.

use serde_json::json;
use z_engine_context::render_template;
use z_engine_prompts::decisions::CUSTOM_RULE_PROMPT;
use z_engine_prompts::reminders::{CUSTOM_RULE_ASK, CUSTOM_RULE_REMIND};
use z_engine_protocol::NoticeLevel;

use super::judge::{Fired, fired};
use super::tool::describe;
use crate::decisions::context::UseContext;
use crate::decisions::uses::digest::head;

const USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
const MESSAGE_CHARS: usize = 1_500;

pub(super) async fn at_prompt(cx: &UseContext, text: &str) -> Vec<String> {
    let settings = cx.core.settings();
    let rules: Vec<_> = settings
        .settings
        .decisions
        .rules
        .iter()
        .filter(|rule| rule.event == USER_PROMPT_SUBMIT)
        .cloned()
        .collect();
    if rules.is_empty() {
        return Vec::new();
    }
    let state = json!({ "message": head(text, MESSAGE_CHARS) });
    let outcome = |rule: &z_engine_config::DecisionRule| match rule.action.as_str() {
        "ask" => "asked",
        "notice" => "noticed",
        _ => "reminded",
    };
    let fired = fired(cx, rules, state, CUSTOM_RULE_PROMPT, outcome).await;
    let mut notes = Vec::new();
    for Fired { rule, answer } in &fired {
        let fields = [
            ("question", rule.question.as_str()),
            ("answer", answer.as_str()),
        ];
        match rule.action.as_str() {
            "ask" => notes.push(render_template(CUSTOM_RULE_ASK, &fields)),
            "remind" => notes.push(render_template(CUSTOM_RULE_REMIND, &fields)),
            _ if !cx.shadow => {
                let text = format!("Decision rule on your message: {}", describe(rule, answer));
                cx.core.events.notice(NoticeLevel::Info, text);
            }
            _ => {}
        }
    }
    notes
}
