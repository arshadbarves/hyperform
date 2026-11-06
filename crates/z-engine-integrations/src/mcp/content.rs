//! MCP content as tool-result parts the model can read: text as is, images
//! as base64 media, everything else described in text lines.

use z_engine_protocol::{MediaSource, ToolResultPart};

use super::types::McpContent;

/// One part per content item, in order.
pub fn content_to_parts(content: &[McpContent]) -> Vec<ToolResultPart> {
    content
        .iter()
        .map(|item| match item {
            McpContent::Image { data, mime_type } => image(data, mime_type),
            McpContent::Resource {
                blob: Some(data),
                mime_type: Some(mime_type),
                ..
            } if mime_type.starts_with("image/") => image(data, mime_type),
            other => ToolResultPart::Text {
                text: describe(other),
            },
        })
        .collect()
}

/// A text rendering of one content item.
pub(crate) fn describe(item: &McpContent) -> String {
    match item {
        McpContent::Text { text } => text.clone(),
        McpContent::Image { data, mime_type } => {
            format!("[image {mime_type}, {} bytes]", decoded_len(data))
        }
        McpContent::Audio { data, mime_type } => {
            format!("[audio {mime_type}, {} bytes]", decoded_len(data))
        }
        McpContent::ResourceLink { uri, name } => format!("[resource link] {name}: {uri}"),
        McpContent::Resource {
            uri,
            text: Some(text),
            ..
        } => format!("[resource {uri}]\n{text}"),
        McpContent::Resource {
            uri,
            mime_type,
            blob: Some(blob),
            ..
        } => format!(
            "[resource {uri}: {}, {} bytes of binary data]",
            mime_type.as_deref().unwrap_or("application/octet-stream"),
            decoded_len(blob)
        ),
        McpContent::Resource { uri, .. } => format!("[resource {uri}: empty]"),
    }
}

fn image(data: &str, mime_type: &str) -> ToolResultPart {
    ToolResultPart::Image {
        source: MediaSource::Base64 {
            media_type: mime_type.to_string(),
            data: data.to_string(),
        },
    }
}

/// Size of the decoded payload of a base64 string.
fn decoded_len(base64: &str) -> usize {
    base64.trim_end_matches('=').len() * 3 / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(part: &ToolResultPart) -> &str {
        match part {
            ToolResultPart::Text { text } => text,
            ToolResultPart::Image { .. } => panic!("expected text"),
        }
    }

    #[test]
    fn converts_each_kind() {
        let parts = content_to_parts(&[
            McpContent::Text {
                text: "plain".into(),
            },
            McpContent::Image {
                data: "aGVsbG8=".into(),
                mime_type: "image/png".into(),
            },
            McpContent::ResourceLink {
                uri: "file:///notes.md".into(),
                name: "notes".into(),
            },
            McpContent::Resource {
                uri: "file:///a.txt".into(),
                mime_type: Some("text/plain".into()),
                text: Some("A".into()),
                blob: None,
            },
            McpContent::Resource {
                uri: "file:///logo.png".into(),
                mime_type: Some("image/png".into()),
                text: None,
                blob: Some("iVBO".into()),
            },
            McpContent::Audio {
                data: "aGVsbG8=".into(),
                mime_type: "audio/wav".into(),
            },
            McpContent::Resource {
                uri: "file:///x.bin".into(),
                mime_type: None,
                text: None,
                blob: Some("AAAA".into()),
            },
        ]);
        assert_eq!(parts.len(), 7);
        assert_eq!(text(&parts[0]), "plain");
        assert_eq!(
            parts[1],
            ToolResultPart::Image {
                source: MediaSource::Base64 {
                    media_type: "image/png".into(),
                    data: "aGVsbG8=".into()
                }
            }
        );
        assert_eq!(text(&parts[2]), "[resource link] notes: file:///notes.md");
        assert_eq!(text(&parts[3]), "[resource file:///a.txt]\nA");
        assert!(matches!(parts[4], ToolResultPart::Image { .. }));
        assert_eq!(text(&parts[5]), "[audio audio/wav, 5 bytes]");
        assert_eq!(
            text(&parts[6]),
            "[resource file:///x.bin: application/octet-stream, 3 bytes of binary data]"
        );
    }
}
