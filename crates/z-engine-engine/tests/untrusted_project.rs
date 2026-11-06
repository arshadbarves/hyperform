//! Hooks defined by a project run only after the workspace is trusted;
//! until then the user is told they are disabled.

mod support;

use support::{Harness, hook_script, hook_toml};
use z_engine_protocol::{Event, NoticeLevel, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

fn blocking_project(scripts: &std::path::Path) -> String {
    let block = hook_script(
        scripts,
        "block.sh",
        "cat > /dev/null\necho 'project says no' >&2\nexit 2\n",
    );
    format!(
        "schema = 2\n{}",
        hook_toml("UserPromptSubmit", None, &block)
    )
}

#[tokio::test]
async fn untrusted_project_hooks_are_ignored() {
    let scripts = tempfile::tempdir().unwrap();
    let builder =
        Harness::builder(FixtureRepo::empty()).project_settings(&blocking_project(scripts.path()));
    builder.model().push(Script::text("hello"));
    let mut h = builder.start().await;
    h.expect(|e| {
        matches!(
            e,
            Event::Notice { level: NoticeLevel::Warn, text } if text.contains("not trusted")
        )
    })
    .await;
    let turn = h.run_turn("hi").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);
    assert_eq!(h.events.count(|e| matches!(e, Event::HookRan { .. })), 0);
}

#[tokio::test]
async fn trusted_project_hooks_run() {
    let scripts = tempfile::tempdir().unwrap();
    let mut h = Harness::builder(FixtureRepo::empty())
        .project_settings(&blocking_project(scripts.path()))
        .trusted()
        .start()
        .await;
    h.submit("hi");
    h.wait(|e| {
        matches!(
            e,
            Event::Notice { level: NoticeLevel::Warn, text } if text.contains("project says no")
        )
    })
    .await;
    assert_eq!(
        h.events.count(|e| matches!(e, Event::TurnStarted { .. })),
        0
    );
    assert!(h.main_requests().is_empty());
}
