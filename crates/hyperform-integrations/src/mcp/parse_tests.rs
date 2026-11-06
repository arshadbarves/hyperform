use serde_json::json;

use super::*;

#[test]
fn initialize_reads_version_capabilities_and_info() {
    let init = initialize_result(&json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {"tools": {"listChanged": true}, "logging": {}},
        "serverInfo": {"name": "fake", "version": "1.0"},
        "instructions": "Use carefully."
    }))
    .unwrap();
    assert_eq!(init.protocol_version, "2025-06-18");
    assert!(init.capabilities.tools && init.capabilities.logging);
    assert!(!init.capabilities.resources && !init.capabilities.prompts);
    assert_eq!(init.server_info.name, "fake");
    assert_eq!(init.instructions.as_deref(), Some("Use carefully."));
    assert!(initialize_result(&json!({"capabilities": {}})).is_err());
}

#[test]
fn tools_page_reads_titles_annotations_and_cursor() {
    let page = tools_page(&json!({
        "tools": [
            {"name": "a", "inputSchema": {"type": "object"}, "annotations": {"title": "Alpha", "readOnlyHint": true}},
            {"name": "b", "title": "Beta", "description": "does b"}
        ],
        "nextCursor": "next"
    }))
    .unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some("next"));
    assert_eq!(page.items[0].title.as_deref(), Some("Alpha"));
    assert_eq!(page.items[0].annotations.read_only_hint, Some(true));
    assert_eq!(page.items[1].title.as_deref(), Some("Beta"));
    assert_eq!(page.items[1].input_schema["type"], "object");
    assert!(tools_page(&json!({"tools": [{"description": "no name"}]})).is_err());
    assert!(tools_page(&json!({})).is_err());
    let last = tools_page(&json!({"tools": [], "nextCursor": ""})).unwrap();
    assert!(last.next_cursor.is_none());
}

#[test]
fn call_results_decode_every_content_kind() {
    let result = call_result(&json!({
        "content": [
            {"type": "text", "text": "hi"},
            {"type": "image", "data": "aGk=", "mimeType": "image/png"},
            {"type": "audio", "data": "aGk=", "mimeType": "audio/wav"},
            {"type": "resource_link", "uri": "file:///a", "name": "a"},
            {"type": "resource", "resource": {"uri": "file:///b", "mimeType": "text/plain", "text": "B"}},
            {"type": "hologram"}
        ],
        "isError": true,
        "structuredContent": {"n": 1}
    }))
    .unwrap();
    assert!(result.is_error);
    assert_eq!(result.structured, Some(json!({"n": 1})));
    assert_eq!(result.content.len(), 6);
    assert!(matches!(&result.content[4], McpContent::Resource { text: Some(t), .. } if t == "B"));
    assert!(matches!(&result.content[5], McpContent::Text { text } if text.contains("hologram")));
}

#[test]
fn malformed_call_results_are_protocol_errors() {
    for bad in [
        json!(null),
        json!({"content": "oops"}),
        json!({}),
        json!({"content": [{"type": "text"}]}),
        json!({"content": [], "isError": "yes"}),
        json!({"content": [{"type": "resource"}]}),
    ] {
        assert!(
            matches!(call_result(&bad), Err(IntegrationError::Protocol(_))),
            "{bad} should be rejected"
        );
    }
    let structured_only = call_result(&json!({"structuredContent": {"ok": true}})).unwrap();
    assert!(structured_only.content.is_empty() && !structured_only.is_error);
}

#[test]
fn resources_and_prompts_decode() {
    let resources = resources_page(&json!({
        "resources": [{"uri": "file:///r", "name": "r", "mimeType": "text/plain"}]
    }))
    .unwrap();
    assert_eq!(resources.items[0].mime_type.as_deref(), Some("text/plain"));
    let contents =
        resource_contents(&json!({"contents": [{"uri": "file:///r", "blob": "AA=="}]})).unwrap();
    assert_eq!(contents[0].blob.as_deref(), Some("AA=="));
    let prompts = prompts_page(&json!({
        "prompts": [{"name": "review", "arguments": [{"name": "file", "required": true}]}]
    }))
    .unwrap();
    assert!(prompts.items[0].arguments[0].required);
    let messages = prompt_messages(&json!({
        "messages": [
            {"role": "user", "content": {"type": "text", "text": "Review a.rs"}},
            {"role": "assistant", "content": {"type": "resource_link", "uri": "file:///a.rs", "name": "a.rs"}}
        ]
    }))
    .unwrap();
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[0].text, "Review a.rs");
    assert!(messages[1].text.contains("file:///a.rs"));
    assert!(prompt_messages(&json!({"messages": [{"role": "system", "content": {}}]})).is_err());
}
