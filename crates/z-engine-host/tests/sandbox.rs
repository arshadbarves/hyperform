//! The command sandbox on this machine: writes inside the profile succeed,
//! writes elsewhere fail, the network is cut off except localhost, and
//! wrapped shells keep cwd tracking and work as background shells. Tests
//! skip (with a note) when the machine offers no sandbox.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use z_engine_host::sandbox::{SandboxBackend, detect, wrap};
use z_engine_host::{
    BackgroundShells, BackgroundSpec, EnvPolicy, RunOutput, RunSpec, SandboxProfile, ShellSpec,
    resolve_shell, run,
};
use z_engine_protocol::JobStatus;

/// A root outside the temp directory, so its writability comes from the
/// profile rather than from the always-writable temp paths.
fn workspace() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

fn sandboxed(root: &Path, allow_network: bool) -> Option<ShellSpec> {
    let backend = detect();
    if let SandboxBackend::Unavailable(reason) = &backend {
        eprintln!("skipping: no sandbox on this machine ({reason})");
        return None;
    }
    let profile = SandboxProfile::for_workspace(root, &[], &[], allow_network);
    Some(wrap(&resolve_shell(None), &profile, &backend).unwrap())
}

async fn run_in(shell: &ShellSpec, command: &str, cwd: &Path) -> RunOutput {
    let mut spec = RunSpec::new(command, cwd);
    spec.shell = shell.clone();
    run(spec, CancellationToken::new(), None).await.unwrap()
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", ulid::Ulid::new().to_string().to_lowercase())
}

#[test]
fn detect_reports_the_platform_backend() {
    let backend = detect();
    eprintln!("sandbox backend: {backend:?}");
    if cfg!(target_os = "macos") && Path::new("/usr/bin/sandbox-exec").is_file() {
        assert!(
            matches!(
                backend,
                SandboxBackend::MacSeatbelt | SandboxBackend::Unavailable(_)
            ),
            "{backend:?}"
        );
    }
    if cfg!(target_os = "linux") && which::which("bwrap").is_err() {
        assert!(matches!(backend, SandboxBackend::Unavailable(_)));
    }
}

#[tokio::test]
async fn writes_are_confined_to_the_profile() {
    let root = workspace();
    let Some(shell) = sandboxed(root.path(), true) else {
        return;
    };
    let ok = run_in(&shell, "touch ok && echo made", root.path()).await;
    assert_eq!(ok.exit_code, Some(0), "{}", ok.combined);
    assert!(root.path().join("ok").is_file());

    let home = dirs::home_dir().unwrap();
    let denied: PathBuf = home.join(unique("zengine-sandbox-denied"));
    let sibling = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(unique("zengine-sibling"));
    for target in [&denied, &sibling] {
        let command = format!("touch '{}'", target.display());
        let out = run_in(&shell, &command, root.path()).await;
        assert_ne!(out.exit_code, Some(0), "{}", out.combined);
        assert!(!target.exists(), "{} was written", target.display());
        assert!(
            z_engine_host::is_sandbox_denial(&out.stderr),
            "{}",
            out.stderr
        );
    }

    let temp = run_in(&shell, "mktemp", root.path()).await;
    assert_eq!(temp.exit_code, Some(0), "{}", temp.combined);
    std::fs::remove_file(temp.stdout.trim()).unwrap();
}

#[tokio::test]
async fn protected_files_inside_the_root_stay_read_only() {
    let root = workspace();
    std::fs::create_dir_all(root.path().join(".git/hooks")).unwrap();
    let Some(shell) = sandboxed(root.path(), true) else {
        return;
    };
    let hook = run_in(&shell, "echo evil > .git/hooks/pre-commit", root.path()).await;
    assert_ne!(hook.exit_code, Some(0), "{}", hook.combined);
    assert!(!root.path().join(".git/hooks/pre-commit").exists());
    if cfg!(target_os = "macos") {
        let settings = "mkdir -p .z-engine && echo x > .z-engine/settings.toml";
        let out = run_in(&shell, settings, root.path()).await;
        assert_ne!(out.exit_code, Some(0), "{}", out.combined);
        assert!(!root.path().join(".z-engine/settings.toml").exists());
    }
    let index = run_in(&shell, "echo ok > .git/index", root.path()).await;
    assert_eq!(index.exit_code, Some(0), "{}", index.combined);
}

#[tokio::test]
async fn cwd_tracking_still_works_inside_the_sandbox() {
    let root = workspace();
    std::fs::create_dir(root.path().join("sub")).unwrap();
    let Some(shell) = sandboxed(root.path(), true) else {
        return;
    };
    let out = run_in(&shell, "cd sub", root.path()).await;
    assert_eq!(out.exit_code, Some(0), "{}", out.combined);
    let expected = std::fs::canonicalize(root.path().join("sub")).unwrap();
    let got = out.final_cwd.map(|dir| std::fs::canonicalize(dir).unwrap());
    assert_eq!(got, Some(expected));
}

#[tokio::test]
async fn network_is_blocked_except_localhost() {
    let root = workspace();
    let Some(shell) = sandboxed(root.path(), false) else {
        return;
    };
    if which::which("nc").is_err() {
        eprintln!("skipping: nc is not installed");
        return;
    }
    // bubblewrap's private network namespace has its own loopback, so only
    // Seatbelt reaches the host's localhost services.
    if cfg!(target_os = "macos") {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let local = run_in(&shell, &format!("nc -z 127.0.0.1 {port}"), root.path()).await;
        assert_eq!(local.exit_code, Some(0), "{}", local.combined);
    }

    let probe = "nc -z -w 3 1.1.1.1 443";
    let open = run_in(&resolve_shell(None), probe, root.path()).await;
    if open.exit_code != Some(0) {
        eprintln!("skipping the remote half: no outbound network here");
        return;
    }
    let blocked = run_in(&shell, probe, root.path()).await;
    assert_ne!(blocked.exit_code, Some(0), "{}", blocked.combined);
}

#[tokio::test]
async fn background_shells_accept_a_wrapped_shell() {
    let root = workspace();
    let Some(shell) = sandboxed(root.path(), true) else {
        return;
    };
    let shells = BackgroundShells::new(None);
    let outside = dirs::home_dir()
        .unwrap()
        .join(unique("zengine-sandbox-bg-denied"));
    let spec = BackgroundSpec {
        command: format!("touch inside; touch '{}'", outside.display()),
        cwd: root.path().to_path_buf(),
        shell,
        env: EnvPolicy::default(),
        label: "sandboxed".into(),
        owner: "main".into(),
    };
    let id = shells.spawn(spec).await.unwrap();
    let read = shells.wait(&id, Duration::from_secs(10)).await.unwrap();
    assert_eq!(read.status, JobStatus::Failed, "{}", read.output);
    assert!(root.path().join("inside").is_file());
    assert!(!outside.exists());
}
