//! Copilot's request quirks, applied around the shared adapters.

use serde_json::{Value, json};
use z_engine_prompts::providers::COPILOT_NOOP_TOOL;
use z_engine_protocol::ContentBlock;

use super::models::{Resolved, Route};
use crate::openai::{ChatDialect, effort_label};
use crate::provider::ReasoningStyle;
use crate::types::{ModelRequest, ThinkingConfig, ToolSpec};
use crate::{anthropic, openai, responses};

/// Copilot rejects a history with tool calls when the request declares no
/// tools, so such a request gets this one.
const NOOP_TOOL: &str = "_noop";

/// Adjust `request` before its body and headers are built: declare the
/// no-op tool when needed, and drop a reasoning effort the model does not
/// list (Messages models take a thinking budget instead).
pub(super) fn prepare(mut request: ModelRequest, resolved: &Resolved) -> ModelRequest {
    if request.tools.is_empty() && has_tool_history(&request) {
        request.tools.push(ToolSpec {
            name: NOOP_TOOL.into(),
            description: COPILOT_NOOP_TOOL.trim().into(),
            input_schema: json!({
                "type": "object",
                "properties": {"reason": {"type": "string", "description": "Unused"}},
            }),
        });
    }
    let unsupported = request
        .thinking
        .is_some_and(|thinking| !supports_effort(resolved, thinking));
    if resolved.route != Route::Messages && unsupported {
        request.thinking = None;
    }
    request
}

/// The wire body for a [`prepare`]d request on its route.
pub(super) fn build(request: &ModelRequest, resolved: &Resolved, cache_control: bool) -> Value {
    let mut body = match resolved.route {
        Route::Chat => chat(request, resolved),
        Route::Responses => responses::build_body(request),
        Route::Messages => return anthropic::build_body(request, cache_control),
    };
    // GitHub's own Copilot CLI sends GPT models no output cap.
    if request.model.contains("gpt") {
        if let Some(fields) = body.as_object_mut() {
            for key in ["max_tokens", "max_completion_tokens", "max_output_tokens"] {
                fields.remove(key);
            }
        }
    }
    body
}

fn chat(request: &ModelRequest, resolved: &Resolved) -> Value {
    let dialect = ChatDialect {
        cache_control: false,
        reasoning: ReasoningStyle::None,
    };
    let mut body = openai::build_body(request, dialect);
    if let (Some(thinking), Some(_)) = (request.thinking, &resolved.efforts) {
        body["reasoning_effort"] = json!(effort_label(thinking.effort));
        if let Some(fields) = body.as_object_mut() {
            fields.remove("temperature");
        }
    }
    body
}

/// Unknown efforts (a model routed by name) are assumed supported.
fn supports_effort(resolved: &Resolved, thinking: ThinkingConfig) -> bool {
    let label = effort_label(thinking.effort);
    resolved
        .efforts
        .as_ref()
        .is_none_or(|efforts| efforts.iter().any(|effort| effort == label))
}

fn has_tool_history(request: &ModelRequest) -> bool {
    request
        .messages
        .iter()
        .flat_map(|message| &message.content)
        .any(|block| {
            matches!(
                block,
                ContentBlock::ToolUse { .. } | ContentBlock::ToolResult { .. }
            )
        })
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::{CallId, Effort, Message, Role};

    use super::*;

    fn resolved(route: Route, efforts: Option<&[&str]>) -> Resolved {
        Resolved {
            route,
            efforts: efforts.map(|efforts| efforts.iter().map(|e| (*e).to_string()).collect()),
        }
    }

    fn thinking(model: &str, effort: Effort) -> ModelRequest {
        let mut request = ModelRequest::new(model, vec![Message::user_text("hi")]);
        request.thinking = Some(ThinkingConfig {
            effort,
            budget_tokens: None,
        });
        request.temperature = Some(0.3);
        request
    }

    fn body(request: ModelRequest, resolved: &Resolved) -> Value {
        build(&prepare(request, resolved), resolved, true)
    }

    #[test]
    fn gpt_models_send_no_output_cap() {
        let request = ModelRequest::new("gpt-4.1", vec![Message::user_text("hi")]);
        let chat = body(request.clone(), &resolved(Route::Chat, Some(&[])));
        assert!(chat.get("max_tokens").is_none());
        let responses = body(request, &resolved(Route::Responses, None));
        assert!(responses.get("max_output_tokens").is_none());
        let other = ModelRequest::new("gemini-2.5-pro", vec![Message::user_text("hi")]);
        assert_eq!(
            body(other, &resolved(Route::Chat, None))["max_tokens"],
            4_096
        );
        let claude = ModelRequest::new("claude-sonnet-4.5", vec![Message::user_text("hi")]);
        let messages = body(claude, &resolved(Route::Messages, None));
        assert_eq!(messages["max_tokens"], 4_096);
    }

    #[test]
    fn chat_sends_an_effort_only_when_the_model_lists_it() {
        let listed = body(
            thinking("gpt-5-mini", Effort::High),
            &resolved(Route::Chat, Some(&["low", "high"])),
        );
        assert_eq!(listed["reasoning_effort"], "high");
        assert!(listed.get("temperature").is_none());
        let unlisted = resolved(Route::Chat, Some(&["low"]));
        let plain = body(thinking("gpt-5-mini", Effort::High), &unlisted);
        assert!(plain.get("reasoning_effort").is_none());
        assert_eq!(plain["temperature"], json!(0.3));
        let unknown = body(
            thinking("gpt-4o", Effort::Low),
            &resolved(Route::Chat, None),
        );
        assert!(unknown.get("reasoning_effort").is_none() && unknown.get("reasoning").is_none());
    }

    #[test]
    fn responses_reasoning_follows_the_listed_efforts() {
        let listed = resolved(Route::Responses, Some(&["medium"]));
        let body_listed = body(thinking("gpt-5", Effort::Medium), &listed);
        assert_eq!(body_listed["reasoning"]["effort"], "medium");
        let none = resolved(Route::Responses, Some(&[]));
        assert!(
            body(thinking("gpt-5", Effort::Medium), &none)
                .get("reasoning")
                .is_none()
        );
        let unknown = resolved(Route::Responses, None);
        assert_eq!(
            body(thinking("gpt-6", Effort::Low), &unknown)["reasoning"]["effort"],
            "low"
        );
        let messages = resolved(Route::Messages, Some(&[]));
        let claude = prepare(thinking("claude-sonnet-4.5", Effort::Low), &messages);
        assert!(claude.thinking.is_some());
    }

    #[test]
    fn a_tool_history_without_tools_declares_the_noop_tool() {
        let history = vec![
            Message::user_text("task"),
            Message::new(
                Role::User,
                vec![ContentBlock::tool_result(CallId::from("c1"), "ok", false)],
            ),
        ];
        let chat = resolved(Route::Chat, None);
        let summary = prepare(ModelRequest::new("gpt-4.1", history.clone()), &chat);
        assert_eq!(summary.tools.len(), 1);
        assert_eq!(summary.tools[0].name, NOOP_TOOL);
        let plain = prepare(
            ModelRequest::new("gpt-4.1", vec![Message::user_text("hi")]),
            &chat,
        );
        assert!(plain.tools.is_empty());
        let mut tooled = ModelRequest::new("gpt-4.1", history);
        tooled.tools = summary.tools.clone();
        tooled.tools[0].name = "Read".into();
        assert_eq!(prepare(tooled, &chat).tools.len(), 1);
    }
}
