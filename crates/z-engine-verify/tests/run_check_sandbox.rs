//! Checks in the sandbox: a check may write inside the project but not
//! elsewhere. Skips when the machine offers no sandbox.
#![cfg(unix)]

use std::path::PathBuf;

use tokio_util::sync::CancellationToken;
use z_engine_host::sandbox::{SandboxBackend, detect};
use z_engine_host::{EnvPolicy, SandboxProfile, resolve_shell};
use z_engine_protocol::{AgentId, CheckKind};
use z_engine_verify::{CheckEnv, CheckSource, CheckSpec, run_check};

fn spec(command: String) -> CheckSpec {
    CheckSpec {
        id: "custom:sandboxed".to_string(),
        label: "sandboxed".to_string(),
        kind: CheckKind::Custom,
        command,
        cwd: PathBuf::from("."),
        source: CheckSource::Configured,
        timeout_secs: 30,
    }
}

#[tokio::test]
async fn sandboxed_checks_cannot_write_outside_the_project() {
    if let SandboxBackend::Unavailable(reason) = detect() {
        eprintln!("skipping: no sandbox on this machine ({reason})");
        return;
    }
    let project = tempfile::tempdir().unwrap();
    let session = tempfile::tempdir().unwrap();
    let env = CheckEnv {
        shell: resolve_shell(None),
        env: EnvPolicy::default(),
        agent_id: AgentId::main(),
        artifacts_dir: session.path().join("artifacts"),
        project_root: project.path().to_path_buf(),
        sandbox: Some(SandboxProfile::for_workspace(
            project.path(),
            &[],
            &[],
            false,
        )),
    };
    let inside = run_check(
        &spec("touch out.txt".into()),
        &env,
        CancellationToken::new(),
        None,
    )
    .await
    .unwrap();
    assert!(inside.passed, "{}", inside.output_tail);
    assert!(project.path().join("out.txt").is_file());

    let home = PathBuf::from(std::env::var("HOME").unwrap());
    let target = home.join(format!("zengine-check-denied-{}", ulid::Ulid::new()));
    let command = format!("touch '{}'", target.display());
    let outside = run_check(&spec(command), &env, CancellationToken::new(), None)
        .await
        .unwrap();
    assert!(!outside.passed);
    assert!(!target.exists());
    assert!(
        outside.output_tail.contains("Operation not permitted")
            || outside.output_tail.contains("Read-only file system"),
        "{}",
        outside.output_tail
    );
}
