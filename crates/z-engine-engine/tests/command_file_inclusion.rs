//! `@path` in a command body inlines project files the policy lets the
//! model read; a file outside the project is not included.

mod support;

use support::{Harness, user_blocks};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn project_files_are_inlined_and_outside_files_are_not() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "TOP SECRET").unwrap();
    let repo = FixtureRepo::empty();
    repo.write("src/lib.rs", "pub fn answer() -> u32 { 42 }\n");
    let secret = outside.path().join("secret.txt");
    repo.write(
        ".z-engine/commands/explain.md",
        &format!(
            "Explain @src/lib.rs and @{} and @nobody please.",
            secret.display()
        ),
    );
    let mut h = Harness::start(repo).await;
    h.model.push(Script::text("explained"));
    h.command("explain", "");
    h.turn_finished().await;
    let body = user_blocks(&h.main_requests().pop().unwrap())[1].clone();
    assert!(
        body.contains("```src/lib.rs\npub fn answer() -> u32 { 42 }\n```"),
        "{body}"
    );
    assert!(!body.contains("TOP SECRET"), "{body}");
    assert!(body.contains("was not included"), "{body}");
    assert!(
        body.contains("@nobody please"),
        "unknown tokens stay: {body}"
    );
}
