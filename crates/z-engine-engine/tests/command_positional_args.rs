//! `$1`..`$9` take shell-style words (quotes respected); `$ARGUMENTS` the
//! whole argument string.

mod support;

use support::{Harness, user_blocks};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn positional_placeholders_take_quoted_words() {
    let repo = FixtureRepo::empty();
    repo.write(
        ".z-engine/commands/fix.md",
        "---\nargument-hint: <issue> <file>\n---\nFix issue #$1 in $2 (all: $ARGUMENTS; none: [$3]).",
    );
    let mut h = Harness::start(repo).await;
    h.model.push(Script::text("fixed"));
    h.command("fix", r#"42 "src/my file.rs""#);
    h.turn_finished().await;
    let blocks = user_blocks(&h.main_requests().pop().unwrap());
    assert_eq!(blocks[0], r#"/fix 42 "src/my file.rs""#);
    assert!(
        blocks[1]
            .contains(r#"Fix issue #42 in src/my file.rs (all: 42 "src/my file.rs"; none: [])."#),
        "{}",
        blocks[1]
    );
}
