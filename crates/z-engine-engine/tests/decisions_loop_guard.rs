//! `decisions_loop_guard` end to end with the decision model down: its
//! rules still remind the agent that repeats the same call with the same
//! result, and a run that searches for different things gets no reminder.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, last_user_text};
use z_engine_testkit::{FixtureRepo, Script};

const REMINDER: &str = "with the same input 3 times";

fn guarded() -> String {
    format!(
        "{BASE_SETTINGS}\n[experimental]\ndecisions_loop_guard = \"on\"\n\n\
         [decisions]\nendpoint = \"http://127.0.0.1:9\"\ntimeout_ms = 50\n"
    )
}

/// The last user text of each main request of a turn that greps `patterns`.
async fn greps(patterns: &[&str]) -> Vec<String> {
    let repo = FixtureRepo::git(&[("notes.txt", "remember the milk\n")]);
    let mut h = Harness::builder(repo).settings(&guarded()).start().await;
    for pattern in patterns {
        h.model
            .push(Script::tool("Grep", json!({ "pattern": pattern })));
    }
    h.model.push(Script::text("Done."));
    h.run_turn("find the milk").await;
    h.main_requests().iter().map(last_user_text).collect()
}

#[tokio::test]
async fn the_third_identical_call_gets_a_reminder() {
    let texts = greps(&["milk", "milk", "milk"]).await;
    assert_eq!(texts.len(), 4);
    assert!(!texts[2].contains(REMINDER));
    assert!(texts[3].contains(REMINDER), "{}", texts[3]);
}

#[tokio::test]
async fn searching_for_different_things_gets_no_reminder() {
    let texts = greps(&["milk", "eggs", "bread"]).await;
    assert_eq!(texts.len(), 4);
    assert!(texts.iter().all(|text| !text.contains(REMINDER)));
}
