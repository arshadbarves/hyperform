//! `agents.max_depth`: an agent at the limit is not offered `Agent`, and a
//! limit of 0 disables subagents for the main agent too.

mod support;

use support::{BASE_SETTINGS, Harness, agent_call, route_task, task_requests, tool_names};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn no_agent_tool_at_the_depth_limit() {
    let settings = format!("{BASE_SETTINGS}\n[agents]\nmax_depth = 1\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    route_task(&h.model, "DEPTH-TASK", vec![Script::text("child done")]);
    h.model.push(Script::tool(
        "Agent",
        agent_call("general", "child", "DEPTH-TASK do a thing"),
    ));
    h.model.push(Script::text("ok"));
    h.run_turn("delegate").await;

    let main = tool_names(&h.main_requests()[0]);
    assert!(main.iter().any(|name| name == "Agent"), "{main:?}");
    let child = tool_names(&task_requests(&h.model, "DEPTH-TASK")[0]);
    assert!(!child.iter().any(|name| name == "Agent"), "{child:?}");
    assert!(child.iter().any(|name| name == "Write"), "{child:?}");
}

#[tokio::test]
async fn depth_zero_disables_subagents() {
    let settings = format!("{BASE_SETTINGS}\n[agents]\nmax_depth = 0\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::text("alone"));
    h.run_turn("hello").await;
    let main = tool_names(&h.main_requests()[0]);
    assert!(!main.iter().any(|name| name == "Agent"), "{main:?}");
}
