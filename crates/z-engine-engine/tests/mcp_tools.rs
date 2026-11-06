//! MCP servers from settings connect in the background; their tools are
//! offered as `mcp__server__tool` (minus `disabled_tools`), callable, and
//! a tool-reported error reaches the model as an error result. Resources
//! are listed and read once a server is ready.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, fake_mcp_toml, results, skip, tool_names};
use z_engine_protocol::{Event, NoticeLevel, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn mcp_tools_are_registered_and_callable() {
    let Some(server) = fake_mcp_toml("fake", "disabled_tools = [\"crash\"]") else {
        return skip("zengine-fake-mcp");
    };
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n{server}");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.expect(|e| {
        matches!(e, Event::Notice { level: NoticeLevel::Info, text } if text.contains("MCP servers ready: fake"))
    })
    .await;
    h.model.push(Script::tools(&[
        ("mcp__fake__echo", json!({"text": "hi"})),
        ("mcp__fake__fail", json!({})),
    ]));
    h.model.push(Script::tools(&[
        ("ListMcpResources", json!({})),
        (
            "ReadMcpResource",
            json!({"server": "fake", "uri": "fake://readme"}),
        ),
    ]));
    h.model.push(Script::text("Done."));
    let turn = h.run_turn("use the server").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);

    let requests = h.main_requests();
    let offered = tool_names(&requests[0]);
    for name in ["mcp__fake__echo", "mcp__fake__fail", "ListMcpResources"] {
        assert!(offered.iter().any(|tool| tool == name), "{name} missing");
    }
    assert!(!offered.iter().any(|tool| tool == "mcp__fake__crash"));
    assert!(!offered.iter().any(|tool| tool == "LoadMcpTools"));

    let called = results(requests[1].messages.last().unwrap());
    assert_eq!(called.len(), 2);
    assert!(
        !called[0].1 && called[0].2.contains("echo: hi"),
        "{called:?}"
    );
    assert!(called[1].1, "isError must be surfaced: {called:?}");
    assert!(called[1].2.contains("failed on purpose"));

    let resources = results(requests[2].messages.last().unwrap());
    assert!(!resources[0].1);
    assert!(
        resources[0]
            .2
            .contains("fake: fake://readme (readme) [text/markdown] - The readme"),
        "{resources:?}"
    );
    assert!(resources[1].2.contains("# Fake readme"), "{resources:?}");
}
