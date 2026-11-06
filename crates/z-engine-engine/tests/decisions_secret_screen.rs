//! `decisions_secret_screen` end to end with the decision model down: the
//! pattern detectors still find a key in a tool result, the user is asked
//! before the next request sends it, and a refusal keeps it out of every
//! later request.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness};
use z_engine_protocol::{ApprovalDecision, Command};
use z_engine_testkit::{FixtureRepo, Script};

const KEY: &str = "AKIAIOSFODNN7REALKEY";
const MASKED: &str = "[secret withheld by the user: AWS access key]";

fn screened() -> String {
    format!(
        "{BASE_SETTINGS}\n[experimental]\ndecisions_secret_screen = \"on\"\n\n\
         [decisions]\nendpoint = \"http://127.0.0.1:9\"\ntimeout_ms = 50\n"
    )
}

/// The main requests of a turn that reads a file holding `KEY`, with the
/// approval card answered by `decision`.
async fn requests(decision: ApprovalDecision) -> Vec<String> {
    let config = format!("region = eu-west-1\naws_access_key_id = {KEY}\n");
    let repo = FixtureRepo::git(&[("config.txt", config.as_str())]);
    let mut h = Harness::builder(repo).settings(&screened()).start().await;
    h.model.push(Script::tool(
        "Read",
        json!({ "file_path": h.path("config.txt") }),
    ));
    h.model.push(Script::tool(
        "Read",
        json!({ "file_path": h.path("config.txt"), "offset": 2 }),
    ));
    h.model.push(Script::text("The region is eu-west-1."));
    h.submit("which region is configured?");
    let request = h.approval().await;
    assert!(
        request.title.contains("possible secret"),
        "{}",
        request.title
    );
    h.send(Command::ResolveApproval {
        request_id: request.request_id,
        decision,
    });
    h.turn_finished().await;
    h.main_requests()
        .iter()
        .map(|request| serde_json::to_string(&request.messages).unwrap())
        .collect()
}

#[tokio::test]
async fn a_refused_key_never_reaches_the_model() {
    let sent = requests(ApprovalDecision::Deny { feedback: None }).await;
    assert_eq!(sent.len(), 3);
    for request in &sent[1..] {
        assert!(!request.contains(KEY), "{request}");
        assert!(request.contains(MASKED), "{request}");
    }
}

#[tokio::test]
async fn an_allowed_key_is_sent_and_not_asked_about_again() {
    let sent = requests(ApprovalDecision::AllowOnce).await;
    assert_eq!(sent.len(), 3);
    assert!(sent[1..].iter().all(|request| request.contains(KEY)));
}
