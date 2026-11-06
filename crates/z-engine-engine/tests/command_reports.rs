//! `/mcp`, `/doctor` and `/todos` answer with markdown reports.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, fake_mcp_toml, skip};
use z_engine_protocol::{Event, NoticeLevel};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn doctor_and_todos_report_the_session() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.command("doctor", "");
    let doctor = h.command_output("doctor").await;
    for needle in [
        "**Provider:**",
        "`test-model`",
        "**Model catalog:**",
        "**git:**",
        "**ripgrep:**",
        "**Language servers:**",
        "**MCP servers:** none",
        "**Hooks:** 0 configured",
        "**Workspace:** not trusted",
        "**Verification:** mode `report`",
    ] {
        assert!(doctor.contains(needle), "{needle} missing from:\n{doctor}");
    }

    h.command("todos", "");
    assert!(h.command_output("todos").await.contains("No todos yet"));
    h.model.push(Script::tool(
        "TodoWrite",
        json!({ "todos": [
            { "content": "Write code", "activeForm": "Writing code", "status": "completed" },
            { "content": "Test it", "activeForm": "Testing it", "status": "in_progress" }
        ]}),
    ));
    h.model.push(Script::text("planned"));
    h.run_turn("plan it").await;
    h.command("todos", "");
    let todos = h.command_output("todos").await;
    assert!(todos.contains("- [x] Write code"), "{todos}");
    assert!(todos.contains("- [ ] **Test it** (in progress)"), "{todos}");
}

#[tokio::test]
async fn mcp_reports_server_states() {
    let Some(server) = fake_mcp_toml("fake", "") else {
        return skip("zengine-fake-mcp");
    };
    let broken = "\n[mcp.servers.broken]\ncommand = \"zengine-no-such-server\"\ntimeout_secs = 5\n";
    let settings = format!("{BASE_SETTINGS}{server}{broken}");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.expect(|e| {
        matches!(e, Event::Notice { level: NoticeLevel::Warn, text } if text.contains("failed to start"))
    })
    .await;
    h.command("mcp", "");
    let report = h.command_output("mcp").await;
    assert!(report.contains("| fake | ready |"), "{report}");
    assert!(report.contains("| broken | failed |"), "{report}");
    assert!(report.contains("**broken**:"), "{report}");
}
