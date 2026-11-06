//! The per-run turn budget stops a run as `BudgetExhausted` after the
//! results of its last round are recorded.

mod support;

use serde_json::json;
use support::{Harness, assert_valid_transcript};
use z_engine_protocol::TurnOutcome;
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn max_turns_ends_the_run() {
    let settings = "schema = 2\n\n[model]\nmain = \"test-model\"\n\n[agents]\nmax_turns = 1\n";
    let repo = FixtureRepo::git(&[("a.txt", "a\n")]);
    let mut h = Harness::builder(repo).settings(settings).start().await;
    h.model.push(Script::tool(
        "Read",
        json!({ "file_path": h.path("a.txt") }),
    ));
    let turn = h.run_turn("keep going").await;
    match &turn.outcome {
        TurnOutcome::BudgetExhausted { reason } => {
            assert!(reason.contains("1 model turns"), "{reason}")
        }
        other => panic!("expected an exhausted budget, got {other:?}"),
    }
    assert_eq!(h.main_requests().len(), 1);
    assert_valid_transcript(&h.transcript());
}
