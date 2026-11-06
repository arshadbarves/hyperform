//! Session export: a readable Markdown transcript (reminders hidden, tool
//! calls summarized) or every log record as JSON.

use std::collections::HashMap;

use z_engine_protocol::{CallId, ContentBlock, Message, Role, SessionId, ToolResultPart};
use z_engine_store::read_records;

use crate::error::EngineError;
use crate::options::ExportFormat;
use crate::session::{SessionCore, Shared};

const INPUT_CHARS: usize = 200;
const RESULT_CHARS: usize = 160;

pub(crate) fn export(
    shared: &Shared,
    live: Option<&SessionCore>,
    id: &SessionId,
    format: ExportFormat,
) -> Result<String, EngineError> {
    match format {
        ExportFormat::Json => {
            let path = match live {
                Some(core) => core.journal.path().to_path_buf(),
                None => shared.store.open_append(id)?.path().to_path_buf(),
            };
            let records = read_records(&path)?.records;
            serde_json::to_string_pretty(&records)
                .map_err(|error| EngineError::encode("session records", error))
        }
        ExportFormat::Markdown => {
            let (title, transcript) = match live {
                Some(core) => {
                    core.with_state(|state| (state.title.clone(), state.transcript.clone()))
                }
                None => {
                    let loaded = shared.store.load(id)?;
                    (loaded.state.title, loaded.state.transcript)
                }
            };
            let title = title.unwrap_or_else(|| format!("Session {id}"));
            Ok(markdown(&title, &transcript))
        }
    }
}

fn markdown(title: &str, transcript: &[Message]) -> String {
    let names: HashMap<&CallId, &str> = transcript
        .iter()
        .flat_map(Message::tool_uses)
        .map(|(id, name, _)| (id, name))
        .collect();
    let mut out = format!("# {title}\n");
    for message in transcript {
        let mut lines = Vec::new();
        for block in &message.content {
            match block {
                ContentBlock::Text { text } if !is_reminder(text) && !text.trim().is_empty() => {
                    lines.push(text.trim().to_string());
                }
                ContentBlock::Image { .. } => lines.push("_[image]_".to_string()),
                ContentBlock::Document { title, .. } => {
                    lines.push(format!(
                        "_[document{}]_",
                        title
                            .as_deref()
                            .map(|t| format!(": {t}"))
                            .unwrap_or_default()
                    ));
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    lines.push(format!(
                        "- Tool call `{name}`: `{}`",
                        clip(&input.to_string(), INPUT_CHARS)
                    ));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let name = names.get(tool_use_id).copied().unwrap_or("tool");
                    let label = if *is_error { "Error from" } else { "Result of" };
                    lines.push(format!("- {label} `{name}`: {}", summary(content)));
                }
                _ => {}
            }
        }
        if lines.is_empty() {
            continue;
        }
        let speaker = match message.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
        };
        out.push_str(&format!("\n## {speaker}\n\n{}\n", lines.join("\n\n")));
    }
    out
}

fn summary(content: &[ToolResultPart]) -> String {
    let text: String = content
        .iter()
        .filter_map(|part| match part {
            ToolResultPart::Text { text } => Some(text.as_str()),
            ToolResultPart::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let first = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    format!(
        "{} ({} characters)",
        clip(first.trim(), RESULT_CHARS),
        text.chars().count()
    )
}

fn is_reminder(text: &str) -> bool {
    let text = text.trim();
    text.starts_with("<system-reminder>") && text.ends_with("</system-reminder>")
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(max).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn markdown_hides_reminders_and_summarizes_tools() {
        let call = CallId::from("c1");
        let transcript = vec![
            Message::new(
                Role::User,
                vec![
                    ContentBlock::text("Read the file"),
                    ContentBlock::text("<system-reminder>\nplan mode\n</system-reminder>"),
                ],
            ),
            Message::new(
                Role::Assistant,
                vec![ContentBlock::ToolUse {
                    id: call.clone(),
                    name: "Read".into(),
                    input: json!({"file_path": "a.rs"}),
                }],
            ),
            Message::new(
                Role::User,
                vec![ContentBlock::tool_result(call, "fn main() {}", false)],
            ),
        ];
        let out = markdown("Demo", &transcript);
        assert!(out.starts_with("# Demo"));
        assert!(out.contains("Read the file") && !out.contains("plan mode"));
        assert!(out.contains("- Tool call `Read`"));
        assert!(out.contains("- Result of `Read`: fn main() {} (12 characters)"));
    }
}
