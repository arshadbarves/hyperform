//! `Verify` through a fake check port: listing, running, records, gating.

mod support;

use std::sync::{Arc, Mutex};

use serde_json::json;
use support::fakes::FakeChecks;
use support::{ctx, ctx_with, err_text, ok_text, project};
use z_engine_policy::Action;
use z_engine_protocol::{AgentId, CheckKind, CheckRecord, TestCounts};
use z_engine_tools::builtin::VerifyTool;
use z_engine_tools::{CheckSummary, Ports, Tool, ToolCtx, ToolError};

fn record(passed: bool) -> CheckRecord {
    CheckRecord {
        record_id: "rec_1".into(),
        check_id: "test".into(),
        label: "Tests".into(),
        kind: CheckKind::Test,
        command: "cargo test".into(),
        cwd: "/p".into(),
        agent_id: AgentId::main(),
        exit_code: Some(if passed { 0 } else { 101 }),
        passed,
        timed_out: false,
        started_at: 0,
        duration_ms: 12_340,
        tests: Some(TestCounts {
            passed: 120,
            failed: u32::from(!passed),
            skipped: 2,
        }),
        artifact: Some("/artifacts/check.log".into()),
        fingerprint_before: "a".into(),
        fingerprint_after: "a".into(),
        output_tail: "test result: ok\n".into(),
    }
}

fn setup(passed: bool) -> (tempfile::TempDir, ToolCtx, Arc<FakeChecks>) {
    let dir = project(&[]);
    let port = Arc::new(FakeChecks {
        checks: vec![
            CheckSummary {
                id: "test".into(),
                label: "Tests".into(),
                kind: CheckKind::Test,
                command: "cargo test".into(),
            },
            CheckSummary {
                id: "lint".into(),
                label: "Clippy".into(),
                kind: CheckKind::Lint,
                command: "cargo clippy".into(),
            },
        ],
        record: record(passed),
        runs: Mutex::default(),
    });
    let ctx = ctx_with(
        dir.path(),
        Ports {
            checks: Some(port.clone()),
            ..Ports::default()
        },
    );
    (dir, ctx, port)
}

#[tokio::test]
async fn list_shows_a_table_of_checks() {
    let (_dir, ctx, _port) = setup(true);
    let text = ok_text(&VerifyTool, &ctx, json!({"action": "list"})).await;
    assert_eq!(
        text,
        "| id | kind | label | command |\n|---|---|---|---|\n| test | test | Tests | `cargo test` |\n| lint | lint | Clippy | `cargo clippy` |\n"
    );
}

#[tokio::test]
async fn run_formats_the_record_as_evidence() {
    let (_dir, ctx, port) = setup(true);
    let output = VerifyTool
        .call(json!({"action": "run", "check": "test"}), &ctx)
        .await
        .unwrap();
    let text = output.text_content();
    assert!(
        text.starts_with("PASS: Tests (test) `cargo test` (exit code 0, 12.3 s)\n"),
        "{text}"
    );
    assert!(text.contains("Tests: 120 passed, 0 failed, 2 skipped\n"));
    assert!(text.contains("Full output: /artifacts/check.log\n"));
    assert!(
        text.ends_with("Output (last lines):\ntest result: ok\n"),
        "{text}"
    );
    assert!(!output.is_error && output.effects.ran_command);
    assert_eq!(output.summary, "PASS Tests");
    assert_eq!(*port.runs.lock().unwrap(), vec!["test".to_string()]);
}

#[tokio::test]
async fn failing_checks_are_errors() {
    let (_dir, ctx, _port) = setup(false);
    let output = VerifyTool
        .call(json!({"action": "run", "check": "test"}), &ctx)
        .await
        .unwrap();
    assert!(output.is_error);
    assert!(
        output.text_content().starts_with("FAIL: Tests"),
        "{}",
        output.text_content()
    );
}

#[tokio::test]
async fn unknown_checks_and_actions_are_rejected() {
    let (_dir, ctx, _port) = setup(true);
    let err = err_text(
        &VerifyTool,
        &ctx,
        json!({"action": "run", "check": "bench"}),
    )
    .await;
    assert!(
        err.contains("unknown check \"bench\"; available checks: test, lint"),
        "{err}"
    );
    let err = err_text(&VerifyTool, &ctx, json!({"action": "run"})).await;
    assert!(err.contains("`check` is required"), "{err}");
    let err = err_text(&VerifyTool, &ctx, json!({"action": "fix"})).await;
    assert!(err.contains("`action` must be list or run"), "{err}");
    let dir = project(&[]);
    let err = VerifyTool
        .call(json!({"action": "list"}), &ctx_plain(&dir))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Unavailable(_)));
}

fn ctx_plain(dir: &tempfile::TempDir) -> ToolCtx {
    ctx(dir.path())
}

#[tokio::test]
async fn run_gates_as_the_checks_command_and_list_as_read_only() {
    let (_dir, ctx, _port) = setup(true);
    let run = json!({"action": "run", "check": "lint"});
    let list = json!({"action": "list"});
    assert_eq!(
        VerifyTool.action(&run, &ctx),
        Action::Execute {
            command: "cargo clippy".into()
        }
    );
    assert_eq!(
        VerifyTool.action(&list, &ctx),
        Action::Other { read_only: true }
    );
    assert_eq!(
        VerifyTool.action(&json!({"action": "run", "check": "nope"}), &ctx),
        Action::Other { read_only: false }
    );
    assert!(!VerifyTool.is_concurrency_safe(&run) && VerifyTool.is_concurrency_safe(&list));
    assert_eq!(VerifyTool.title(&run, &ctx), "Run check lint");
}
