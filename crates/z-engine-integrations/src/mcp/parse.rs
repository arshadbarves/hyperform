//! Decoding of MCP results into typed values. Shapes that break the schema
//! are protocol errors, never empty successes.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use z_engine_protocol::Role;

use super::content::describe;
use super::types::{
    CallToolResult, McpContent, McpPromptInfo, McpResourceInfo, McpToolInfo, PromptArgument,
    PromptMessage, ResourceContents, ServerCapabilities, ServerInfo, ToolAnnotations,
};
use crate::error::IntegrationError;

#[derive(Debug)]
pub(crate) struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Debug)]
pub(crate) struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
    pub instructions: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireTool {
    name: String,
    title: Option<String>,
    description: Option<String>,
    input_schema: Option<Value>,
    annotations: Option<WireAnnotations>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAnnotations {
    title: Option<String>,
    read_only_hint: Option<bool>,
    destructive_hint: Option<bool>,
    idempotent_hint: Option<bool>,
    open_world_hint: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireResource {
    uri: String,
    name: String,
    description: Option<String>,
    mime_type: Option<String>,
}

#[derive(Deserialize)]
struct WirePrompt {
    name: String,
    description: Option<String>,
    #[serde(default)]
    arguments: Vec<WirePromptArgument>,
}

#[derive(Deserialize)]
struct WirePromptArgument {
    name: String,
    description: Option<String>,
    #[serde(default)]
    required: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireContents {
    uri: String,
    mime_type: Option<String>,
    text: Option<String>,
    blob: Option<String>,
}

fn malformed(what: &str, problem: impl std::fmt::Display) -> IntegrationError {
    IntegrationError::Protocol(format!("malformed {what} result: {problem}"))
}

fn items<W: DeserializeOwned>(
    result: &Value,
    key: &str,
    what: &str,
) -> Result<Vec<W>, IntegrationError> {
    let list = result
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| malformed(what, format!("`{key}` is not an array")))?;
    list.iter()
        .map(|item| W::deserialize(item).map_err(|e| malformed(what, e)))
        .collect()
}

fn page<W: DeserializeOwned, T>(
    result: &Value,
    key: &str,
    what: &str,
    convert: impl Fn(W) -> T,
) -> Result<Page<T>, IntegrationError> {
    let items = items::<W>(result, key, what)?
        .into_iter()
        .map(convert)
        .collect();
    let next_cursor = result
        .get("nextCursor")
        .and_then(Value::as_str)
        .filter(|cursor| !cursor.is_empty())
        .map(str::to_string);
    Ok(Page { items, next_cursor })
}

pub(crate) fn initialize_result(result: &Value) -> Result<InitializeResult, IntegrationError> {
    let protocol_version = result
        .get("protocolVersion")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("initialize", "no `protocolVersion`"))?
        .to_string();
    let declared = |name: &str| {
        result["capabilities"]
            .get(name)
            .is_some_and(Value::is_object)
    };
    let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
    Ok(InitializeResult {
        protocol_version,
        capabilities: ServerCapabilities {
            tools: declared("tools"),
            resources: declared("resources"),
            prompts: declared("prompts"),
            logging: declared("logging"),
        },
        server_info: ServerInfo {
            name: text(&result["serverInfo"]["name"]),
            version: text(&result["serverInfo"]["version"]),
        },
        instructions: result["instructions"].as_str().map(str::to_string),
    })
}

pub(crate) fn tools_page(result: &Value) -> Result<Page<McpToolInfo>, IntegrationError> {
    page(result, "tools", "tools/list", |tool: WireTool| {
        let annotations = tool.annotations;
        McpToolInfo {
            name: tool.name,
            title: tool
                .title
                .or_else(|| annotations.as_ref().and_then(|a| a.title.clone())),
            description: tool.description,
            input_schema: tool
                .input_schema
                .unwrap_or_else(|| json!({"type": "object", "properties": {}})),
            annotations: annotations.map_or_else(ToolAnnotations::default, |a| ToolAnnotations {
                read_only_hint: a.read_only_hint,
                destructive_hint: a.destructive_hint,
                idempotent_hint: a.idempotent_hint,
                open_world_hint: a.open_world_hint,
            }),
        }
    })
}

pub(crate) fn resources_page(result: &Value) -> Result<Page<McpResourceInfo>, IntegrationError> {
    page(result, "resources", "resources/list", |r: WireResource| {
        McpResourceInfo {
            uri: r.uri,
            name: r.name,
            description: r.description,
            mime_type: r.mime_type,
        }
    })
}

pub(crate) fn prompts_page(result: &Value) -> Result<Page<McpPromptInfo>, IntegrationError> {
    page(result, "prompts", "prompts/list", |p: WirePrompt| {
        McpPromptInfo {
            name: p.name,
            description: p.description,
            arguments: p
                .arguments
                .into_iter()
                .map(|a| PromptArgument {
                    name: a.name,
                    description: a.description,
                    required: a.required,
                })
                .collect(),
        }
    })
}

pub(crate) fn call_result(result: &Value) -> Result<CallToolResult, IntegrationError> {
    const WHAT: &str = "tools/call";
    let object = result
        .as_object()
        .ok_or_else(|| malformed(WHAT, "the result is not an object"))?;
    let structured = object
        .get("structuredContent")
        .filter(|value| !value.is_null())
        .cloned();
    let content = match object.get("content") {
        Some(Value::Array(list)) => list.iter().map(content_item).collect::<Result<_, _>>()?,
        Some(_) => return Err(malformed(WHAT, "`content` is not an array")),
        None if structured.is_some() => Vec::new(),
        None => return Err(malformed(WHAT, "the result has no `content`")),
    };
    let is_error = match object.get("isError") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => return Err(malformed(WHAT, "`isError` is not a boolean")),
    };
    Ok(CallToolResult {
        content,
        is_error,
        structured,
    })
}

pub(crate) fn content_item(item: &Value) -> Result<McpContent, IntegrationError> {
    let kind = item
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("content", "an item has no `type`"))?;
    let field = |name: &str| {
        item.get(name)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| malformed("content", format!("`{kind}` item without `{name}`")))
    };
    Ok(match kind {
        "text" => McpContent::Text {
            text: field("text")?,
        },
        "image" => McpContent::Image {
            data: field("data")?,
            mime_type: field("mimeType")?,
        },
        "audio" => McpContent::Audio {
            data: field("data")?,
            mime_type: field("mimeType")?,
        },
        "resource_link" => McpContent::ResourceLink {
            uri: field("uri")?,
            name: field("name")?,
        },
        "resource" => {
            let contents = resource_contents_item(item.get("resource").unwrap_or(&Value::Null))?;
            McpContent::Resource {
                uri: contents.uri,
                mime_type: contents.mime_type,
                text: contents.text,
                blob: contents.blob,
            }
        }
        other => McpContent::Text {
            text: format!("[unsupported MCP content type `{other}`]"),
        },
    })
}

fn resource_contents_item(value: &Value) -> Result<ResourceContents, IntegrationError> {
    let wire = WireContents::deserialize(value).map_err(|e| malformed("resource", e))?;
    Ok(ResourceContents {
        uri: wire.uri,
        mime_type: wire.mime_type,
        text: wire.text,
        blob: wire.blob,
    })
}

pub(crate) fn resource_contents(result: &Value) -> Result<Vec<ResourceContents>, IntegrationError> {
    let list = result
        .get("contents")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed("resources/read", "`contents` is not an array"))?;
    list.iter().map(resource_contents_item).collect()
}

pub(crate) fn prompt_messages(result: &Value) -> Result<Vec<PromptMessage>, IntegrationError> {
    const WHAT: &str = "prompts/get";
    let list = result
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed(WHAT, "`messages` is not an array"))?;
    list.iter()
        .map(|message| {
            let role = match message.get("role").and_then(Value::as_str) {
                Some("user") => Role::User,
                Some("assistant") => Role::Assistant,
                other => return Err(malformed(WHAT, format!("unknown role {other:?}"))),
            };
            let content = content_item(message.get("content").unwrap_or(&Value::Null))?;
            Ok(PromptMessage {
                role,
                text: describe(&content),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
