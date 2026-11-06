//! An untrusted project that defines hooks asks for trust when a session
//! opens; trusting it records the root and enables the project's hooks.

mod support;

use support::{Harness, hook_script, hook_toml};
use z_engine_config::TrustStore;
use z_engine_protocol::{Command, Event, NoticeLevel};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn trusting_the_workspace_enables_project_hooks() {
    let scripts = tempfile::tempdir().unwrap();
    let block = hook_script(
        scripts.path(),
        "block.sh",
        "cat > /dev/null\necho 'project says no' >&2\nexit 2\n",
    );
    let project = format!(
        "schema = 2\n{}",
        hook_toml("UserPromptSubmit", None, &block)
    );
    let builder = Harness::builder(FixtureRepo::empty()).project_settings(&project);
    builder.model().push(Script::text("hello"));
    let mut h = builder.start().await;
    let (root, defines) = match h.expect(|e| matches!(e, Event::TrustRequired { .. })).await {
        Event::TrustRequired {
            project_root,
            defines,
        } => (project_root, defines),
        other => panic!("{other:?}"),
    };
    assert_eq!(defines, ["hooks"]);
    let canonical = std::fs::canonicalize(h.repo.path()).unwrap();
    assert_eq!(root, canonical.to_string_lossy());

    h.send(Command::TrustWorkspace { trusted: false });
    h.notice("stays untrusted").await;
    h.run_turn("still allowed").await;

    h.send(Command::TrustWorkspace { trusted: true });
    h.notice("Trusted").await;
    assert!(
        TrustStore::load(&h.paths.trust_file)
            .unwrap()
            .is_trusted(h.repo.path())
    );
    h.submit("now blocked");
    h.wait(|e| {
        matches!(
            e,
            Event::Notice { level: NoticeLevel::Warn, text } if text.contains("project says no")
        )
    })
    .await;
    assert_eq!(h.main_requests().len(), 1);
}
