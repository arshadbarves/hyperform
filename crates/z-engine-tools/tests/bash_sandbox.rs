//! `Bash` with a sandbox profile: writes inside the project work, writes
//! elsewhere fail with a hint. Skips when the machine offers no sandbox.
#![cfg(unix)]

mod support;

use std::sync::Arc;

use serde_json::json;
use support::project;
use z_engine_host::SandboxProfile;
use z_engine_host::sandbox::{SandboxBackend, detect};
use z_engine_tools::builtin::BashTool;
use z_engine_tools::{ShellConfig, Tool, ToolCtx};

#[tokio::test]
async fn sandboxed_commands_write_only_inside_the_project() {
    if let SandboxBackend::Unavailable(reason) = detect() {
        eprintln!("skipping: no sandbox on this machine ({reason})");
        return;
    }
    let dir = project(&[]);
    let profile = SandboxProfile::for_workspace(dir.path(), &[], &[], false);
    let shell = ShellConfig::detect().with_sandbox(Some(profile));
    let mut ctx = ToolCtx::for_tests(dir.path());
    ctx.shell = Arc::new(shell);

    let inside = BashTool
        .call(json!({"command": "touch made.txt"}), &ctx)
        .await
        .unwrap();
    assert!(!inside.is_error, "{}", inside.text_content());
    assert!(dir.path().join("made.txt").is_file());

    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap());
    let target = home.join(format!("zengine-bash-denied-{}", ulid::Ulid::new()));
    let command = format!("touch '{}'", target.display());
    let outside = BashTool
        .call(json!({ "command": command }), &ctx)
        .await
        .unwrap();
    assert!(outside.is_error);
    assert!(!target.exists());
    let text = outside.text_content();
    assert!(text.contains("runs in a sandbox"), "{text}");
    assert!(text.ends_with("Exit code 1"), "{text}");
}

#[test]
fn effective_spec_is_the_plain_shell_without_a_sandbox() {
    let shell = ShellConfig::detect();
    assert_eq!(shell.effective_spec().unwrap(), shell.spec);
}
