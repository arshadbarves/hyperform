//! The turn badge in report mode: `Verified` after a passing check newer
//! than the last edit, `Unverified` when the edit came after the check,
//! `Failed` on a failing check; mode `off` is always `NotApplicable`.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness};
use z_engine_protocol::{TurnRecord, VerificationOutcome};
use z_engine_testkit::{FixtureRepo, Script};

fn settings(command: &str, mode: &str) -> String {
    format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n\n[verification]\nmode = \"{mode}\"\n\n[[verification.checks]]\nid = \"unit\"\ncommand = \"{command}\"\nkind = \"test\"\n"
    )
}

/// Runs one turn of `steps` (a `Write` of `a.txt` and/or a check run).
async fn turn(command: &str, mode: &str, steps: &[&str]) -> TurnRecord {
    let repo = FixtureRepo::git(&[("README.md", "demo\n")]);
    let mut h = Harness::builder(repo)
        .settings(&settings(command, mode))
        .start()
        .await;
    for step in steps {
        let script = match *step {
            "write" => Script::tool(
                "Write",
                json!({"file_path": h.path("a.txt"), "content": "changed\n"}),
            ),
            _ => Script::tool("Verify", json!({"action": "run", "check": "custom:unit"})),
        };
        h.model.push(script);
    }
    h.model.push(Script::text("Done."));
    h.run_turn("change it").await
}

#[tokio::test]
async fn passing_check_after_the_edit_verifies() {
    let turn = turn("true", "report", &["write", "check"]).await;
    assert!(
        matches!(&turn.verification, VerificationOutcome::Verified { checks } if checks.len() == 1),
        "{:?}",
        turn.verification
    );
}

#[tokio::test]
async fn edit_after_the_check_is_unverified() {
    let turn = turn("true", "report", &["check", "write"]).await;
    assert!(
        matches!(&turn.verification, VerificationOutcome::Unverified { .. }),
        "{:?}",
        turn.verification
    );
}

#[tokio::test]
async fn failing_check_fails() {
    let turn = turn("exit 3", "report", &["write", "check"]).await;
    let VerificationOutcome::Failed { reason } = &turn.verification else {
        panic!("{:?}", turn.verification);
    };
    assert!(reason.contains("exit 3"), "{reason}");
}

#[tokio::test]
async fn off_mode_is_not_applicable() {
    let turn = turn("true", "off", &["write"]).await;
    assert_eq!(turn.verification, VerificationOutcome::NotApplicable);
}
