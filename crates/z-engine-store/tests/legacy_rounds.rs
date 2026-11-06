//! v1 conversion rules: unfinished and interrupted tool rounds, stray
//! results, turn closure, titles, and prompt content.

mod support;

use serde_json::{Value, json};
use support::v1::write_v1;
use support::{assert_tool_rounds_complete, temp_store, ulid_at};
use z_engine_protocol::{ContentBlock, MediaSource, Role, SessionId, TurnOutcome};
use z_engine_store::{LoadedSession, import_v1};

fn import(events: &[Value]) -> LoadedSession {
    let (_dir, store) = temp_store();
    let id = ulid_at(1_700_000_000_000);
    let path = write_v1(store.dir(), &id, events);
    import_v1(store.dir(), &path).unwrap();
    store.load(&SessionId::from(id.as_str())).unwrap()
}

fn user(text: &str) -> Value {
    json!({"type": "user_msg", "text": text})
}

fn calls(content: Option<&str>, ids: &[&str]) -> Value {
    let tool_calls: Vec<Value> = ids
        .iter()
        .map(|id| json!({"id": id, "name": "bash", "arguments": "{\"command\":\"ls\"}"}))
        .collect();
    json!({"type": "assistant_msg", "content": content, "tool_calls": tool_calls})
}

fn result(id: &str) -> Value {
    json!({"type": "tool_result", "tool_call_id": id, "content": format!("output of {id}")})
}

fn turn_end(outcome: &str) -> Value {
    json!({"type": "turn_end", "outcome": outcome})
}

fn outcomes(loaded: &LoadedSession) -> Vec<TurnOutcome> {
    loaded
        .state
        .turns
        .iter()
        .map(|turn| turn.outcome.clone())
        .collect()
}

#[test]
fn trailing_unfinished_round_is_dropped_with_its_partial_results() {
    let loaded = import(&[
        user("go"),
        calls(Some("Working."), &["c1", "c2"]),
        result("c1"),
        turn_end("failed"),
    ]);
    let transcript = &loaded.state.transcript;
    assert_eq!(transcript.len(), 1);
    assert_eq!(transcript[0].text(), "go");
    assert!(matches!(
        outcomes(&loaded)[..],
        [TurnOutcome::Failed { .. }]
    ));
}

#[test]
fn trailing_round_at_end_of_file_is_dropped_and_the_turn_interrupted() {
    let loaded = import(&[user("go"), calls(None, &["c1"])]);
    assert_eq!(loaded.state.working.len(), 1);
    assert_eq!(loaded.state.working[0].role, Role::User);
    assert_eq!(outcomes(&loaded), [TurnOutcome::Interrupted]);
    assert_eq!(loaded.state.open_turn, None);
    assert_eq!(loaded.meta.last_outcome, Some(TurnOutcome::Interrupted));
}

#[test]
fn interrupted_round_before_later_messages_gets_error_results() {
    let loaded = import(&[
        user("a"),
        calls(None, &["c1", "c2"]),
        result("c2"),
        user("b"),
        json!({"type": "assistant_msg", "content": "done"}),
    ]);
    let transcript = &loaded.state.transcript;
    assert_eq!(transcript.len(), 5);
    let errors: Vec<(&str, bool)> = transcript[2]
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                is_error,
                ..
            } => Some((tool_use_id.as_str(), *is_error)),
            _ => None,
        })
        .collect();
    assert_eq!(errors, [("c1", true), ("c2", false)]);
    assert_tool_rounds_complete(&loaded.state.working);
    assert_eq!(
        outcomes(&loaded),
        [TurnOutcome::Interrupted, TurnOutcome::Interrupted]
    );
}

#[test]
fn stray_and_duplicate_results_are_dropped() {
    let loaded = import(&[
        user("go"),
        result("ghost"),
        calls(None, &["c1"]),
        result("c1"),
        result("c1"),
        result("ghost2"),
        turn_end("completed"),
    ]);
    let transcript = &loaded.state.transcript;
    assert_eq!(transcript.len(), 3);
    assert!(transcript[2].is_tool_results());
    assert_eq!(transcript[2].content.len(), 1);
    assert_tool_rounds_complete(&loaded.state.working);
    assert_eq!(outcomes(&loaded), [TurnOutcome::Completed]);
}

#[test]
fn missing_meta_and_title_fall_back_to_v1_display_rules() {
    let line = "Refactor the parser module so that it handles nested generics correctly";
    let loaded = import(&[
        user(&format!("\n  {line}\nmore detail")),
        json!({"type": "assistant_msg", "content": "ok"}),
        turn_end("completed"),
    ]);
    let expected: String = line.chars().take(48).chain(['…']).collect();
    assert_eq!(loaded.meta.title.as_deref(), Some(expected.as_str()));
    assert_eq!(loaded.meta.project_root, "");
    assert_eq!(loaded.meta.model, "");
}

#[test]
fn only_the_first_non_blank_v1_title_is_kept() {
    let loaded = import(&[
        user("go"),
        json!({"type": "title", "text": "   "}),
        json!({"type": "title", "text": "First"}),
        json!({"type": "title", "text": "Second"}),
    ]);
    assert_eq!(loaded.meta.title.as_deref(), Some("First"));
}

#[test]
fn image_only_prompts_have_no_empty_text_block() {
    let loaded = import(&[json!({
        "type": "user_msg",
        "text": "",
        "images": ["data:image/jpeg;base64,/9j/4AAQ", "https://example.com/shot.png"]
    })]);
    assert_eq!(
        loaded.state.transcript[0].content,
        [
            ContentBlock::Image {
                source: MediaSource::Base64 {
                    media_type: "image/jpeg".into(),
                    data: "/9j/4AAQ".into()
                }
            },
            ContentBlock::Image {
                source: MediaSource::Url {
                    url: "https://example.com/shot.png".into()
                }
            }
        ]
    );
    assert_eq!(loaded.meta.title, None);
}

#[test]
fn empty_assistant_messages_are_skipped() {
    let loaded = import(&[
        user("go"),
        json!({"type": "assistant_msg"}),
        json!({"type": "assistant_msg", "content": ""}),
        json!({"type": "assistant_msg", "content": "real"}),
        turn_end("completed"),
    ]);
    let roles: Vec<Role> = loaded.state.transcript.iter().map(|m| m.role).collect();
    assert_eq!(roles, [Role::User, Role::Assistant]);
    assert_eq!(loaded.state.transcript[1].text(), "real");
}
