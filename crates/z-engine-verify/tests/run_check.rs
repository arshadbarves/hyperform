#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;
use z_engine_host::{EnvPolicy, HostError, OutputSink, resolve_shell};
use z_engine_protocol::{AgentId, CheckKind, TestCounts};
use z_engine_verify::{CheckEnv, CheckSource, CheckSpec, VerifyError, run_check};

struct Setup {
    project: tempfile::TempDir,
    session: tempfile::TempDir,
}

impl Setup {
    fn new() -> Self {
        Self {
            project: tempfile::tempdir().unwrap(),
            session: tempfile::tempdir().unwrap(),
        }
    }

    fn env(&self) -> CheckEnv {
        CheckEnv {
            shell: resolve_shell(None),
            env: EnvPolicy::default(),
            agent_id: AgentId::main(),
            artifacts_dir: self.session.path().join("artifacts"),
            project_root: self.project.path().to_path_buf(),
            sandbox: None,
        }
    }
}

fn spec(id: &str, command: &str) -> CheckSpec {
    CheckSpec {
        id: id.to_string(),
        label: command.to_string(),
        kind: CheckKind::Test,
        command: command.to_string(),
        cwd: PathBuf::from("."),
        source: CheckSource::Configured,
        timeout_secs: 30,
    }
}

async fn run(
    setup: &Setup,
    spec: &CheckSpec,
) -> Result<z_engine_protocol::CheckRecord, VerifyError> {
    run_check(spec, &setup.env(), CancellationToken::new(), None).await
}

#[tokio::test]
async fn a_passing_check_is_recorded_with_its_artifact() {
    let setup = Setup::new();
    let record = run(&setup, &spec("custom:hello", "echo hello"))
        .await
        .unwrap();
    assert!(record.passed && !record.timed_out);
    assert_eq!(record.exit_code, Some(0));
    assert!(record.record_id.starts_with("chk_"));
    assert_eq!(record.check_id, "custom:hello");
    assert_eq!(
        (record.label.as_str(), record.cwd.as_str()),
        ("echo hello", ".")
    );
    assert_eq!(record.agent_id, AgentId::main());
    assert_eq!(record.tests, None);
    assert_eq!(record.output_tail, "hello\n");
    assert!(record.started_at > 0);
    assert_eq!(record.fingerprint_before, record.fingerprint_after);
    let artifact = PathBuf::from(record.artifact.unwrap());
    assert!(artifact.starts_with(setup.session.path().join("artifacts")));
    let name = artifact.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("check-custom-hello-") && name.ends_with(".log"),
        "{name}"
    );
    assert_eq!(std::fs::read_to_string(artifact).unwrap(), "hello\n");
}

#[tokio::test]
async fn a_failing_check_is_a_record_not_an_error() {
    let setup = Setup::new();
    let record = run(&setup, &spec("custom:boom", "echo boom >&2; exit 3"))
        .await
        .unwrap();
    assert!(!record.passed);
    assert_eq!(record.exit_code, Some(3));
    assert!(record.output_tail.contains("boom"));
}

#[tokio::test]
async fn parsed_failures_fail_a_zero_exit() {
    let setup = Setup::new();
    let line = |passed, failed| {
        format!(
            "printf 'test result: ok. {passed} passed; {failed} failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s\\n'"
        )
    };
    let failing = run(&setup, &spec("cargo:test", &line(3, 1))).await.unwrap();
    assert_eq!(failing.exit_code, Some(0));
    assert_eq!(
        failing.tests,
        Some(TestCounts {
            passed: 3,
            failed: 1,
            skipped: 1
        })
    );
    assert!(!failing.passed);
    let passing = run(&setup, &spec("cargo:test", &line(4, 0))).await.unwrap();
    assert!(passing.passed);
    assert_eq!(passing.tests.map(|t| t.passed), Some(4));
}

#[tokio::test]
async fn timeouts_are_recorded_as_failures() {
    let setup = Setup::new();
    let started = Instant::now();
    let slow = CheckSpec {
        timeout_secs: 1,
        ..spec("custom:slow", "sleep 10")
    };
    let record = run(&setup, &slow).await.unwrap();
    assert!(record.timed_out && !record.passed);
    assert_eq!(record.exit_code, None);
    assert!(started.elapsed() < Duration::from_secs(8));
}

#[tokio::test]
async fn cancellation_is_an_error() {
    let setup = Setup::new();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let early = run_check(&spec("custom:x", "echo x"), &setup.env(), cancelled, None).await;
    assert!(matches!(early, Err(VerifyError::Cancelled)));

    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        trigger.cancel();
    });
    let started = Instant::now();
    let during = run_check(&spec("custom:slow", "sleep 10"), &setup.env(), cancel, None).await;
    assert!(matches!(during, Err(VerifyError::Cancelled)), "{during:?}");
    assert!(started.elapsed() < Duration::from_secs(8));
}

#[tokio::test]
async fn writes_during_a_check_change_the_fingerprint() {
    let setup = Setup::new();
    std::fs::write(setup.project.path().join("existing.txt"), "a").unwrap();
    let record = run(&setup, &spec("custom:gen", "echo data > created.txt"))
        .await
        .unwrap();
    assert!(record.passed);
    assert_ne!(record.fingerprint_before, record.fingerprint_after);
    assert!(setup.project.path().join("created.txt").is_file());
}

#[tokio::test]
async fn nested_directories_output_streaming_and_long_output() {
    let setup = Setup::new();
    std::fs::create_dir(setup.project.path().join("sub")).unwrap();
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink_lines = Arc::clone(&lines);
    let sink: OutputSink =
        Arc::new(move |line: &str| sink_lines.lock().unwrap().push(line.to_string()));
    let nested = CheckSpec {
        cwd: PathBuf::from("sub"),
        ..spec("sub/custom:pwd", "pwd; yes line | head -n 2000")
    };
    let record = run_check(&nested, &setup.env(), CancellationToken::new(), Some(sink))
        .await
        .unwrap();
    assert_eq!(record.cwd, "sub");
    let artifact = std::fs::read_to_string(record.artifact.as_deref().unwrap()).unwrap();
    assert!(artifact.lines().next().unwrap().ends_with("/sub"));
    assert_eq!(artifact.lines().filter(|l| *l == "line").count(), 2000);
    assert!(record.output_tail.len() <= 4096);
    assert!(record.output_tail.ends_with("line\n"));
    assert_eq!(lines.lock().unwrap().len(), 2001);
}

#[tokio::test]
async fn invalid_specs_and_unwritable_artifacts_are_errors() {
    let setup = Setup::new();
    let empty = run(&setup, &spec("custom:empty", "  ")).await;
    assert!(matches!(empty, Err(VerifyError::Invalid(_))));

    let missing = CheckSpec {
        cwd: PathBuf::from("missing"),
        ..spec("custom:x", "echo x")
    };
    let error = run(&setup, &missing).await.unwrap_err();
    assert!(
        matches!(error, VerifyError::Host(HostError::NotFound(_))),
        "{error}"
    );

    let blocked = setup.session.path().join("artifacts");
    std::fs::write(&blocked, "a file where the directory should be").unwrap();
    let error = run(&setup, &spec("custom:x", "echo x")).await.unwrap_err();
    let VerifyError::Io { path, .. } = error else {
        panic!("expected an I/O error, got {error}");
    };
    assert!(Path::new(&path).starts_with(&blocked));
}
