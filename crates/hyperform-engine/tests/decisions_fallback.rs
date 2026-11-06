//! Every decision feature on with a decision model that is down or never
//! answers in time behaves exactly like every feature off: over the same
//! scripted session (two tasks, approvals, a denied command, automatic
//! checks, a compaction, a question) the main model gets the same
//! requests, the same calls ask and finish the same way, and verification
//! ends the same. A model that fails must never change a turn.

mod laya;
mod support;

use laya::workout::{self, BASE, Workout};

/// Automatic checks in a trusted project, so check selection is consulted.
const CHECKED: &str = "\n[verification]\nmode = \"auto\"\nauto_checks = [\"test\"]\n\n\
                       [[verification.checks]]\nid = \"unit\"\ncommand = \"true\"\nkind = \"test\"\n";

async fn assert_like_off(failing_endpoint: &str) {
    let base = format!("{BASE}{CHECKED}");
    let mut off = workout::run(&base, true).await;
    let settings = laya::every_feature(&base, failing_endpoint, "on", 50);
    let mut on = workout::run(&settings, true).await;
    assert_same(&mut off, &mut on);
}

fn assert_same(off: &mut Workout, on: &mut Workout) {
    let verification = off.verification();
    assert!(verification[0].starts_with("Verified"), "{verification:?}");
    assert_eq!(on.verification(), verification);
    assert_eq!(on.asked(), off.asked());
    assert_eq!(on.statuses(), off.statuses());
    let (off_requests, on_requests) = (off.requests(), on.requests());
    let cleared = off_requests.iter().any(|r| r.contains("[cleared:"));
    assert!(cleared, "old results were cleared under pressure");
    assert_eq!(off_requests.len(), on_requests.len());
    for (index, (seen, expected)) in on_requests.iter().zip(&off_requests).enumerate() {
        assert_eq!(seen, expected, "main request {index}");
    }
}

#[tokio::test]
async fn a_dead_decision_model_changes_nothing() {
    assert_like_off("http://127.0.0.1:9").await;
}

#[tokio::test]
async fn a_decision_model_that_never_answers_changes_nothing() {
    let endpoint = laya::silent().await;
    assert_like_off(&endpoint).await;
}
