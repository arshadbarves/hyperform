//! `resume` continues a finished agent from its stored transcript: the
//! follow-up is a new user message after the earlier exchange, under the
//! same agent id.

mod support;

use serde_json::json;
use support::{Harness, agent_call, all_text, results, route_task, task_requests};
use z_engine_protocol::{AgentStatus, Role};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn resume_continues_a_finished_agent() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    route_task(
        &h.model,
        "FIRST-TASK",
        vec![Script::text("first report"), Script::text("second report")],
    );
    h.model.push(Script::tool(
        "Agent",
        agent_call("general", "research", "FIRST-TASK look into it"),
    ));
    h.model.push(Script::text("ok"));
    h.run_turn("research").await;
    let agent = h.agent_started().await.agent_id;
    assert_eq!(
        h.agent_finished(&agent).await.status,
        AgentStatus::Completed
    );

    let mut follow_up = agent_call("general", "research", "FOLLOW-UP dig deeper");
    follow_up["resume"] = json!(agent);
    h.model.push(Script::tool("Agent", follow_up));
    h.model.push(Script::text("ok again"));
    h.run_turn("continue it").await;

    let resumed = task_requests(&h.model, "FIRST-TASK");
    assert_eq!(resumed.len(), 2);
    let messages = &resumed[1].messages;
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[1].role, Role::Assistant);
    assert!(messages[1].text().contains("first report"));
    assert!(all_text(&messages[2]).contains("FOLLOW-UP"));

    let answered = results(h.main_requests()[3].messages.last().unwrap());
    assert!(answered[0].2.contains("second report"), "{}", answered[0].2);
    assert!(answered[0].2.contains(agent.as_str()), "{}", answered[0].2);
    let transcript = h.engine.agent_transcript(&h.session, &agent).unwrap();
    assert_eq!(transcript.len(), 4);
    let info = h.agent_info(&agent);
    assert_eq!(info.status, AgentStatus::Completed);
    assert_eq!(info.usage.input_tokens, 200);
}
