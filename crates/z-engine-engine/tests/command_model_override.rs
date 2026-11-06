//! A command's `model` frontmatter applies to its turn only.

mod support;

use support::Harness;
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn command_model_is_used_for_one_turn() {
    let repo = FixtureRepo::empty();
    repo.write(
        ".z-engine/commands/deep.md",
        "---\nmodel: big-model\n---\nThink hard.",
    );
    let mut h = Harness::start(repo).await;
    h.model.push(Script::text("deep answer"));
    h.command("deep", "");
    h.turn_finished().await;
    h.model.push(Script::text("normal answer"));
    h.run_turn("and now?").await;

    let models: Vec<String> = h
        .main_requests()
        .into_iter()
        .map(|request| request.model)
        .collect();
    assert_eq!(models, ["big-model", "test-model"]);
    h.command("status", "");
    assert!(h.command_output("status").await.contains("`test-model`"));
}
