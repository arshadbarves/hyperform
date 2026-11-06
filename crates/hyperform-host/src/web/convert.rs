//! Response bodies to model-readable text: HTML to Markdown, JSON
//! pretty-printed, other text as-is, binary summarized.

const SKIPPED_TAGS: &[&str] = &["script", "style", "noscript", "template", "svg", "iframe"];

pub(super) fn to_content(content_type: &str, body: &[u8]) -> String {
    let kind = content_type.to_ascii_lowercase();
    let text = String::from_utf8_lossy(body);
    if kind.contains("html") || (kind.is_empty() && looks_like_html(&text)) {
        return html_to_markdown(&text);
    }
    if kind.contains("json") {
        let pretty = serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok());
        if let Some(pretty) = pretty {
            return pretty;
        }
    }
    let textual = kind.starts_with("text/")
        || kind.contains("xml")
        || kind.contains("javascript")
        || kind.contains("json");
    if textual || !body.iter().take(8 * 1024).any(|&b| b == 0) {
        return text.into_owned();
    }
    let kind = if kind.is_empty() {
        "unknown type"
    } else {
        kind.as_str()
    };
    format!("[binary content: {} bytes of {kind}]", body.len())
}

fn html_to_markdown(html: &str) -> String {
    let converter = htmd::HtmlToMarkdown::builder()
        .skip_tags(SKIPPED_TAGS.to_vec())
        .build();
    match converter.convert(html) {
        Ok(markdown) => markdown.trim().to_string(),
        Err(e) => {
            tracing::debug!(error = %e, "html conversion failed; returning raw html");
            html.to_string()
        }
    }
}

fn looks_like_html(text: &str) -> bool {
    let start = text
        .trim_start()
        .get(..15)
        .unwrap_or("")
        .to_ascii_lowercase();
    start.starts_with("<!doctype html") || start.starts_with("<html")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_by_content_type() {
        let md = to_content(
            "text/html; charset=utf-8",
            b"<html><head><script>x()</script></head><body><h1>Title</h1><p>Hi <b>there</b></p></body></html>",
        );
        assert!(md.contains("# Title"), "{md}");
        assert!(md.contains("**there**"), "{md}");
        assert!(!md.contains("x()"), "{md}");
        assert_eq!(
            to_content("application/json", br#"{"a":1}"#),
            "{\n  \"a\": 1\n}"
        );
        assert_eq!(to_content("text/plain", b"plain"), "plain");
        assert!(to_content("image/png", b"\x89PNG\0\0").starts_with("[binary content: 6 bytes"));
    }
}
