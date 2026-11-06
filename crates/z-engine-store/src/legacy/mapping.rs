//! Field-level v1 -> v2 mappings: images, tool names and inputs, turn
//! outcomes, titles, and task-report notes.

use serde_json::{Map, Value};
use z_engine_protocol::{MediaSource, TurnOutcome};

use super::v1::V1TaskReport;

/// The v1 sidebar clipped persisted titles to this many characters.
const TITLE_CHARS: usize = 80;
/// v1's first-prompt fallback title length (plus an ellipsis).
const FALLBACK_TITLE_CHARS: usize = 48;

/// A v1 image (a data URL or a plain URL) as a v2 media source.
pub(super) fn image_source(url: &str) -> MediaSource {
    if let Some((header, data)) = url
        .strip_prefix("data:")
        .and_then(|rest| rest.split_once(','))
    {
        let mut params = header.split(';');
        let media_type = params.next().unwrap_or_default();
        if params.any(|param| param.eq_ignore_ascii_case("base64")) {
            let media_type = if media_type.is_empty() {
                "application/octet-stream"
            } else {
                media_type
            };
            return MediaSource::Base64 {
                media_type: media_type.to_string(),
                data: data.to_string(),
            };
        }
    }
    MediaSource::Url {
        url: url.to_string(),
    }
}

/// The v2 (Claude Code) name of a v1 tool. Other names are history and
/// stay as they were.
pub(super) fn tool_name(v1: &str) -> &str {
    match v1 {
        "read_file" => "Read",
        "write_file" => "Write",
        "edit_file" => "Edit",
        "bash" => "Bash",
        "glob" => "Glob",
        "grep" => "Grep",
        "task" => "Agent",
        "run_verification" | "inspect_project" => "Verify",
        other => other,
    }
}

/// Tool input from v1's JSON-encoded arguments; `{}` unless they decode to
/// a JSON object (providers require an object).
pub(super) fn tool_input(arguments: &str) -> Value {
    match serde_json::from_str::<Value>(arguments) {
        Ok(value @ Value::Object(_)) => value,
        _ => Value::Object(Map::new()),
    }
}

pub(super) fn turn_outcome(v1: &str) -> TurnOutcome {
    match v1 {
        "completed" => TurnOutcome::Completed,
        "aborted" => TurnOutcome::Cancelled,
        "failed" => TurnOutcome::Failed {
            message: "the v1 turn failed; no error detail was recorded".into(),
        },
        other => TurnOutcome::Failed {
            message: format!("unrecognized v1 turn outcome {other:?}"),
        },
    }
}

/// A persisted v1 title as the v1 sidebar showed it; `None` when blank.
pub(super) fn persisted_title(text: &str) -> Option<String> {
    let title = text.trim();
    (!title.is_empty()).then(|| title.chars().take(TITLE_CHARS).collect())
}

/// v1's fallback title: the first non-empty line of the first prompt,
/// clipped with an ellipsis.
pub(super) fn fallback_title(prompt: &str) -> Option<String> {
    let line = prompt
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let mut title: String = line.chars().take(FALLBACK_TITLE_CHARS).collect();
    if line.chars().count() > FALLBACK_TITLE_CHARS {
        title.push('…');
    }
    Some(title)
}

/// One note summarizing the last v1 verification report of a turn.
pub(super) fn report_note(report: &V1TaskReport) -> String {
    let status = if report.status.is_empty() {
        "unknown"
    } else {
        report.status.as_str()
    };
    let mut lines = vec![format!("v1 task report ({status}): {}", report.goal.trim())];
    lines.extend(
        report
            .checks
            .iter()
            .map(|check| format!("check {}: {}", check.outcome, check.summary.trim())),
    );
    lines.extend(
        report
            .blockers
            .iter()
            .map(|blocker| format!("blocker: {}", blocker.trim())),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy::v1::V1Check;

    #[test]
    fn data_urls_become_base64_sources() {
        assert_eq!(
            image_source("data:image/png;base64,iVBORw0KGgo="),
            MediaSource::Base64 {
                media_type: "image/png".into(),
                data: "iVBORw0KGgo=".into()
            }
        );
        assert_eq!(
            image_source("data:image/jpeg;name=a.jpg;BASE64,/9j/"),
            MediaSource::Base64 {
                media_type: "image/jpeg".into(),
                data: "/9j/".into()
            }
        );
        for url in ["https://example.com/a.png", "data:image/svg+xml,%3Csvg%3E"] {
            assert_eq!(image_source(url), MediaSource::Url { url: url.into() });
        }
    }

    #[test]
    fn tool_names_follow_claude_code_and_unknown_names_stay() {
        let pairs = [
            ("read_file", "Read"),
            ("write_file", "Write"),
            ("edit_file", "Edit"),
            ("bash", "Bash"),
            ("glob", "Glob"),
            ("grep", "Grep"),
            ("task", "Agent"),
            ("run_verification", "Verify"),
            ("inspect_project", "Verify"),
            ("update_context_notes", "update_context_notes"),
            ("go_to_definition", "go_to_definition"),
        ];
        for (v1, v2) in pairs {
            assert_eq!(tool_name(v1), v2);
        }
    }

    #[test]
    fn tool_input_falls_back_to_an_empty_object() {
        assert_eq!(
            tool_input(r#"{"path":"a"}"#),
            serde_json::json!({"path": "a"})
        );
        for bad in ["", "{not json", "[1,2]", "\"text\""] {
            assert_eq!(tool_input(bad), serde_json::json!({}));
        }
    }

    #[test]
    fn outcomes_and_titles_match_v1_display_rules() {
        assert_eq!(turn_outcome("completed"), TurnOutcome::Completed);
        assert_eq!(turn_outcome("aborted"), TurnOutcome::Cancelled);
        assert!(matches!(turn_outcome("failed"), TurnOutcome::Failed { .. }));
        assert!(
            matches!(turn_outcome("odd"), TurnOutcome::Failed { message } if message.contains("odd"))
        );
        assert_eq!(persisted_title("  Fix auth  ").as_deref(), Some("Fix auth"));
        assert_eq!(persisted_title("   "), None);
        let long = format!("\n  {}\nsecond line", "a".repeat(60));
        let title = fallback_title(&long).unwrap();
        assert_eq!(title.chars().count(), FALLBACK_TITLE_CHARS + 1);
        assert!(title.ends_with('…'));
        assert_eq!(fallback_title(" \n "), None);
    }

    #[test]
    fn report_notes_list_checks_and_blockers() {
        let report = V1TaskReport {
            goal: "fix it".into(),
            status: "blocked".into(),
            blockers: vec!["needs a database".into()],
            checks: vec![V1Check {
                outcome: "failed".into(),
                summary: "2 tests failed".into(),
            }],
        };
        assert_eq!(
            report_note(&report),
            "v1 task report (blocked): fix it\ncheck failed: 2 tests failed\nblocker: needs a database"
        );
    }
}
