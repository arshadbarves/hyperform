//! A server announcing `tools/list_changed` gets its tools listed again;
//! the new tool is offered from the next request on.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, py_mcp_toml, skip, tool_names};
use z_engine_protocol::Event;
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn list_changed_refreshes_the_registry() {
    let dir = tempfile::tempdir().unwrap();
    let Some(server) = py_mcp_toml(dir.path(), "py", 2) else {
        return skip("python3");
    };
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n{server}");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.expect(|e| matches!(e, Event::Notice { text, .. } if text.contains("MCP servers ready: py")))
        .await;
    h.model.push(Script::tool("mcp__py__grow", json!({})));
    h.model.push(Script::text("Grown."));
    h.run_turn("grow the server").await;
    h.expect(
        |e| matches!(e, Event::Notice { text, .. } if text.contains("MCP tools changed on py")),
    )
    .await;
    h.model.push(Script::text("Now there is more."));
    h.run_turn("what now?").await;

    let requests = h.main_requests();
    let before = tool_names(&requests[0]);
    let after = tool_names(requests.last().unwrap());
    assert!(before.iter().any(|tool| tool == "mcp__py__t1"));
    assert!(!before.iter().any(|tool| tool == "mcp__py__extra"));
    assert!(
        after.iter().any(|tool| tool == "mcp__py__extra"),
        "{after:?}"
    );
}
