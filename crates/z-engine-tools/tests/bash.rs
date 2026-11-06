//! `Bash` in the foreground: exit codes, the persistent working directory,
//! timeouts, cancellation, streaming, truncation with spill, and gating.
#![cfg(unix)]

mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;
use support::{ctx, ok_text, project, recording_spill};
use z_engine_host::OutputSink;
use z_engine_policy::Action;
use z_engine_protocol::Preview;
use z_engine_tools::builtin::BashTool;
use z_engine_tools::{Tool, ToolError};

#[tokio::test]
async fn reports_output_and_exit_codes() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let output = BashTool
        .call(
            json!({"command": "echo out; sleep 0.2; echo err >&2; exit 3"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(output.is_error);
    assert_eq!(output.text_content(), "out\nerr\n\nExit code 3");
    assert_eq!(output.summary, "Exit code 3");
    assert!(output.effects.ran_command);
    let output = BashTool
        .call(json!({"command": "true"}), &ctx)
        .await
        .unwrap();
    assert!(!output.is_error);
    assert_eq!(output.text_content(), "(no output)");
}

#[tokio::test]
async fn the_working_directory_persists_and_stays_inside_the_project() {
    let dir = project(&[("sub/file.txt", "x")]);
    let ctx = ctx(dir.path());
    ok_text(&BashTool, &ctx, json!({"command": "cd sub"})).await;
    let text = ok_text(&BashTool, &ctx, json!({"command": "ls"})).await;
    assert_eq!(text, "file.txt");
    let text = ok_text(&BashTool, &ctx, json!({"command": "cd /"})).await;
    assert!(text.contains("Shell cwd was reset to"), "{text}");
    assert_eq!(ctx.current_cwd(), ctx.root);
    let text = ok_text(&BashTool, &ctx, json!({"command": "ls"})).await;
    assert_eq!(text, "sub");
}

#[tokio::test]
async fn a_vanished_working_directory_falls_back_to_the_root() {
    let dir = project(&[("gone/x", "")]);
    let ctx = ctx(dir.path());
    ctx.set_cwd(dir.path().join("gone"));
    std::fs::remove_dir_all(dir.path().join("gone")).unwrap();
    let text = ok_text(&BashTool, &ctx, json!({"command": "echo hi"})).await;
    assert!(text.starts_with("hi\n\nThe working directory"), "{text}");
    let canonical = |path: &std::path::Path| std::fs::canonicalize(path).unwrap();
    assert_eq!(canonical(&ctx.current_cwd()), canonical(&ctx.root));
}

#[tokio::test]
async fn timeouts_kill_the_command() {
    let dir = project(&[]);
    let started = Instant::now();
    let output = BashTool
        .call(
            json!({"command": "echo started; sleep 20", "timeout": 400}),
            &ctx(dir.path()),
        )
        .await
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(output.is_error);
    let text = output.text_content();
    assert!(text.starts_with("started"), "{text}");
    assert!(
        text.contains("Command timed out after 400 ms and was killed."),
        "{text}"
    );
}

#[tokio::test]
async fn cancellation_stops_the_command_quickly() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let cancel = ctx.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        cancel.cancel();
    });
    let started = Instant::now();
    let result = BashTool.call(json!({"command": "sleep 20"}), &ctx).await;
    assert_eq!(result.unwrap_err(), ToolError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(10));
    let again = BashTool.call(json!({"command": "echo no"}), &ctx).await;
    assert_eq!(again.unwrap_err(), ToolError::Cancelled);
}

#[tokio::test]
async fn output_streams_to_the_progress_sink() {
    let dir = project(&[]);
    let mut ctx = ctx(dir.path());
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink_lines = Arc::clone(&lines);
    let sink: OutputSink =
        Arc::new(move |line: &str| sink_lines.lock().unwrap().push(line.to_string()));
    ctx.progress = Some(sink);
    ok_text(&BashTool, &ctx, json!({"command": "echo one; echo two"})).await;
    assert_eq!(*lines.lock().unwrap(), vec!["one\n", "two\n"]);
}

#[tokio::test]
async fn long_output_is_truncated_and_spilled() {
    let dir = project(&[]);
    let mut ctx = ctx(dir.path());
    let (spill, stored) = recording_spill();
    ctx.spill = Some(spill);
    ctx.limits.max_result_chars = 2_000;
    let text = ok_text(&BashTool, &ctx, json!({"command": "seq 1 5000"})).await;
    assert!(text.starts_with("1\n2\n3\n"), "{text}");
    assert!(text.contains("\n5000"), "{text}");
    assert!(
        text.contains("saved at /artifacts/full-output.txt"),
        "{text}"
    );
    assert!(text.chars().count() < 2_500);
    let stored = stored.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].0, "bash");
    assert!(stored[0].1.contains("\n2500\n"));
}

#[tokio::test]
async fn read_only_commands_are_classified_and_gated_as_execute() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let ls = json!({"command": "ls -la src"});
    let rm = json!({"command": "rm -rf build", "description": "Remove the build directory"});
    assert!(BashTool.is_read_only(&ls) && BashTool.is_concurrency_safe(&ls));
    assert!(!BashTool.is_read_only(&rm) && !BashTool.is_concurrency_safe(&rm));
    assert_eq!(
        BashTool.action(&rm, &ctx),
        Action::Execute {
            command: "rm -rf build".into()
        }
    );
    assert_eq!(BashTool.title(&rm, &ctx), "Remove the build directory");
    assert_eq!(BashTool.title(&ls, &ctx), "ls -la src");
    assert_eq!(
        BashTool.preview(&rm, &ctx).await,
        Some(Preview::Command {
            command: "rm -rf build".into(),
            description: Some("Remove the build directory".into())
        })
    );
}
