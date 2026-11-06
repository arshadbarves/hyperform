//! `decisions_session_context` end to end: before a chat's first request the
//! files the model calls relevant rank first in the repository map, which
//! then stays byte-identical for every later request of the session.

mod laya;
mod support;

use serde_json::{Value, json};
use support::{BASE_SETTINGS, Harness};
use z_engine_llm::ModelRequest;
use z_engine_testkit::{FixtureRepo, Script};

fn login_is_relevant(name: &str, state: &Value) -> &'static str {
    let file = state["file"].as_str().unwrap_or_default();
    if name == "session_context_file" && file.contains("login") {
        laya::YES
    } else {
        laya::NO
    }
}

fn repo() -> FixtureRepo {
    FixtureRepo::git(&[
        ("src/alpha.rs", "pub fn stripes() -> u32 {\n    1\n}\n"),
        (
            "src/login_form.rs",
            "pub fn validate_login(form: &str) -> bool {\n    !form.is_empty()\n}\n",
        ),
    ])
}

fn repo_map(request: &ModelRequest) -> String {
    let block = request
        .system
        .iter()
        .find(|b| b.text.contains("src/alpha.rs:"));
    block.map(|b| b.text.clone()).unwrap_or_default()
}

fn first_file(map: &str) -> &str {
    let header = map
        .lines()
        .find(|line| line.ends_with(".rs:"))
        .unwrap_or_default();
    header.trim_end_matches(':')
}

async fn maps(settings: &str) -> Vec<String> {
    let mut h = Harness::builder(repo()).settings(settings).start().await;
    let alpha = h.path("src/alpha.rs");
    h.model
        .push(Script::tool("Read", json!({ "file_path": alpha })));
    h.model.push(Script::text("Read."));
    h.run_turn("fix the login form validation").await;
    h.model.push(Script::text("Again."));
    h.run_turn("and the login form message").await;
    h.main_requests().iter().map(repo_map).collect()
}

#[tokio::test]
async fn relevant_files_rank_first_and_the_map_then_stays_fixed() {
    let off = maps(BASE_SETTINGS).await;
    assert_eq!(first_file(&off[0]), "src/alpha.rs", "{}", off[0]);
    let endpoint = laya::laya(login_is_relevant).await;
    let on = maps(&laya::settings(
        BASE_SETTINGS,
        &endpoint,
        &["decisions_session_context"],
    ))
    .await;
    assert_eq!(on.len(), 3);
    assert_eq!(first_file(&on[0]), "src/login_form.rs", "{}", on[0]);
    assert!(
        on.iter().all(|map| map == &on[0]),
        "byte-stable after the first request"
    );
}
