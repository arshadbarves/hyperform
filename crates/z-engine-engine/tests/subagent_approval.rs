//! A subagent's approval goes through the session broker labelled with
//! its agent id; while it waits the agent shows `Waiting`, and the answer
//! lets its call run.

mod support;

use serde_json::json;
use support::{Harness, agent_call, results, route_task, task_requests};
use z_engine_protocol::{AgentStatus, ApprovalDecision, Command, Event};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn subagent_approval_carries_its_agent_id() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    route_task(
        &h.model,
        "WRITE-TASK",
        vec![
            Script::tool(
                "Write",
                json!({ "file_path": "child.txt", "content": "from the child" }),
            ),
            Script::text("wrote child.txt"),
        ],
    );
    h.model.push(Script::tool(
        "Agent",
        agent_call("general", "write file", "WRITE-TASK create child.txt"),
    ));
    h.model.push(Script::text("the child wrote it"));
    h.submit("have a child write a file");

    let started = h.agent_started().await;
    let request = h.approval().await;
    assert_eq!(request.agent_id, started.agent_id);
    assert_eq!(request.tool, "Write");
    let agent = started.agent_id.clone();
    h.expect(|e| {
        matches!(e, Event::AgentUpdated { info }
            if info.agent_id == agent && info.status == AgentStatus::Waiting)
    })
    .await;
    h.send(Command::ResolveApproval {
        request_id: request.request_id,
        decision: ApprovalDecision::AllowOnce,
    });
    h.turn_finished().await;

    assert_eq!(h.repo.read("child.txt"), "from the child");
    let child = task_requests(&h.model, "WRITE-TASK");
    let answered = results(child[1].messages.last().unwrap());
    assert!(!answered[0].1, "{}", answered[0].2);
    let finished = h.agent_finished(&started.agent_id).await;
    assert_eq!(finished.status, AgentStatus::Completed);
    assert_eq!(finished.tool_calls, 1);
}
