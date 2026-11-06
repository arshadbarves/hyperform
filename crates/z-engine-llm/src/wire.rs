//! JSON and text fragments shared by the wire adapters.

use serde_json::{Value, json};
use z_engine_protocol::{CallId, MediaSource};

/// Sent in place of a tool result that has no text.
pub(crate) const EMPTY_TOOL_OUTPUT: &str = "(no text output)";

/// The prompt-cache breakpoint marker (`cache_control`).
pub(crate) fn ephemeral() -> Value {
    json!({"type": "ephemeral"})
}

/// An `f32` as the JSON number it prints as (0.2, not 0.20000000298...).
pub(crate) fn float(value: f32) -> Value {
    let shortest = value.to_string().parse::<f64>().unwrap_or(f64::from(value));
    json!(shortest)
}

/// Introduces tool-result images re-sent in a user message, for APIs whose
/// tool outputs carry text only.
pub(crate) fn tool_images_note(id: &CallId) -> String {
    format!("Images returned by tool call {id}:")
}

/// A data URL for inline media, or the remote URL as is.
pub(crate) fn media_url(source: &MediaSource) -> String {
    match source {
        MediaSource::Base64 { media_type, data } => format!("data:{media_type};base64,{data}"),
        MediaSource::Url { url } => url.clone(),
    }
}

pub(crate) fn document_filename(source: &MediaSource, title: Option<&str>) -> String {
    match (title, source) {
        (Some(title), _) => title.to_string(),
        (None, MediaSource::Base64 { media_type, .. }) if media_type == "text/plain" => {
            "document.txt".to_string()
        }
        _ => "document.pdf".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_keep_their_printed_value() {
        assert_eq!(float(0.2).to_string(), "0.2");
        assert_eq!(float(1.0).to_string(), "1.0");
        assert_eq!(ephemeral(), json!({"type": "ephemeral"}));
    }

    #[test]
    fn media_urls_and_document_names() {
        let text = MediaSource::Base64 {
            media_type: "text/plain".into(),
            data: "aGk=".into(),
        };
        assert_eq!(media_url(&text), "data:text/plain;base64,aGk=");
        assert_eq!(document_filename(&text, None), "document.txt");
        assert_eq!(document_filename(&text, Some("notes.md")), "notes.md");
        let remote = MediaSource::Url {
            url: "https://example.com/a.pdf".into(),
        };
        assert_eq!(media_url(&remote), "https://example.com/a.pdf");
        assert_eq!(document_filename(&remote, None), "document.pdf");
    }
}
