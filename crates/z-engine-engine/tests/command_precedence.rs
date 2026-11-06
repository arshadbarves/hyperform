//! A project command replaces a user command of the same name (and the
//! built-in prompt command), but never an engine built-in.

mod support;

use support::{Harness, user_blocks};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn project_commands_override_user_and_builtin_ones() {
    let repo = FixtureRepo::empty();
    repo.write(".z-engine/commands/greet.md", "Project greeting.");
    repo.write(
        ".z-engine/commands/review.md",
        "Project review of $ARGUMENTS.",
    );
    repo.write(".z-engine/commands/status.md", "Should never run.");
    let builder = Harness::builder(repo);
    let mut h = builder.start().await;
    let user_commands = h.paths.config_dir.join("commands");
    std::fs::create_dir_all(&user_commands).unwrap();
    std::fs::write(user_commands.join("greet.md"), "User greeting.").unwrap();
    h.send(z_engine_protocol::Command::ReloadExtensions);

    h.model.push(Script::text("hi"));
    h.command("greet", "");
    h.turn_finished().await;
    let blocks = user_blocks(&h.main_requests().pop().unwrap());
    assert!(blocks[1].contains("Project greeting."), "{blocks:?}");

    h.model.push(Script::text("reviewed"));
    h.command("review", "main.rs");
    h.turn_finished().await;
    let blocks = user_blocks(&h.main_requests().pop().unwrap());
    assert!(
        blocks[1].contains("Project review of main.rs."),
        "{blocks:?}"
    );

    h.command("status", "");
    let status = h.command_output("status").await;
    assert!(status.contains("test-model"), "{status}");
}
