//! Background jobs through the job port: `Bash` with `run_in_background`,
//! `JobOutput`, and `JobKill`.

mod support;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use support::fakes::FakeJobs;
use support::{ctx, ctx_with, err_text, ok_text, project};
use z_engine_policy::Action;
use z_engine_protocol::{JobKind, JobStatus};
use z_engine_tools::builtin::{BashTool, JobKillTool, JobOutputTool};
use z_engine_tools::{JobOutput, Ports, Tool, ToolError};

fn with_jobs(dir: &tempfile::TempDir, jobs: Arc<FakeJobs>) -> z_engine_tools::ToolCtx {
    ctx_with(
        dir.path(),
        Ports {
            jobs: Some(jobs),
            ..Ports::default()
        },
    )
}

#[tokio::test]
async fn background_bash_starts_a_job_and_points_at_job_output() {
    let dir = project(&[]);
    let jobs = Arc::new(FakeJobs::default());
    let ctx = with_jobs(&dir, Arc::clone(&jobs));
    let output = BashTool
        .call(
            json!({"command": "npm run dev", "description": "Start the dev server", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        output.text_content().contains("job job_1") && output.text_content().contains("JobOutput")
    );
    assert_eq!(output.summary, "Running in background (job_1)");
    assert!(output.effects.ran_command);
    assert_eq!(
        *jobs.spawned.lock().unwrap(),
        vec![(
            "npm run dev".to_string(),
            Some("Start the dev server".to_string())
        )]
    );
}

#[tokio::test]
async fn background_bash_without_a_job_port_is_unavailable() {
    let dir = project(&[]);
    let err = BashTool
        .call(
            json!({"command": "sleep 1", "run_in_background": true}),
            &ctx(dir.path()),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Unavailable(_)), "{err:?}");
    let err = err_text(
        &BashTool,
        &ctx(dir.path()),
        json!({"command": "sleep 1", "run_in_background": "later"}),
    )
    .await;
    assert!(
        err.contains("`run_in_background` must be a boolean"),
        "{err}"
    );
}

#[tokio::test]
async fn job_output_reports_state_and_passes_filter_and_wait() {
    let dir = project(&[]);
    let jobs = Arc::new(FakeJobs::default());
    let ctx = with_jobs(&dir, Arc::clone(&jobs));
    let text = ok_text(
        &JobOutputTool,
        &ctx,
        json!({"job_id": "job_1", "filter": "ready|error", "wait_ms": 1500}),
    )
    .await;
    assert_eq!(
        text,
        "Job job_1 (shell) status: completed, exit code 0.\n\nserver ready"
    );
    assert_eq!(
        *jobs.reads.lock().unwrap(),
        vec![(
            "job_1".into(),
            Some("ready|error".to_string()),
            Some(Duration::from_millis(1500))
        )]
    );
    let err = err_text(
        &JobOutputTool,
        &ctx,
        json!({"job_id": "job_1", "filter": "("}),
    )
    .await;
    assert!(err.contains("`filter` is not a valid regex"), "{err}");
    let err = err_text(&JobOutputTool, &ctx, json!({"job_id": "job_9"})).await;
    assert_eq!(err, "no job job_9");
}

#[tokio::test]
async fn failed_jobs_are_errors_and_empty_reads_say_so() {
    let dir = project(&[]);
    let jobs = Arc::new(FakeJobs {
        output: JobOutput {
            status: JobStatus::Failed,
            exit_code: Some(1),
            output: String::new(),
            kind: JobKind::Agent,
        },
        ..FakeJobs::default()
    });
    let ctx = with_jobs(&dir, jobs);
    let output = JobOutputTool
        .call(json!({"job_id": "job_1"}), &ctx)
        .await
        .unwrap();
    assert!(output.is_error);
    assert_eq!(
        output.text_content(),
        "Job job_1 (agent) status: failed, exit code 1.\n\n(no new output)"
    );
}

#[tokio::test]
async fn job_kill_stops_the_job() {
    let dir = project(&[]);
    let jobs = Arc::new(FakeJobs::default());
    let ctx = with_jobs(&dir, Arc::clone(&jobs));
    let output = JobKillTool
        .call(json!({"job_id": "job_1"}), &ctx)
        .await
        .unwrap();
    assert_eq!(output.summary, "Stopped job_1");
    assert_eq!(jobs.killed.lock().unwrap().len(), 1);
    let input = json!({"job_id": "job_1"});
    assert_eq!(
        JobKillTool.action(&input, &ctx),
        Action::Other { read_only: true }
    );
    assert_eq!(
        JobOutputTool.action(&input, &ctx),
        Action::Other { read_only: true }
    );
    assert!(JobOutputTool.is_read_only(&input));
}
