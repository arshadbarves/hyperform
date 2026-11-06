//! More than 40 MCP tools: no MCP schemas are sent, a reminder lists every
//! tool, and `LoadMcpTools` makes the named ones callable from the next
//! request on (for this run only).

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, last_user_text, py_mcp_toml, results, skip, tool_names};
use z_engine_protocol::{Event, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

fn mcp_tools(names: &[String]) -> Vec<&String> {
    names
        .iter()
        .filter(|name| name.starts_with("mcp__"))
        .collect()
}

#[tokio::test]
async fn deferred_tools_load_on_demand() {
    let dir = tempfile::tempdir().unwrap();
    let Some(server) = py_mcp_toml(dir.path(), "py", 45) else {
        return skip("python3");
    };
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n{server}");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.expect(|e| matches!(e, Event::Notice { text, .. } if text.contains("MCP servers ready: py")))
        .await;
    h.model.push(Script::tool(
        "LoadMcpTools",
        json!({"names": ["mcp__py__t3", "nope"]}),
    ));
    h.model
        .push(Script::tool("mcp__py__t3", json!({"text": "x"})));
    h.model.push(Script::text("Done."));
    let turn = h.run_turn("use tool three").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);

    let requests = h.main_requests();
    assert_eq!(requests.len(), 3);
    let first = tool_names(&requests[0]);
    assert!(mcp_tools(&first).is_empty(), "{first:?}");
    assert!(first.iter().any(|tool| tool == "LoadMcpTools"));
    let listing = last_user_text(&requests[0]);
    assert!(listing.contains("- mcp__py__t44: Tool 44."), "{listing}");
    assert!(listing.contains("- mcp__py__grow: Adds a tool."));

    let loaded = results(requests[1].messages.last().unwrap());
    assert!(
        !loaded[0].1 && loaded[0].2.contains("mcp__py__t3"),
        "{loaded:?}"
    );
    assert!(loaded[0].2.contains("nope"));
    assert_eq!(mcp_tools(&tool_names(&requests[1])), ["mcp__py__t3"]);

    let called = results(requests[2].messages.last().unwrap());
    assert!(!called[0].1 && called[0].2.contains("t3: x"), "{called:?}");
    assert!(
        !last_user_text(&requests[1]).contains("mcp__py__t44"),
        "the listing is sent once per catalog version"
    );
}
