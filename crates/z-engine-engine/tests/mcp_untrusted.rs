//! MCP servers defined by an untrusted project are not started; servers
//! from the user's settings still are.

mod support;

use support::{BASE_SETTINGS, Harness, fake_mcp_toml, skip, tool_names};
use z_engine_protocol::{Event, NoticeLevel};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn untrusted_project_servers_are_ignored() {
    let (Some(user), Some(project)) = (fake_mcp_toml("user", ""), fake_mcp_toml("proj", "")) else {
        return skip("zengine-fake-mcp");
    };
    let builder = Harness::builder(FixtureRepo::empty())
        .settings(&format!("{BASE_SETTINGS}{user}"))
        .project_settings(&format!("schema = 2\n{project}"));
    builder.model().push(Script::text("hello"));
    let mut h = builder.start().await;
    let ready = h
        .expect(|e| matches!(e, Event::Notice { text, .. } if text.contains("MCP servers ready")))
        .await;
    let Event::Notice { level, text } = ready else {
        unreachable!()
    };
    assert_eq!(level, NoticeLevel::Info);
    assert!(text.contains("user") && !text.contains("proj"), "{text}");
    h.run_turn("hi").await;
    let offered = tool_names(&h.main_requests()[0]);
    assert!(offered.iter().any(|tool| tool.starts_with("mcp__user__")));
    assert!(!offered.iter().any(|tool| tool.starts_with("mcp__proj__")));
}
