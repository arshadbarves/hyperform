use serde_json::json;
use z_engine_protocol::Message;

use super::*;
use crate::types::ToolSpec;

fn request() -> ModelRequest {
    ModelRequest::new("claude-test", vec![Message::user_text("hi")])
}

fn tool(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: format!("{name} tool"),
        input_schema: json!({"type": "object", "properties": {}}),
    }
}

fn thinking(effort: Effort, budget_tokens: Option<u32>) -> Option<ThinkingConfig> {
    Some(ThinkingConfig {
        effort,
        budget_tokens,
    })
}

fn cache_marks(value: &Value) -> usize {
    match value {
        Value::Object(map) => {
            usize::from(map.contains_key("cache_control"))
                + map.values().map(cache_marks).sum::<usize>()
        }
        Value::Array(items) => items.iter().map(cache_marks).sum(),
        _ => 0,
    }
}

#[test]
fn base_fields_and_optional_fields() {
    let body = build_body(&request(), true);
    assert_eq!(body["model"], "claude-test");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 4_096);
    assert_eq!(
        body["messages"],
        json!([{"role": "user", "content": [{"type": "text", "text": "hi"}]}])
    );
    for absent in [
        "system",
        "tools",
        "tool_choice",
        "thinking",
        "temperature",
        "stop_sequences",
    ] {
        assert!(body.get(absent).is_none(), "{absent}");
    }
    let mut req = request();
    req.stop_sequences = vec!["END".into()];
    req.temperature = Some(0.3);
    let body = build_body(&req, true);
    assert_eq!(body["stop_sequences"], json!(["END"]));
    assert_eq!(body["temperature"], json!(0.3));
}

#[test]
fn system_blocks_become_text_blocks_with_cache_control() {
    let req = request().with_system(vec![
        SystemBlock::cached("stable"),
        SystemBlock::new("  "),
        SystemBlock::new("dynamic"),
    ]);
    assert_eq!(
        build_body(&req, true)["system"],
        json!([
            {"type": "text", "text": "stable", "cache_control": {"type": "ephemeral"}},
            {"type": "text", "text": "dynamic"}
        ])
    );
    assert_eq!(cache_marks(&build_body(&req, false)), 0);
    let blank = request().with_system(vec![SystemBlock::new("")]);
    assert!(build_body(&blank, true).get("system").is_none());
}

#[test]
fn tools_cache_only_the_last_definition() {
    let mut req = request().with_tools(vec![tool("Read"), tool("Bash")]);
    req.cache_tools = true;
    let body = build_body(&req, true);
    assert_eq!(
        body["tools"][0],
        json!({"name": "Read", "description": "Read tool", "input_schema": {"type": "object", "properties": {}}})
    );
    assert_eq!(
        body["tools"][1]["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert_eq!(cache_marks(&body), 1);
    assert_eq!(cache_marks(&build_body(&req, false)), 0);
}

#[test]
fn tool_choice_maps_only_with_tools() {
    let mut req = request().with_tools(vec![tool("Read")]);
    let cases = [
        (ToolChoice::Auto, json!({"type": "auto"})),
        (ToolChoice::None, json!({"type": "none"})),
        (ToolChoice::Any, json!({"type": "any"})),
        (
            ToolChoice::Tool("Read".into()),
            json!({"type": "tool", "name": "Read"}),
        ),
    ];
    for (choice, expected) in cases {
        req.tool_choice = choice;
        assert_eq!(build_body(&req, true)["tool_choice"], expected);
    }
    let mut bare = request();
    bare.tool_choice = ToolChoice::Any;
    assert!(build_body(&bare, true).get("tool_choice").is_none());
}

#[test]
fn thinking_budget_follows_effort_and_clamps() {
    let config = |effort, budget| thinking(effort, budget).unwrap();
    assert_eq!(
        thinking_budget(config(Effort::High, None), 32_000),
        (16_384, 32_000)
    );
    assert_eq!(
        thinking_budget(config(Effort::Max, None), 64_000),
        (32_000, 64_000)
    );
    assert_eq!(
        thinking_budget(config(Effort::High, None), 8_192),
        (7_168, 8_192)
    );
    assert_eq!(
        thinking_budget(config(Effort::Low, None), 1_500),
        (1_024, 5_120)
    );
    assert_eq!(
        thinking_budget(config(Effort::Low, None), 2_048),
        (1_024, 2_048)
    );
    assert_eq!(
        thinking_budget(config(Effort::Low, Some(5_000)), 10_000),
        (5_000, 10_000)
    );
    assert_eq!(
        thinking_budget(config(Effort::Max, Some(10)), 10_000),
        (1_024, 10_000)
    );
}

#[test]
fn thinking_replaces_temperature_and_raises_small_max_tokens() {
    let mut req = request().with_max_tokens(1_500);
    req.temperature = Some(0.5);
    req.thinking = thinking(Effort::Medium, None);
    let body = build_body(&req, true);
    assert_eq!(
        body["thinking"],
        json!({"type": "enabled", "budget_tokens": 1_024})
    );
    assert_eq!(body["max_tokens"], 5_120);
    assert!(body.get("temperature").is_none());
}

#[test]
fn forced_tool_choice_disables_thinking() {
    let mut req = request().with_tools(vec![tool("Read")]);
    req.thinking = thinking(Effort::High, None);
    req.tool_choice = ToolChoice::Tool("Read".into());
    assert!(!thinking_enabled(&req));
    assert!(build_body(&req, true).get("thinking").is_none());
    req.tool_choice = ToolChoice::Auto;
    assert!(thinking_enabled(&req));
    assert!(build_body(&req, true).get("thinking").is_some());
}

#[test]
fn breakpoints_are_capped_at_four_dropping_earliest_messages() {
    let messages = (0..4)
        .map(|i| Message::user_text(format!("m{i}")))
        .collect();
    let mut req = ModelRequest::new("claude-test", messages)
        .with_system(vec![SystemBlock::cached("a"), SystemBlock::cached("b")])
        .with_tools(vec![tool("Read")]);
    req.cache_tools = true;
    req.messages.insert(1, Message::assistant_text("between"));
    req.messages.insert(3, Message::assistant_text("between"));
    req.cache_breakpoints = vec![0, 2, 4];
    let body = build_body(&req, true);
    assert_eq!(cache_marks(&body), 4);
    let marked: Vec<usize> = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, message)| cache_marks(message) > 0)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(marked, [4]);
}

#[test]
fn excess_system_breakpoints_keep_the_latest() {
    let system = (0..6)
        .map(|i| SystemBlock::cached(format!("s{i}")))
        .collect();
    let mut req = request().with_system(system);
    req.cache_breakpoints = vec![0];
    let body = build_body(&req, true);
    assert_eq!(cache_marks(&body), 4);
    assert!(body["system"][1].get("cache_control").is_none());
    assert!(body["system"][2].get("cache_control").is_some());
    assert_eq!(cache_marks(&body["messages"]), 0);
}
