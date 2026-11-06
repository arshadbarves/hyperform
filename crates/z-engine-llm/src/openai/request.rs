//! [`ModelRequest`] -> OpenAI-compatible Chat Completions request body.

use serde_json::{Value, json};
use z_engine_protocol::Effort;

use super::messages::conversation;
use crate::provider::ReasoningStyle;
use crate::types::{ModelRequest, SystemBlock, ToolChoice, ToolSpec};
use crate::wire::{ephemeral, float};

/// Dialect switches resolved from the provider configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChatDialect {
    /// Emit `cache_control` breakpoints (OpenRouter passes them through).
    pub cache_control: bool,
    pub reasoning: ReasoningStyle,
}

pub(crate) fn build_body(request: &ModelRequest, dialect: ChatDialect) -> Value {
    let mut messages = Vec::new();
    if let Some(system) = system_message(&request.system, dialect.cache_control) {
        messages.push(system);
    }
    messages.extend(conversation(request, dialect.cache_control));
    let mut body = json!({
        "model": request.model,
        "messages": messages,
        "stream": true,
        "stream_options": {"include_usage": true},
    });
    let openai_reasoning = dialect.reasoning == ReasoningStyle::OpenAi;
    // OpenAI's reasoning models reject `max_tokens` and custom temperatures.
    let max_tokens_key = if openai_reasoning {
        "max_completion_tokens"
    } else {
        "max_tokens"
    };
    body[max_tokens_key] = json!(request.max_tokens);
    if !request.tools.is_empty() {
        body["tools"] = request.tools.iter().map(tool_definition).collect();
        body["tool_choice"] = tool_choice(&request.tool_choice);
    }
    match (request.thinking, dialect.reasoning) {
        (Some(thinking), ReasoningStyle::OpenRouter) => {
            body["reasoning"] = json!({"effort": effort_label(thinking.effort)});
        }
        (Some(thinking), ReasoningStyle::OpenAi) => {
            body["reasoning_effort"] = json!(effort_label(thinking.effort));
        }
        _ => {}
    }
    let reasoning_sent = request.thinking.is_some() && openai_reasoning;
    if let Some(temperature) = request.temperature.filter(|_| !reasoning_sent) {
        body["temperature"] = float(temperature);
    }
    if !request.stop_sequences.is_empty() {
        body["stop"] = json!(request.stop_sequences);
    }
    body
}

/// One system message: content parts when a cached block needs a
/// breakpoint, otherwise the blocks joined into one string.
fn system_message(system: &[SystemBlock], cache_control: bool) -> Option<Value> {
    let blocks: Vec<&SystemBlock> = system
        .iter()
        .filter(|block| !block.text.is_empty())
        .collect();
    if blocks.is_empty() {
        return None;
    }
    if cache_control && blocks.iter().any(|block| block.cache) {
        let parts: Vec<Value> = blocks
            .iter()
            .map(|block| {
                let mut part = json!({"type": "text", "text": block.text});
                if block.cache {
                    part["cache_control"] = ephemeral();
                }
                part
            })
            .collect();
        return Some(json!({"role": "system", "content": parts}));
    }
    let text = blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    Some(json!({"role": "system", "content": text}))
}

fn tool_definition(tool: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
        },
    })
}

fn tool_choice(choice: &ToolChoice) -> Value {
    match choice {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::None => json!("none"),
        ToolChoice::Any => json!("required"),
        ToolChoice::Tool(name) => json!({"type": "function", "function": {"name": name}}),
    }
}

pub(crate) fn effort_label(effort: Effort) -> &'static str {
    match effort {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High | Effort::Max => "high",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ThinkingConfig;
    use z_engine_protocol::Message;

    const OPENROUTER: ChatDialect = ChatDialect {
        cache_control: true,
        reasoning: ReasoningStyle::OpenRouter,
    };
    const PLAIN: ChatDialect = ChatDialect {
        cache_control: false,
        reasoning: ReasoningStyle::None,
    };

    fn request() -> ModelRequest {
        ModelRequest::new("m", vec![Message::user_text("hi")])
    }

    fn tool(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            description: format!("{name} tool"),
            input_schema: json!({"type": "object", "properties": {}}),
        }
    }

    fn thinking(effort: Effort) -> Option<ThinkingConfig> {
        Some(ThinkingConfig {
            effort,
            budget_tokens: None,
        })
    }

    #[test]
    fn minimal_body_streams_with_usage_and_no_optional_fields() {
        let body = build_body(&request(), PLAIN);
        assert_eq!(body["model"], "m");
        assert_eq!(body["stream"], true);
        assert_eq!(body["stream_options"], json!({"include_usage": true}));
        assert_eq!(body["max_tokens"], 4_096);
        assert_eq!(body["messages"], json!([{"role": "user", "content": "hi"}]));
        for absent in [
            "tools",
            "tool_choice",
            "temperature",
            "stop",
            "reasoning",
            "reasoning_effort",
        ] {
            assert!(body.get(absent).is_none(), "{absent}");
        }
    }

    #[test]
    fn system_blocks_join_unless_a_cached_block_needs_a_breakpoint() {
        let system = vec![SystemBlock::cached("stable"), SystemBlock::new("dynamic")];
        let req = request().with_system(system);
        let joined = build_body(&req, PLAIN);
        assert_eq!(
            joined["messages"][0],
            json!({"role": "system", "content": "stable\n\ndynamic"})
        );
        let parts = build_body(&req, OPENROUTER);
        assert_eq!(
            parts["messages"][0]["content"],
            json!([
                {"type": "text", "text": "stable", "cache_control": {"type": "ephemeral"}},
                {"type": "text", "text": "dynamic"}
            ])
        );
        let uncached = request().with_system(vec![SystemBlock::new("a"), SystemBlock::new("")]);
        assert_eq!(
            build_body(&uncached, OPENROUTER)["messages"][0]["content"],
            "a"
        );
    }

    #[test]
    fn tools_and_every_tool_choice_map() {
        let mut req = request().with_tools(vec![tool("Read")]);
        let body = build_body(&req, PLAIN);
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "Read");
        assert_eq!(body["tools"][0]["function"]["parameters"]["type"], "object");
        let cases = [
            (ToolChoice::Auto, json!("auto")),
            (ToolChoice::None, json!("none")),
            (ToolChoice::Any, json!("required")),
            (
                ToolChoice::Tool("Read".into()),
                json!({"type": "function", "function": {"name": "Read"}}),
            ),
        ];
        for (choice, expected) in cases {
            req.tool_choice = choice;
            assert_eq!(build_body(&req, PLAIN)["tool_choice"], expected);
        }
        let mut no_tools = request();
        no_tools.tool_choice = ToolChoice::Any;
        assert!(build_body(&no_tools, PLAIN).get("tool_choice").is_none());
    }

    #[test]
    fn reasoning_follows_the_dialect() {
        let mut req = request();
        req.temperature = Some(0.2);
        req.thinking = thinking(Effort::Max);
        let openrouter = build_body(&req, OPENROUTER);
        assert_eq!(openrouter["reasoning"], json!({"effort": "high"}));
        assert_eq!(openrouter["temperature"], json!(0.2));
        let openai = ChatDialect {
            cache_control: false,
            reasoning: ReasoningStyle::OpenAi,
        };
        let body = build_body(&req, openai);
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["max_completion_tokens"], 4_096);
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("temperature").is_none());
        let none = build_body(&req, PLAIN);
        assert!(none.get("reasoning").is_none() && none.get("reasoning_effort").is_none());
        req.thinking = None;
        assert!(build_body(&req, OPENROUTER).get("reasoning").is_none());
        assert_eq!(build_body(&req, openai)["temperature"], json!(0.2));
    }

    #[test]
    fn efforts_and_stop_sequences_map() {
        let mut req = request();
        req.stop_sequences = vec!["END".into()];
        for (effort, label) in [
            (Effort::Low, "low"),
            (Effort::Medium, "medium"),
            (Effort::High, "high"),
        ] {
            req.thinking = thinking(effort);
            let body = build_body(&req, OPENROUTER);
            assert_eq!(body["reasoning"]["effort"], label);
            assert_eq!(body["stop"], json!(["END"]));
        }
    }
}
