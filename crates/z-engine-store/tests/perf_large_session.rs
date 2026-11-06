//! Timing harness for opening a large session. Ignored by default; run with
//! `cargo test -p z-engine-store --release --test perf_large_session -- --ignored --nocapture`.

use std::time::Instant;

use z_engine_protocol::{
    CallId, ContentBlock, Message, PermissionMode, Role, SessionId, ToolResultPart,
};
use z_engine_store::{LogRecord, NewSession, SessionStore};

const ROUNDS: usize = 2_500;

fn tool_round(index: usize) -> [Message; 2] {
    let id = CallId::from(format!("call_{index}"));
    let call = Message::new(
        Role::Assistant,
        vec![
            ContentBlock::text(format!("Reading file number {index} to check the parser.")),
            ContentBlock::ToolUse {
                id: id.clone(),
                name: "Read".into(),
                input: serde_json::json!({ "file_path": format!("src/module_{index}.rs") }),
            },
        ],
    );
    let result = Message::new(
        Role::User,
        vec![ContentBlock::ToolResult {
            tool_use_id: id,
            content: vec![ToolResultPart::Text {
                text: "     1\tfn main() {}\n".repeat(400),
            }],
            is_error: false,
        }],
    );
    [call, result]
}

#[test]
#[ignore = "timing harness"]
fn opening_a_large_session_is_fast() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::new(dir.path().to_path_buf());
    let session_id = SessionId::new();
    let (mut log, _meta) = store
        .create(NewSession {
            session_id: session_id.clone(),
            project_root: "/tmp/project".into(),
            model: "test-model".into(),
            mode: PermissionMode::Default,
            planning: false,
        })
        .unwrap();
    log.append(&LogRecord::Message {
        message: Message::user_text("Refactor the parser"),
        turn_id: None,
    })
    .unwrap();
    for index in 0..ROUNDS {
        for message in tool_round(index) {
            log.append(&LogRecord::Message {
                message,
                turn_id: None,
            })
            .unwrap();
        }
    }
    log.sync().unwrap();
    let bytes = std::fs::metadata(log.path()).unwrap().len();

    let started = Instant::now();
    let loaded = store.load(&session_id).unwrap();
    let elapsed = started.elapsed();

    println!(
        "loaded {} messages from {:.1} MB in {:?}",
        loaded.state.transcript.len(),
        bytes as f64 / 1_048_576.0,
        elapsed
    );
    assert_eq!(loaded.state.transcript.len(), ROUNDS * 2 + 1);
    assert!(elapsed.as_secs_f64() < 5.0, "opening took {elapsed:?}");
}
