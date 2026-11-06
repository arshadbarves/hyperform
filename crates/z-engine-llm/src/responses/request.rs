//! [`ModelRequest`] -> OpenAI Responses request body. Nothing is stored
//! server side, so encrypted reasoning travels with each request.

use serde_json::{Value, json};

use super::messages::input;
use crate::openai::effort_label;
use crate::types::{ModelRequest, ToolChoice, ToolSpec};
use crate::wire::float;

pub(crate) fn build_body(request: &ModelRequest) -> Value {
    let mut body = json!({
        "model": request.model,
        "input": input(&request.messages),
        "stream": true,
        "store": false,
        "include": ["reasoning.encrypted_content"],
        "max_output_tokens": request.max_tokens,
    });
    let instructions: Vec<&str> = request
        .system
        .iter()
        .map(|block| block.text.as_str())
        .filter(|text| !text.trim().is_empty())
        .collect();
    if !instructions.is_empty() {
        body["instructions"] = json!(instructions.join("\n\n"));
    }
    if !request.tools.is_empty() {
        body["tools"] = request.tools.iter().map(tool_definition).collect();
        body["tool_choice"] = tool_choice(&request.tool_choice);
    }
    match request.thinking {
        Some(thinking) => {
            body["reasoning"] = json!({"effort": effort_label(thinking.effort), "summary": "auto"});
        }
        // Reasoning models reject custom temperatures.
        None => {
            if let Some(temperature) = request.temperature {
                body["temperature"] = float(temperature);
            }
        }
    }
    body
}

/// `strict` defaults to on in this API, which our schemas do not satisfy.
fn tool_definition(tool: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "parameters": tool.input_schema,
        "strict": false,
    })
}

fn tool_choice(choice: &ToolChoice) -> Value {
    match choice {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::None => json!("none"),
        ToolChoice::Any => json!("required"),
        ToolChoice::Tool(name) => json!({"type": "function", "name": name}),
    }
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::{Effort, Message};

    use super::*;
    use crate::types::{SystemBlock, ThinkingConfig};

    fn request() -> ModelRequest {
        ModelRequest::new("gpt-5", vec![Message::user_text("hi")])
    }

    #[test]
    fn minimal_body_is_stateless_and_streams() {
        let body = build_body(&request());
        assert_eq!(body["model"], "gpt-5");
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);
        assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
        assert_eq!(body["max_output_tokens"], 4_096);
        assert_eq!(
            body["input"],
            json!([{"role": "user", "content": [{"type": "input_text", "text": "hi"}]}])
        );
        for absent in [
            "instructions",
            "tools",
            "tool_choice",
            "reasoning",
            "temperature",
        ] {
            assert!(body.get(absent).is_none(), "{absent}");
        }
    }

    #[test]
    fn system_blocks_become_instructions() {
        let req = request().with_system(vec![
            SystemBlock::cached("stable"),
            SystemBlock::new(" "),
            SystemBlock::new("dynamic"),
        ]);
        assert_eq!(build_body(&req)["instructions"], "stable\n\ndynamic");
    }

    #[test]
    fn tools_and_every_tool_choice_map() {
        let mut req = request().with_tools(vec![ToolSpec {
            name: "Read".into(),
            description: "Read a file".into(),
            input_schema: json!({"type": "object", "properties": {}}),
        }]);
        assert_eq!(
            build_body(&req)["tools"],
            json!([{
                "type": "function",
                "name": "Read",
                "description": "Read a file",
                "parameters": {"type": "object", "properties": {}},
                "strict": false,
            }])
        );
        let cases = [
            (ToolChoice::Auto, json!("auto")),
            (ToolChoice::None, json!("none")),
            (ToolChoice::Any, json!("required")),
            (
                ToolChoice::Tool("Read".into()),
                json!({"type": "function", "name": "Read"}),
            ),
        ];
        for (choice, expected) in cases {
            req.tool_choice = choice;
            assert_eq!(build_body(&req)["tool_choice"], expected);
        }
    }

    #[test]
    fn reasoning_replaces_the_temperature() {
        let mut req = request();
        req.temperature = Some(0.2);
        assert_eq!(build_body(&req)["temperature"], json!(0.2));
        req.thinking = Some(ThinkingConfig {
            effort: Effort::Max,
            budget_tokens: None,
        });
        let body = build_body(&req);
        assert_eq!(
            body["reasoning"],
            json!({"effort": "high", "summary": "auto"})
        );
        assert!(body.get("temperature").is_none());
    }
}
