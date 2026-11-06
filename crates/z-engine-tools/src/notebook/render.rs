//! Notebook cells as `Read` shows them: each cell with its id, type, and
//! source, followed by its text outputs; image outputs become image parts
//! when the model can see them.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;
use z_engine_protocol::{MediaSource, ToolResultPart};

use super::model::{Notebook, cell_id, cell_kind, joined};

const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// Renders every cell. With `images`, image outputs up to `max_image_bytes`
/// become image parts; otherwise they are noted in the text.
pub(crate) fn render(nb: &Notebook, images: bool, max_image_bytes: u64) -> Vec<ToolResultPart> {
    let mut parts = Vec::new();
    let mut text = String::new();
    for (index, cell) in nb.cells().iter().enumerate() {
        let id = cell_id(cell, index);
        text.push_str(&format!(
            "<cell id=\"{id}\" type=\"{}\">\n",
            cell_kind(cell)
        ));
        push_block(&mut text, &joined(cell.get("source")));
        text.push_str("</cell>\n");
        let outputs = cell.get("outputs").and_then(Value::as_array);
        let Some(outputs) = outputs.filter(|outputs| !outputs.is_empty()) else {
            continue;
        };
        text.push_str(&format!("<outputs cell=\"{id}\">\n"));
        for output in outputs {
            render_output(output, images, max_image_bytes, &mut text, &mut parts);
        }
        text.push_str("</outputs>\n");
    }
    flush(&mut text, &mut parts);
    parts
}

fn render_output(
    output: &Value,
    images: bool,
    max_image_bytes: u64,
    text: &mut String,
    parts: &mut Vec<ToolResultPart>,
) {
    match output.get("output_type").and_then(Value::as_str) {
        Some("stream") => push_block(text, &strip_ansi(&joined(output.get("text")))),
        Some("error") => {
            let name = output
                .get("ename")
                .and_then(Value::as_str)
                .unwrap_or("Error");
            let value = output.get("evalue").and_then(Value::as_str).unwrap_or("");
            let traceback: Vec<&str> = output
                .get("traceback")
                .and_then(Value::as_array)
                .map(|lines| lines.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            push_block(
                text,
                &strip_ansi(&format!("{name}: {value}\n{}", traceback.join("\n"))),
            );
        }
        _ => {
            let Some(data) = output.get("data") else {
                return;
            };
            for media_type in IMAGE_TYPES {
                let Some(encoded) = data.get(media_type) else {
                    continue;
                };
                let payload: String = joined(Some(encoded)).split_whitespace().collect();
                let bytes = payload.len() as u64 * 3 / 4;
                if images && bytes <= max_image_bytes {
                    flush(text, parts);
                    parts.push(ToolResultPart::Image {
                        source: MediaSource::Base64 {
                            media_type: media_type.to_string(),
                            data: payload,
                        },
                    });
                } else {
                    text.push_str(&format!(
                        "[image output: {media_type}, {bytes} bytes, not shown]\n"
                    ));
                }
            }
            if let Some(plain) = data.get("text/plain") {
                push_block(text, &joined(Some(plain)));
            }
        }
    }
}

fn push_block(text: &mut String, block: &str) {
    text.push_str(block);
    if !block.is_empty() && !block.ends_with('\n') {
        text.push('\n');
    }
}

fn flush(text: &mut String, parts: &mut Vec<ToolResultPart>) {
    if !text.is_empty() {
        parts.push(ToolResultPart::Text {
            text: std::mem::take(text),
        });
    }
}

/// Tracebacks carry terminal colour codes.
fn strip_ansi(text: &str) -> String {
    static ANSI: OnceLock<Option<Regex>> = OnceLock::new();
    match ANSI.get_or_init(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").ok()) {
        Some(ansi) => ansi.replace_all(text, "").into_owned(),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn nb(outputs: Value) -> Notebook {
        let doc = json!({"cells": [
            {"cell_type": "code", "id": "c1", "source": ["x = 1\n", "x"], "outputs": outputs}
        ]});
        Notebook::parse(&doc.to_string()).unwrap()
    }

    #[test]
    fn cells_and_text_outputs_are_rendered() {
        let parts = render(
            &nb(json!([
                {"output_type": "stream", "text": "hello\n"},
                {"output_type": "execute_result", "data": {"text/plain": ["1"]}},
                {"output_type": "error", "ename": "ValueError", "evalue": "bad",
                 "traceback": ["\u{1b}[0;31mTraceback\u{1b}[0m"]}
            ])),
            true,
            1024,
        );
        let [ToolResultPart::Text { text }] = parts.as_slice() else {
            panic!("{parts:?}");
        };
        assert!(
            text.starts_with("<cell id=\"c1\" type=\"code\">\nx = 1\nx\n</cell>\n"),
            "{text}"
        );
        assert!(
            text.contains(
                "<outputs cell=\"c1\">\nhello\n1\nValueError: bad\nTraceback\n</outputs>"
            )
        );
    }

    #[test]
    fn image_outputs_become_parts_only_with_vision() {
        let outputs = json!([{"output_type": "display_data", "data": {"image/png": "aGVsbG8=\n"}}]);
        let with = render(&nb(outputs.clone()), true, 1024);
        assert!(
            matches!(&with[1], ToolResultPart::Image { source: MediaSource::Base64 { data, .. } } if data == "aGVsbG8=")
        );
        let without = render(&nb(outputs), false, 1024);
        assert_eq!(without.len(), 1);
        assert!(
            matches!(&without[0], ToolResultPart::Text { text } if text.contains("[image output: image/png"))
        );
    }
}
