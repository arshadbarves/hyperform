//! `!cmd` runs without the model, reports its output, and counts as a
//! change for the next turn's verification badge.

mod support;

use support::Harness;
use z_engine_protocol::{Command, Event, VerificationOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn shell_command_output_and_mutation() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.send(Command::Shell {
        command: "touch made-by-shell.txt && echo from-shell".into(),
    });
    match h.wait(|e| matches!(e, Event::CommandOutput { .. })).await {
        Event::CommandOutput { name, markdown } => {
            assert_eq!(name, "shell");
            assert!(
                markdown.contains("from-shell") && markdown.contains("exit code 0"),
                "{markdown}"
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(h.repo.exists("made-by-shell.txt"));
    assert!(h.main_requests().is_empty(), "no model involved");

    h.model.push(Script::text("noted"));
    let turn = h.run_turn("what changed?").await;
    assert!(
        matches!(turn.verification, VerificationOutcome::Unverified { .. }),
        "{:?}",
        turn.verification
    );
}
