//! MCP: the tool adapter (naming, gating, concurrency, calls) and the
//! resource tools.

mod support;

use std::sync::Arc;

use serde_json::json;
use support::fakes::FakeMcp;
use support::{ctx_with, ok_text, project};
use z_engine_policy::Action;
use z_engine_tools::builtin::{ListMcpResourcesTool, LoadMcpToolsTool, ReadMcpResourceTool};
use z_engine_tools::{Ports, Tool, ToolCtx, mcp_tool};

fn setup(port: Arc<FakeMcp>) -> (tempfile::TempDir, ToolCtx) {
    let dir = project(&[]);
    let ctx = ctx_with(
        dir.path(),
        Ports {
            mcp: Some(port),
            ..Ports::default()
        },
    );
    (dir, ctx)
}

#[tokio::test]
async fn adapter_names_gates_and_forwards_with_original_names() {
    let port = Arc::new(FakeMcp::default());
    let (_dir, ctx) = setup(Arc::clone(&port));
    let tool = mcp_tool(
        "git hub",
        "create.issue",
        "Create an issue.",
        json!({"properties": {"title": {"type": "string"}}}),
        false,
    );
    assert_eq!(tool.name(), "mcp__git_hub__create_issue");
    assert_eq!(tool.description(), "Create an issue.");
    assert_eq!(tool.input_schema()["type"], "object");
    let input = json!({"title": "Bug"});
    assert_eq!(
        tool.action(&input, &ctx),
        Action::Mcp {
            server: "git hub".into(),
            tool: "create.issue".into(),
            read_only: false
        }
    );
    assert!(!tool.is_read_only(&input) && !tool.is_concurrency_safe(&input));
    let output = tool.call(input.clone(), &ctx).await.unwrap();
    assert_eq!(output.text_content(), "issue #7 created\nmore");
    assert_eq!(output.summary, "issue #7 created");
    assert!(!output.is_error);
    assert_eq!(
        *port.calls.lock().unwrap(),
        vec![("git hub".to_string(), "create.issue".to_string(), input)]
    );
}

#[tokio::test]
async fn read_only_hints_make_tools_concurrent_and_errors_propagate() {
    let port = Arc::new(FakeMcp {
        is_error: true,
        ..FakeMcp::default()
    });
    let (_dir, ctx) = setup(port);
    let tool = mcp_tool("docs", "search", "", json!(null), true);
    assert!(tool.is_read_only(&json!({})) && tool.is_concurrency_safe(&json!({})));
    assert_eq!(tool.description(), "Tool search from the MCP server docs.");
    assert_eq!(tool.title(&json!({}), &ctx), "docs: search");
    let output = tool.call(json!({}), &ctx).await.unwrap();
    assert!(output.is_error);
}

#[tokio::test]
async fn resource_tools_list_and_read_through_the_port() {
    let port = Arc::new(FakeMcp::default());
    let (_dir, ctx) = setup(Arc::clone(&port));
    let text = ok_text(&ListMcpResourcesTool, &ctx, json!({})).await;
    assert_eq!(text, "docs: file:///readme (README)");
    ok_text(&ListMcpResourcesTool, &ctx, json!({"server": "docs"})).await;
    assert_eq!(
        *port.listed.lock().unwrap(),
        vec![None, Some("docs".to_string())]
    );
    let text = ok_text(
        &ReadMcpResourceTool,
        &ctx,
        json!({"server": "docs", "uri": "file:///readme"}),
    )
    .await;
    assert_eq!(text, "docs:file:///readme body");

    let resources = |server: &str| Action::Mcp {
        server: server.into(),
        tool: "resources".into(),
        read_only: true,
    };
    assert_eq!(
        ListMcpResourcesTool.action(&json!({}), &ctx),
        resources("*")
    );
    assert_eq!(
        ReadMcpResourceTool.action(&json!({"server": "docs", "uri": "u"}), &ctx),
        resources("docs")
    );
    assert!(ReadMcpResourceTool.is_concurrency_safe(&json!({})));
}

#[tokio::test]
async fn load_mcp_tools_validates_names_and_needs_deferral() {
    let port = Arc::new(FakeMcp::default());
    let (_dir, ctx) = setup(port);
    let tool = LoadMcpToolsTool;
    let input = json!({"names": ["mcp__docs__search"]});
    assert_eq!(tool.name(), "LoadMcpTools");
    assert_eq!(tool.action(&input, &ctx), Action::Other { read_only: true });
    assert!(tool.is_read_only(&input) && tool.is_concurrency_safe(&input));
    assert_eq!(tool.title(&input, &ctx), "Load 1 MCP tool");
    assert!(tool.call(json!({"names": [" "]}), &ctx).await.is_err());
    let refused = tool.call(input, &ctx).await.unwrap_err();
    assert!(refused.to_string().contains("not deferred"), "{refused}");
}
