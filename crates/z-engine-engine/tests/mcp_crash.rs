//! An MCP server that crashes during a call: the call's result is an error
//! (never an empty success), the server's tools stay callable, and the
//! next call reconnects.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, fake_mcp_toml, results, skip, tool_names};
use z_engine_protocol::{Event, NoticeLevel, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn a_crash_is_an_error_result_and_the_next_call_reconnects() {
    let Some(server) = fake_mcp_toml("fake", "") else {
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
    h.model.push(Script::tool("mcp__fake__crash", json!({})));
    h.model
        .push(Script::tool("mcp__fake__echo", json!({"text": "back"})));
    h.model.push(Script::text("Done."));
    let turn = h.run_turn("crash it, then echo").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);

    let requests = h.main_requests();
    let crashed = results(requests[1].messages.last().unwrap());
    assert!(crashed[0].1, "a crash must be an error: {crashed:?}");
    assert!(!crashed[0].2.trim().is_empty(), "{crashed:?}");
    assert!(
        tool_names(&requests[1])
            .iter()
            .any(|tool| tool == "mcp__fake__echo"),
        "a crashed server's tools stay offered so a call can reconnect"
    );
    let echoed = results(requests[2].messages.last().unwrap());
    assert!(
        !echoed[0].1 && echoed[0].2.contains("echo: back"),
        "{echoed:?}"
    );
}
