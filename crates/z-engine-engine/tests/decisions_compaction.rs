//! `decisions_compaction` end to end: under context pressure the model's
//! verdicts decide which older results go, while the kernel's hard keeps
//! (a file the request names) stay whatever the model says. With no
//! verdict at all (nothing asked yet, or the model down) today's plan runs.

mod laya;
mod support;

use serde_json::{Value, json};
use support::{Harness, results};
use z_engine_llm::ModelRequest;
use z_engine_testkit::{FixtureRepo, Script};

const SETTINGS: &str = "schema = 2\n\n[model]\nmain = \"test-model\"\ncontext_window = 10000\n\n\
                        [context]\nkeep_recent_tool_results = 1\ncompact_at_percent = 99\n";

/// Needed when the output says so; the request's own file is "not needed"
/// too, which the hard keep must overrule.
fn needed_when_marked(name: &str, state: &Value) -> &'static str {
    let output = state["output"].as_str().unwrap_or_default();
    if name == "compaction_relevant" && output.contains("NEEDED") {
        laya::YES
    } else {
        laya::NO
    }
}

fn file(tag: &str) -> String {
    (0..500)
        .map(|line| format!("{tag} line {line:04} with some filler text\n"))
        .collect()
}

async fn last_request(settings: &str) -> ModelRequest {
    let repo = FixtureRepo::git(&[
        ("needed.txt", &file("NEEDED")),
        ("auth.txt", &file("auth")),
        ("other.txt", &file("other")),
        ("last.txt", &file("last")),
    ]);
    let mut h = Harness::builder(repo).settings(settings).start().await;
    for name in ["needed.txt", "auth.txt", "other.txt", "last.txt"] {
        h.model
            .push(Script::tool("Read", json!({ "file_path": h.path(name) })));
    }
    h.model.push(Script::text("done"));
    h.run_turn("Fix the bug described in auth.txt").await;
    h.main_requests().pop().unwrap()
}

/// Per read, in order: whether its result is still there in full.
fn kept(request: &ModelRequest) -> Vec<bool> {
    let parts = request.messages.iter().flat_map(results);
    parts
        .map(|(_, _, text)| !text.starts_with("[cleared"))
        .collect()
}

#[tokio::test]
async fn the_model_picks_what_goes_and_hard_keeps_stay() {
    let off = last_request(SETTINGS).await;
    assert_eq!(
        kept(&off),
        [false, false, false, true],
        "today: all but the newest go"
    );
    let endpoint = laya::laya(needed_when_marked).await;
    let on = last_request(&laya::settings(
        SETTINGS,
        &endpoint,
        &["decisions_compaction"],
    ))
    .await;
    assert_eq!(
        kept(&on),
        [true, true, false, true],
        "named file by rule, NEEDED by the model"
    );
}
