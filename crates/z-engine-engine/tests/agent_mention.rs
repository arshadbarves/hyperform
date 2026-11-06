//! `@agent-<name>` of a known agent type adds a reminder to use the Agent
//! tool with that subagent type; unknown names add nothing.

mod support;

use support::{Harness, user_blocks};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn known_agent_mentions_add_a_reminder() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::text("on it"));
    h.run_turn("@agent-explore find the parser, not @agent-nobody")
        .await;
    let blocks = user_blocks(&h.main_requests().pop().unwrap());
    let reminders: Vec<&String> = blocks
        .iter()
        .filter(|block| block.contains("subagent_type"))
        .collect();
    assert_eq!(reminders.len(), 1, "{blocks:?}");
    assert!(reminders[0].starts_with("<system-reminder>"));
    assert!(
        reminders[0].contains("subagent_type \"explore\""),
        "{}",
        reminders[0]
    );
}
