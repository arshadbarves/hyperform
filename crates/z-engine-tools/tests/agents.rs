//! `Agent` and `ApplyAgentChanges` through a fake agent port.

mod support;

use std::sync::Arc;

use serde_json::json;
use support::fakes::FakeAgents;
use support::{ctx_with, err_text, ok_text, project};
use z_engine_policy::Action;
use z_engine_protocol::{AgentId, Isolation, JobId};
use z_engine_tools::builtin::{AgentTool, ApplyAgentChangesTool};
use z_engine_tools::{AgentCard, Ports, Tool, ToolCtx};

fn catalog() -> Vec<AgentCard> {
    vec![
        AgentCard {
            name: "explore".into(),
            description: "Fast, read-only search.".into(),
            tools: "Read, Glob, Grep".into(),
        },
        AgentCard {
            name: "general".into(),
            description: "Full tool access.".into(),
            tools: "*".into(),
        },
    ]
}

fn setup(port: Arc<FakeAgents>) -> (tempfile::TempDir, ToolCtx) {
    let dir = project(&[]);
    let ctx = ctx_with(
        dir.path(),
        Ports {
            agents: Some(port),
            ..Ports::default()
        },
    );
    (dir, ctx)
}

#[test]
fn description_lists_the_catalog_captured_at_construction() {
    let description = AgentTool::new(catalog()).description();
    assert!(description.contains("- explore: Fast, read-only search. (Tools: Read, Glob, Grep)"));
    assert!(description.contains("- general: Full tool access. (Tools: *)"));
    assert!(!description.contains("{{"));
    assert!(
        AgentTool::new(Vec::new())
            .description()
            .contains("(no agent types are available)")
    );
}

#[tokio::test]
async fn foreground_agents_return_their_report_and_id() {
    let port = Arc::new(FakeAgents {
        catalog: catalog(),
        ..FakeAgents::default()
    });
    let (_dir, ctx) = setup(Arc::clone(&port));
    let output = AgentTool::new(catalog())
        .call(
            json!({"description": "Find the parser", "prompt": "Where is parse()?", "subagent_type": "explore", "isolation": "worktree"}),
            &ctx,
        )
        .await
        .unwrap();
    let text = output.text_content();
    assert!(
        text.starts_with(
            "Found it in src/lib.rs:3.\n\n12 tool calls, 3.1k tokens\nagent_id: agt_child"
        ),
        "{text}"
    );
    assert_eq!(output.summary, "explore agent finished");
    let requests = port.requests.lock().unwrap();
    assert_eq!(requests[0].agent_type, "explore");
    assert_eq!(requests[0].isolation, Some(Isolation::Worktree));
    assert!(!requests[0].background && requests[0].resume.is_none());
}

#[tokio::test]
async fn background_and_resumed_agents() {
    let port = Arc::new(FakeAgents {
        catalog: catalog(),
        background_job: Some(JobId::from("job_7")),
        ..FakeAgents::default()
    });
    let (_dir, ctx) = setup(Arc::clone(&port));
    let text = ok_text(
        &AgentTool::new(catalog()),
        &ctx,
        json!({"description": "Run suite", "prompt": "Continue", "subagent_type": "general",
               "run_in_background": true, "resume": "agt_old"}),
    )
    .await;
    assert!(
        text.contains("agent_id: agt_child\njob_id: job_7"),
        "{text}"
    );
    let requests = port.requests.lock().unwrap();
    assert!(requests[0].background);
    assert_eq!(requests[0].resume, Some(AgentId::from("agt_old")));
}

#[tokio::test]
async fn unknown_types_and_bad_isolation_are_rejected() {
    let port = Arc::new(FakeAgents {
        catalog: catalog(),
        ..FakeAgents::default()
    });
    let (_dir, ctx) = setup(port);
    let tool = AgentTool::new(catalog());
    let err = err_text(
        &tool,
        &ctx,
        json!({"description": "x", "prompt": "y", "subagent_type": "wizard"}),
    )
    .await;
    assert!(
        err.contains("unknown subagent_type \"wizard\"; available types: explore, general"),
        "{err}"
    );
    let err = err_text(
        &tool,
        &ctx,
        json!({"description": "x", "prompt": "y", "subagent_type": "explore", "isolation": "vm"}),
    )
    .await;
    assert!(
        err.contains("`isolation` must be shared or worktree"),
        "{err}"
    );
}

#[tokio::test]
async fn agents_are_parallel_and_gated_as_agent_actions() {
    let (_dir, ctx) = setup(Arc::new(FakeAgents::default()));
    let tool = AgentTool::new(catalog());
    let input = json!({"description": "Find it", "prompt": "p", "subagent_type": "explore"});
    assert!(tool.is_concurrency_safe(&input) && !tool.is_read_only(&input));
    assert_eq!(
        tool.action(&input, &ctx),
        Action::Agent {
            agent_type: "explore".into()
        }
    );
    assert_eq!(tool.title(&input, &ctx), "explore: Find it");
}

#[tokio::test]
async fn apply_agent_changes_merges_through_the_port() {
    let port = Arc::new(FakeAgents::default());
    let (dir, ctx) = setup(Arc::clone(&port));
    let output = ApplyAgentChangesTool
        .call(json!({"agent_id": "agt_child"}), &ctx)
        .await
        .unwrap();
    assert_eq!(output.text_content(), "Applied 2 files from agt_child");
    assert_eq!(
        *port.applied.lock().unwrap(),
        vec![AgentId::from("agt_child")]
    );
    assert_eq!(
        ApplyAgentChangesTool.action(&json!({}), &ctx),
        Action::Write {
            paths: vec![dir.path().to_path_buf()]
        }
    );
    assert!(!ApplyAgentChangesTool.is_concurrency_safe(&json!({})));
}
