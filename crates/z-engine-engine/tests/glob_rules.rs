//! A rule scoped by globs joins the context once, on the first read of a
//! matching file; an `alwaysApply` rule sits in the system prompt.

mod support;

use serde_json::json;
use support::{Harness, last_user_text};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn glob_rule_is_injected_once_on_first_matching_read() {
    let repo = FixtureRepo::git(&[
        ("src/lib.rs", "pub fn one() {}\n"),
        ("notes.txt", "plain\n"),
        (
            ".z-engine/rules/rust.md",
            "---\ndescription: Rust style\nglobs: \"*.rs\"\n---\nUse snake_case for every Rust item.\n",
        ),
        (
            ".z-engine/rules/always.md",
            "---\nalwaysApply: true\n---\nAlways answer briefly.\n",
        ),
    ]);
    let mut h = Harness::builder(repo).trusted().start().await;
    let (notes, lib) = (h.path("notes.txt"), h.path("src/lib.rs"));
    h.model
        .push(Script::tool("Read", json!({"file_path": notes})));
    h.model
        .push(Script::tool("Read", json!({"file_path": lib})));
    h.model
        .push(Script::tool("Read", json!({"file_path": lib})));
    h.model.push(Script::text("Read it."));
    h.run_turn("read the files").await;

    let requests = h.main_requests();
    assert_eq!(requests.len(), 4);
    let system: String = requests[0]
        .system
        .iter()
        .map(|block| block.text.as_str())
        .collect();
    let needle = "Use snake_case for every Rust item.";
    assert!(system.contains("Always answer briefly."));
    assert!(!system.contains(needle));
    assert!(!last_user_text(&requests[1]).contains(needle));
    let injected = last_user_text(&requests[2]);
    assert!(
        injected.contains(needle) && injected.contains("Rule (rust)"),
        "{injected}"
    );
    assert!(!last_user_text(&requests[3]).contains(needle));
}
