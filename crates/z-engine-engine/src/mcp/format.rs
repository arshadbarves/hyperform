//! MCP results as the model reads them: tool content as result parts
//! (structured content when a tool returns nothing else), resource
//! listings as lines, resource contents as text or images.

use z_engine_integrations::{CallToolResult, McpResourceInfo, ResourceContents, content_to_parts};
use z_engine_protocol::{MediaSource, ToolResultPart};

pub(crate) fn call_parts(result: &CallToolResult) -> Vec<ToolResultPart> {
    let parts = content_to_parts(&result.content);
    match (&result.structured, parts.is_empty()) {
        (Some(structured), true) => vec![text(
            serde_json::to_string_pretty(structured).unwrap_or_else(|_| structured.to_string()),
        )],
        _ => parts,
    }
}

/// `server: uri (name) [mime] - description`, one line per resource.
pub(crate) fn resource_listing(resources: &[(String, McpResourceInfo)]) -> String {
    resources
        .iter()
        .map(|(server, resource)| {
            let mut line = format!("{server}: {} ({})", resource.uri, resource.name);
            if let Some(mime) = &resource.mime_type {
                line.push_str(&format!(" [{mime}]"));
            }
            if let Some(description) = resource.description.as_deref().map(str::trim) {
                if !description.is_empty() {
                    line.push_str(&format!(" - {}", description.replace('\n', " ")));
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn resource_parts(contents: &[ResourceContents]) -> Vec<ToolResultPart> {
    let labelled = contents.len() > 1;
    contents
        .iter()
        .map(|content| {
            let mime = content.mime_type.as_deref();
            match (&content.text, &content.blob) {
                (Some(body), _) if labelled => text(format!("[{}]\n{body}", content.uri)),
                (Some(body), _) => text(body.clone()),
                (None, Some(data)) if mime.is_some_and(|mime| mime.starts_with("image/")) => {
                    ToolResultPart::Image {
                        source: MediaSource::Base64 {
                            media_type: mime.unwrap_or_default().to_string(),
                            data: data.clone(),
                        },
                    }
                }
                (None, Some(data)) => text(format!(
                    "[resource {}: {}, {} bytes of binary data]",
                    content.uri,
                    mime.unwrap_or("application/octet-stream"),
                    data.trim_end_matches('=').len() * 3 / 4
                )),
                (None, None) => text(format!("[resource {}: empty]", content.uri)),
            }
        })
        .collect()
}

fn text(text: String) -> ToolResultPart {
    ToolResultPart::Text { text }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_integrations::McpContent;

    use super::*;

    fn text_of(part: &ToolResultPart) -> &str {
        match part {
            ToolResultPart::Text { text } => text,
            ToolResultPart::Image { .. } => panic!("text expected"),
        }
    }

    #[test]
    fn structured_content_fills_an_empty_result() {
        let result = CallToolResult {
            content: Vec::new(),
            is_error: false,
            structured: Some(json!({"ok": true})),
        };
        assert!(text_of(&call_parts(&result)[0]).contains("\"ok\": true"));
        let plain = CallToolResult {
            content: vec![McpContent::Text { text: "hi".into() }],
            ..result
        };
        assert_eq!(text_of(&call_parts(&plain)[0]), "hi");
    }

    #[test]
    fn resources_list_and_read_as_text_or_images() {
        let listed = resource_listing(&[(
            "docs".into(),
            McpResourceInfo {
                uri: "fake://readme".into(),
                name: "readme".into(),
                description: Some("The readme".into()),
                mime_type: Some("text/markdown".into()),
            },
        )]);
        assert_eq!(
            listed,
            "docs: fake://readme (readme) [text/markdown] - The readme"
        );
        let content = |text: Option<&str>, blob: Option<&str>, mime: &str| ResourceContents {
            uri: "u".into(),
            mime_type: Some(mime.into()),
            text: text.map(str::to_string),
            blob: blob.map(str::to_string),
        };
        let single = resource_parts(&[content(Some("# Hi"), None, "text/markdown")]);
        assert_eq!(text_of(&single[0]), "# Hi");
        let parts = resource_parts(&[
            content(Some("a"), None, "text/plain"),
            content(None, Some("iVBO"), "image/png"),
            content(None, Some("AAAA"), "application/zip"),
        ]);
        assert_eq!(text_of(&parts[0]), "[u]\na");
        assert!(matches!(parts[1], ToolResultPart::Image { .. }));
        assert!(text_of(&parts[2]).contains("3 bytes of binary data"));
    }
}
