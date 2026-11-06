//! Above half the context window, older tool results are cleared in the
//! working set (the original spilled to an artifact) while the display
//! transcript keeps them and every call keeps its result.

mod support;

use serde_json::json;
use support::{Harness, assert_valid_transcript, results};
use z_engine_testkit::{FixtureRepo, Script};

const SETTINGS: &str = "schema = 2\n\n[model]\nmain = \"test-model\"\ncontext_window = 10000\n\n\
                        [context]\nkeep_recent_tool_results = 1\ncompact_at_percent = 99\n";

fn big_file(tag: &str) -> String {
    (0..500)
        .map(|line| format!("{tag} line {line:04} with some filler text\n"))
        .collect()
}

#[tokio::test]
async fn old_tool_results_are_cleared_under_pressure() {
    let repo = FixtureRepo::git(&[("one.txt", &big_file("one")), ("two.txt", &big_file("two"))]);
    let mut h = Harness::builder(repo).settings(SETTINGS).start().await;
    h.model.push(Script::tool(
        "Read",
        json!({ "file_path": h.path("one.txt") }),
    ));
    h.model.push(Script::tool(
        "Read",
        json!({ "file_path": h.path("two.txt") }),
    ));
    h.model.push(Script::text("read both"));
    h.run_turn("read the files").await;

    let requests = h.main_requests();
    assert_eq!(requests.len(), 3);
    let last = &requests[2];
    assert_valid_transcript(&last.messages);
    let first_result = &results(&last.messages[2])[0].2;
    assert!(
        first_result.starts_with("[cleared"),
        "{}",
        &first_result[..first_result.len().min(200)]
    );
    assert!(
        first_result.contains("artifacts"),
        "the original is spilled: {first_result}"
    );
    let second_result = &results(&last.messages[4])[0].2;
    assert!(second_result.contains("two line 0499"));

    let transcript = h.transcript();
    assert!(
        results(&transcript[2])[0].2.contains("one line 0499"),
        "display keeps originals"
    );
}
