//! One-shot shell runs: streaming, exit codes, deadlines, cancellation,
//! tree kills, the persistent working directory, and the environment.
#![cfg(unix)]

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use support::{canonical, eventually_dead, wait_for_file};
use tokio_util::sync::CancellationToken;
use z_engine_host::{
    EnvPolicy, HostError, OutputSink, RunSpec, ShellKind, ShellSpec, resolve_shell, run,
};

fn spec(command: &str, cwd: &Path) -> RunSpec {
    RunSpec::new(command, cwd)
}

async fn run_ok(spec: RunSpec) -> z_engine_host::RunOutput {
    run(spec, CancellationToken::new(), None).await.unwrap()
}

#[tokio::test]
async fn captures_exit_code_and_both_streams() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_ok(spec("echo out; echo err >&2; exit 3", dir.path())).await;
    assert_eq!(out.exit_code, Some(3));
    assert_eq!(out.stdout, "out\n");
    assert_eq!(out.stderr, "err\n");
    assert!(!out.timed_out && !out.cancelled && !out.truncated);
}

#[tokio::test]
async fn streams_lines_as_they_arrive_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let seen = Arc::new(Mutex::new(Vec::<(String, Instant)>::new()));
    let sink_seen = Arc::clone(&seen);
    let sink: OutputSink = Arc::new(move |line: &str| {
        sink_seen
            .lock()
            .unwrap()
            .push((line.to_string(), Instant::now()));
    });
    let command = "echo one; sleep 0.3; echo two >&2; sleep 0.3; echo three";
    let out = run(
        spec(command, dir.path()),
        CancellationToken::new(),
        Some(sink),
    )
    .await
    .unwrap();
    let seen = seen.lock().unwrap().clone();
    let lines: Vec<&str> = seen.iter().map(|(line, _)| line.as_str()).collect();
    assert_eq!(lines, ["one\n", "two\n", "three\n"]);
    // Delivered live, not batched at exit.
    assert!(seen[1].1.duration_since(seen[0].1) >= Duration::from_millis(200));
    assert_eq!(out.combined, "one\ntwo\nthree\n");
    assert_eq!(out.stdout, "one\nthree\n");
}

#[tokio::test]
async fn cd_persists_through_final_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_ok(spec(
        "mkdir -p sub/deeper && cd sub/deeper && echo moved",
        dir.path(),
    ))
    .await;
    let final_cwd = out.final_cwd.expect("tracked cwd");
    assert_eq!(
        canonical(&final_cwd),
        canonical(&dir.path().join("sub/deeper"))
    );
    assert_eq!(out.combined, "moved\n");

    let mut untracked = spec("cd sub", dir.path());
    untracked.track_cwd = false;
    assert_eq!(run_ok(untracked).await.final_cwd, None);

    // `exit` skips the probe: no directory, but the exit code is kept.
    let early = run_ok(spec("cd sub && exit 4", dir.path())).await;
    assert_eq!((early.exit_code, early.final_cwd), (Some(4), None));
}

#[tokio::test]
async fn timeout_kills_the_shell_and_its_grandchildren() {
    let dir = tempfile::tempdir().unwrap();
    let command = "sh -c 'echo $$ > grandchild.pid; exec sleep 30' & sleep 30";
    let mut slow = spec(command, dir.path());
    slow.timeout = Duration::from_secs(2);
    let started = Instant::now();
    let out = run_ok(slow).await;
    assert!(out.timed_out);
    assert_eq!(out.exit_code, None);
    assert!(started.elapsed() < Duration::from_secs(10));
    let pid = wait_for_file(&dir.path().join("grandchild.pid")).await;
    assert!(eventually_dead(&pid).await, "grandchild {pid} survived");
}

#[tokio::test]
async fn cancellation_stops_a_running_command_quickly() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let cancelled_at = Arc::new(Mutex::new(None));
    let stamp = Arc::clone(&cancelled_at);
    // Cancel once the command is demonstrably running.
    let sink: OutputSink = Arc::new(move |_: &str| {
        stamp.lock().unwrap().get_or_insert_with(Instant::now);
        trigger.cancel();
    });
    let out = run(
        spec("echo started; sleep 30", dir.path()),
        cancel,
        Some(sink),
    )
    .await
    .unwrap();
    assert!(out.cancelled && !out.timed_out);
    assert_eq!(out.stdout, "started\n");
    let cancelled_at = cancelled_at.lock().unwrap().expect("output arrived");
    assert!(cancelled_at.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn cancelled_before_start_runs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = run(spec("touch should-not-exist", dir.path()), cancel, None).await;
    assert!(matches!(result, Err(HostError::Cancelled)));
    assert!(!dir.path().join("should-not-exist").exists());
}

#[tokio::test]
async fn descendants_holding_the_pipes_are_reaped() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let command = "sh -c 'echo $$ > holder.pid; exec sleep 30' & echo done";
    let out = run_ok(spec(command, dir.path())).await;
    assert_eq!(out.exit_code, Some(0));
    assert_eq!(out.stdout, "done\n");
    assert!(started.elapsed() < Duration::from_secs(5));
    let pid = wait_for_file(&dir.path().join("holder.pid")).await;
    assert!(eventually_dead(&pid).await, "pipe holder {pid} survived");
}

#[tokio::test]
async fn environment_is_allowlisted_with_fixed_and_extra_values() {
    let dir = tempfile::tempdir().unwrap();
    let mut with_env = spec(
        "echo \"z=$ZENGINE pager=$GIT_PAGER prompt=$GIT_TERMINAL_PROMPT x=$X_EXTRA pkg=$CARGO_PKG_NAME leak=${CARGO_MANIFEST_DIR:-none}\"",
        dir.path(),
    );
    with_env.env = EnvPolicy {
        passthrough: vec!["CARGO_PKG_NAME".to_string()],
        extra: BTreeMap::from([("X_EXTRA".to_string(), "42".to_string())]),
    };
    let out = run_ok(with_env).await;
    assert_eq!(
        out.stdout,
        "z=1 pager=cat prompt=0 x=42 pkg=z-engine-host leak=none\n"
    );
}

#[tokio::test]
async fn output_beyond_the_budget_keeps_head_and_tail() {
    let dir = tempfile::tempdir().unwrap();
    let mut noisy = spec(
        "echo first; head -c 200000 /dev/zero | tr '\\0' 'a'; echo; echo last",
        dir.path(),
    );
    noisy.max_output_bytes = 10_000;
    let out = run_ok(noisy).await;
    assert!(out.truncated);
    assert!(out.stdout.starts_with("first\naaa"));
    assert!(out.stdout.ends_with("aaa\nlast\n"));
    assert!(out.stdout.len() < 10_100, "{}", out.stdout.len());
    assert!(out.stdout.contains("bytes of output omitted"));
}

#[tokio::test]
async fn stdin_payloads_are_delivered_then_closed() {
    let dir = tempfile::tempdir().unwrap();
    let mut echo = spec("cat; echo end", dir.path());
    echo.stdin = Some(br#"{"hook":"PreToolUse"}"#.to_vec());
    let out = run_ok(echo).await;
    assert_eq!(out.stdout, "{\"hook\":\"PreToolUse\"}end\n");

    let mut line = spec("read -r first; echo \"got $first\"", dir.path());
    line.stdin = Some(b"payload\nrest\n".to_vec());
    assert_eq!(run_ok(line).await.stdout, "got payload\n");

    // Without a payload stdin stays closed: `cat` sees EOF at once.
    assert_eq!(
        run_ok(spec("cat; echo eof", dir.path())).await.stdout,
        "eof\n"
    );
}

#[tokio::test]
async fn a_child_that_ignores_a_large_stdin_does_not_hang() {
    let dir = tempfile::tempdir().unwrap();
    let mut ignoring = spec("echo ignored; exit 5", dir.path());
    ignoring.stdin = Some(vec![b'x'; 8 * 1024 * 1024]);
    ignoring.timeout = Duration::from_secs(20);
    let started = Instant::now();
    let out = run_ok(ignoring).await;
    assert_eq!((out.exit_code, out.stdout.as_str()), (Some(5), "ignored\n"));
    assert!(!out.timed_out);
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn invalid_specs_are_rejected_before_spawning() {
    let dir = tempfile::tempdir().unwrap();
    let empty = run(spec("   ", dir.path()), CancellationToken::new(), None).await;
    assert!(matches!(empty, Err(HostError::Invalid(_))));
    let missing: PathBuf = dir.path().join("missing");
    let gone = run(spec("echo hi", &missing), CancellationToken::new(), None).await;
    assert!(matches!(gone, Err(HostError::NotFound(_))));
}

#[tokio::test]
async fn an_explicit_posix_shell_is_used() {
    let dir = tempfile::tempdir().unwrap();
    let mut explicit = spec("echo $0; cd /", dir.path());
    explicit.shell = ShellSpec {
        program: PathBuf::from("/bin/sh"),
        args: vec!["-c".to_string()],
        kind: ShellKind::Sh,
    };
    let out = run_ok(explicit).await;
    assert!(out.stdout.trim_end().ends_with("sh"), "{}", out.stdout);
    assert_eq!(out.final_cwd, Some(PathBuf::from("/")));
    assert!(resolve_shell(None).kind.is_posix());
}
