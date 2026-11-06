//! Timing harness for per-round token estimation over a long working set.
//! Ignored by default; run with
//! `cargo test -p z-engine-context --release --test perf_token_estimates -- --ignored --nocapture`.

use std::time::Instant;

use z_engine_context::estimate_messages;
use z_engine_protocol::{CallId, ContentBlock, Message, Role, ToolResultPart};

#[test]
#[ignore = "timing harness"]
fn estimating_a_long_working_set_is_fast() {
    let messages: Vec<Message> = (0..1_000)
        .flat_map(|index| {
            let id = CallId::from(format!("call_{index}"));
            [
                Message::new(
                    Role::Assistant,
                    vec![ContentBlock::ToolUse {
                        id: id.clone(),
                        name: "Grep".into(),
                        input: serde_json::json!({ "pattern": "fn parse", "path": "src" }),
                    }],
                ),
                Message::new(
                    Role::User,
                    vec![ContentBlock::ToolResult {
                        tool_use_id: id,
                        content: vec![ToolResultPart::Text {
                            text: "src/parser.rs:12: fn parse(input: &str) -> Ast\n".repeat(40),
                        }],
                        is_error: false,
                    }],
                ),
            ]
        })
        .collect();

    let started = Instant::now();
    let tokens = estimate_messages(&messages);
    let elapsed = started.elapsed();

    println!(
        "estimated {tokens} tokens over {} messages in {elapsed:?}",
        messages.len()
    );
    assert!(tokens > 0);
    assert!(elapsed.as_millis() < 500, "estimation took {elapsed:?}");
}
