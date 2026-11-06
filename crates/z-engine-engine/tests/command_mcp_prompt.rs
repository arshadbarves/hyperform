//! MCP prompts run as `mcp__<server>__<prompt>` commands: arguments map
//! positionally or as `key=value`, the rendered prompt becomes the body,
//! and missing required arguments are reported instead of sent.

mod support;

use support::{BASE_SETTINGS, Harness, fake_mcp_toml, skip, user_blocks};
use z_engine_protocol::{Event, NoticeLevel};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn mcp_prompts_are_commands() {
    let Some(server) = fake_mcp_toml("fake", "") else {
        return skip("zengine-fake-mcp");
    };
    let settings = format!("{BASE_SETTINGS}{server}");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.expect(|e| {
        matches!(e, Event::Notice { level: NoticeLevel::Info, text } if text.contains("MCP servers ready: fake"))
    })
    .await;
    let listed = h.engine.slash_commands(h.repo.path());
    let prompt = listed
        .iter()
        .find(|command| command.name == "mcp__fake__review")
        .expect("the MCP prompt is listed");
    assert_eq!(prompt.source, "mcp");
    assert_eq!(prompt.argument_hint.as_deref(), Some("<file>"));

    h.model.push(Script::text("reviewed"));
    h.command("mcp__fake__review", "src/lib.rs");
    h.turn_finished().await;
    let blocks = user_blocks(&h.main_requests().pop().unwrap());
    assert_eq!(blocks[0], "/mcp__fake__review src/lib.rs");
    assert!(blocks[1].contains("Please review src/lib.rs"), "{blocks:?}");

    h.model.push(Script::text("reviewed again"));
    h.command("mcp__fake__review", "file=main.rs");
    h.turn_finished().await;
    let blocks = user_blocks(&h.main_requests().pop().unwrap());
    assert!(blocks[1].contains("Please review main.rs"), "{blocks:?}");

    h.command("mcp__fake__review", "");
    let notice = h.notice("missing required argument").await;
    assert!(notice.contains("file"), "{notice}");
    assert_eq!(h.main_requests().len(), 2, "nothing was sent");
}
