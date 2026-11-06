//! Custom agents from `.hyperform/agents`: the tool allowlist decides what
//! the agent is offered (anything else is an unknown tool),
//! `permissionMode: plan` denies writes even when the session bypasses, a
//! planning session plans for every agent, and a reload picks up new types.

mod support;

use serde_json::json;
use support::{Harness, agent_call, results, route_task, task_requests, tool_names, write_agent};
use z_engine_protocol::{AgentStatus, Command, Event};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn allowlist_limits_the_tools() {
    let repo = FixtureRepo::empty();
    write_agent(
        &repo,
        "auditor",
        "tools: Read, Grep\n",
        "AUDITOR-PROMPT audit code.",
    );
    let mut h = Harness::start(repo).await;
    route_task(
        &h.model,
        "AUDIT-TASK",
        vec![
            Script::tool("Write", json!({ "file_path": "x.txt", "content": "x" })),
            Script::text("could not write"),
        ],
    );
    h.model.push(Script::tool(
        "Agent",
        agent_call("auditor", "audit", "AUDIT-TASK audit the tree"),
    ));
    h.model.push(Script::text("audited"));
    h.run_turn("audit").await;

    let description = h.main_requests()[0]
        .tools
        .iter()
        .find(|tool| tool.name == "Agent")
        .unwrap()
        .description
        .clone();
    assert!(
        description.contains("- auditor: Test agent auditor."),
        "{description}"
    );

    let child = task_requests(&h.model, "AUDIT-TASK");
    assert!(child[0].system_text().contains("AUDITOR-PROMPT"));
    let mut offered = tool_names(&child[0]);
    offered.sort();
    assert_eq!(offered, ["Grep", "Read"]);
    let answered = results(child[1].messages.last().unwrap());
    assert!(answered[0].1);
    assert!(
        answered[0].2.contains("Unknown tool `Write`"),
        "{}",
        answered[0].2
    );
    assert!(!h.repo.exists("x.txt"));
}

#[tokio::test]
async fn plan_mode_agent_cannot_write() {
    let repo = FixtureRepo::empty();
    write_agent(&repo, "planner", "permissionMode: plan\n", "Plan things.");
    let mut h = Harness::start(repo).await;
    route_task(
        &h.model,
        "PLAN-TASK",
        vec![
            Script::tool("Write", json!({ "file_path": "plan.txt", "content": "p" })),
            Script::text("plan delivered"),
        ],
    );
    h.model.push(Script::tool(
        "Agent",
        agent_call("planner", "plan", "PLAN-TASK plan it"),
    ));
    h.model.push(Script::text("planned"));
    h.run_turn("plan").await;

    let child = task_requests(&h.model, "PLAN-TASK");
    let answered = results(child[1].messages.last().unwrap());
    assert!(answered[0].1);
    assert!(
        answered[0].2.contains("Permission denied"),
        "{}",
        answered[0].2
    );
    assert!(!h.repo.exists("plan.txt"));
    let agent = h.agent_started().await.agent_id;
    assert_eq!(
        h.agent_finished(&agent).await.status,
        AgentStatus::Completed
    );
}

/// Runs `agent` on a task that writes `file`; returns the write's result.
async fn agent_writes(
    h: &mut Harness,
    agent: &str,
    task: &'static str,
    file: &str,
) -> (bool, String) {
    route_task(
        &h.model,
        task,
        vec![
            Script::tool("Write", json!({ "file_path": file, "content": "p" })),
            Script::text("done"),
        ],
    );
    h.model.push(Script::tool(
        "Agent",
        agent_call(agent, "write", &format!("{task} write it")),
    ));
    h.model.push(Script::text("delegated"));
    h.run_turn("delegate").await;
    let child = task_requests(&h.model, task);
    let answered = results(child[1].messages.last().unwrap());
    (answered[0].1, answered[0].2.clone())
}

#[tokio::test]
async fn bypass_session_overrides_the_agent_level_but_not_its_plan_mode() {
    let repo = FixtureRepo::empty();
    write_agent(&repo, "asker", "permissionMode: default\n", "Ask first.");
    write_agent(&repo, "planner", "permissionMode: plan\n", "Plan things.");
    let settings = format!(
        "{}\n[permissions]\nmode = \"bypass\"\n",
        support::BASE_SETTINGS
    );
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    let (failed, text) = agent_writes(&mut h, "asker", "ASK-TASK", "asked.txt").await;
    assert!(!failed, "{text}");
    assert!(h.repo.exists("asked.txt"));
    let (failed, text) = agent_writes(&mut h, "planner", "PLAN-TASK", "plan.txt").await;
    assert!(failed && text.contains("plan mode"), "{text}");
    assert!(!h.repo.exists("plan.txt"));
}

#[tokio::test]
async fn every_agent_plans_while_the_session_does() {
    let repo = FixtureRepo::empty();
    write_agent(&repo, "editor", "permissionMode: acceptEdits\n", "Edit.");
    let settings = format!(
        "{}\n[permissions]\nmode = \"acceptEdits\"\nplanning = true\n",
        support::BASE_SETTINGS
    );
    let mut h = Harness::builder(repo)
        .trusted()
        .settings(&settings)
        .start()
        .await;
    let (failed, text) = agent_writes(&mut h, "editor", "EDIT-TASK", "edit.txt").await;
    assert!(failed && text.contains("plan mode"), "{text}");
    assert!(!h.repo.exists("edit.txt"));
}

#[tokio::test]
async fn reloading_extensions_refreshes_the_agent_types() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    write_agent(&h.repo, "latecomer", "", "Arrived later.");
    h.send(Command::ReloadExtensions);
    h.wait(|e| matches!(e, Event::Snapshot { .. })).await;
    h.model.push(Script::text("ok"));
    h.run_turn("hello").await;
    let agent = h.main_requests()[0]
        .tools
        .iter()
        .find(|tool| tool.name == "Agent")
        .unwrap()
        .description
        .clone();
    assert!(
        agent.contains("- latecomer: Test agent latecomer."),
        "{agent}"
    );
}

#[tokio::test]
async fn unknown_agent_type_lists_the_available_ones() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::tool(
        "Agent",
        agent_call("wizard", "magic", "do magic"),
    ));
    h.model.push(Script::text("ok"));
    h.run_turn("magic").await;
    let answered = results(h.main_requests()[1].messages.last().unwrap());
    assert!(answered[0].1);
    assert!(
        answered[0].2.contains("unknown subagent_type"),
        "{}",
        answered[0].2
    );
    assert!(
        answered[0].2.contains("general, explore"),
        "{}",
        answered[0].2
    );
}
