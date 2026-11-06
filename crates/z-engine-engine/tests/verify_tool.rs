//! `Verify` lists the session's checks and runs one: the record is kept
//! (and survives a reopen), announced with `CheckRecorded` and a new
//! badge, and its full output is stored outside the project.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, results};
use z_engine_protocol::{AgentId, Event, VerificationOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn verify_lists_and_runs_checks_as_evidence() {
    let settings = format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n\n[[verification.checks]]\nid = \"unit\"\nlabel = \"Unit tests\"\ncommand = \"echo all good\"\nkind = \"test\"\n"
    );
    let repo = FixtureRepo::git(&[("README.md", "demo\n")]);
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    h.model
        .push(Script::tool("Verify", json!({"action": "list"})));
    h.model.push(Script::tool(
        "Verify",
        json!({"action": "run", "check": "custom:unit"}),
    ));
    h.model.push(Script::text("Checked."));
    let turn = h.run_turn("run the tests").await;
    assert_eq!(turn.verification, VerificationOutcome::NotApplicable);

    let requests = h.main_requests();
    let listed = results(requests[1].messages.last().unwrap());
    assert!(
        listed[0].2.contains("| custom:unit | test | Unit tests |"),
        "{listed:?}"
    );
    let ran = results(requests[2].messages.last().unwrap());
    assert!(
        !ran[0].1 && ran[0].2.starts_with("PASS: Unit tests"),
        "{ran:?}"
    );

    let recorded = h.expect(|e| matches!(e, Event::CheckRecorded { .. })).await;
    let Event::CheckRecorded { record } = recorded else {
        unreachable!()
    };
    assert!(record.passed && record.agent_id == AgentId::main());
    let artifact = record.artifact.clone().expect("an artifact");
    assert!(
        !artifact.starts_with(&h.path("")),
        "artifacts stay outside the project"
    );
    assert!(
        std::fs::read_to_string(&artifact)
            .unwrap()
            .contains("all good")
    );
    assert!(
        h.events
            .count(|e| matches!(e, Event::VerificationChanged { .. }))
            >= 1
    );

    h.reopen().await;
    let snapshot = h
        .wait(|e| matches!(e, Event::Snapshot { snapshot } if !snapshot.checks.is_empty()))
        .await;
    let Event::Snapshot { snapshot } = snapshot else {
        unreachable!()
    };
    assert_eq!(snapshot.checks[0].record_id, record.record_id);
}
