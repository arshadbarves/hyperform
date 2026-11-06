//! Runs one check through the host shell and turns the result into a
//! protocol `CheckRecord`: workspace fingerprints before and after, parsed
//! test counts, a display tail, and the full output kept as an artifact.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use z_engine_host::{
    EnvPolicy, OutputSink, RunSpec, SandboxProfile, ShellSpec, relative_display, resolve, run,
    sandbox_shell, workspace_fingerprint,
};
use z_engine_protocol::{AgentId, CheckRecord, now_ms};

use crate::artifact::{tail, write_artifact};
use crate::{CheckSpec, DEFAULT_CHECK_TIMEOUT_SECS, VerifyError, parse_counts};

/// Bytes of output kept in the record itself.
const TAIL_BYTES: usize = 4 * 1024;

/// What a check needs from its caller besides the spec.
#[derive(Debug, Clone)]
pub struct CheckEnv {
    pub shell: ShellSpec,
    pub env: EnvPolicy,
    /// The agent the evidence is attributed to.
    pub agent_id: AgentId,
    /// Receives the full output of every run. Keep it outside the project
    /// (or ignored) so artifacts never change the workspace fingerprint.
    pub artifacts_dir: PathBuf,
    /// Fingerprinted before and after the run; relative check directories
    /// resolve against it.
    pub project_root: PathBuf,
    /// `Some` runs the check in the OS sandbox; a sandbox that cannot
    /// start is a host error, never an unconfined run.
    pub sandbox: Option<SandboxProfile>,
}

/// Runs `spec` to completion or timeout and records the evidence. A
/// failing or timed-out check is a record with `passed: false`, not an
/// error. `passed` requires exit code 0, no timeout, and no failed tests
/// when the output reports counts. Cancellation (before or during the run)
/// returns [`VerifyError::Cancelled`] after the process tree is killed;
/// host failures (missing directory, unusable shell, fingerprinting) and a
/// failed artifact write are errors.
pub async fn run_check(
    spec: &CheckSpec,
    env: &CheckEnv,
    cancel: CancellationToken,
    on_output: Option<OutputSink>,
) -> Result<CheckRecord, VerifyError> {
    if spec.command.trim().is_empty() {
        return Err(VerifyError::Invalid(format!(
            "check `{}` has no command",
            spec.id
        )));
    }
    if cancel.is_cancelled() {
        return Err(VerifyError::Cancelled);
    }
    let cwd = resolve(&env.project_root, &spec.cwd);
    let fingerprint_before = tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(VerifyError::Cancelled),
        fingerprint = workspace_fingerprint(&env.project_root) => fingerprint?,
    };

    let mut run_spec = RunSpec::new(spec.command.clone(), cwd.clone());
    run_spec.timeout = Duration::from_secs(timeout_secs(spec));
    run_spec.shell = match &env.sandbox {
        Some(profile) => sandbox_shell(&env.shell, profile)?,
        None => env.shell.clone(),
    };
    run_spec.env = env.env.clone();
    run_spec.track_cwd = false;
    tracing::debug!(check = %spec.id, cwd = %cwd.display(), "check started");
    let started_at = now_ms();
    let output = run(run_spec, cancel, on_output).await?;
    if output.cancelled {
        tracing::debug!(check = %spec.id, "check cancelled");
        return Err(VerifyError::Cancelled);
    }
    let fingerprint_after = workspace_fingerprint(&env.project_root).await?;

    let command = spec.command.clone();
    let combined = output.combined;
    let (tests, combined) =
        tokio::task::spawn_blocking(move || (parse_counts(&command, &combined), combined))
            .await
            .map_err(|error| {
                if error.is_panic() {
                    std::panic::resume_unwind(error.into_panic());
                }
                // Only a runtime that is shutting down drops blocking work.
                VerifyError::Cancelled
            })?;
    let unique = ulid::Ulid::new().to_string().to_lowercase();
    let artifact = write_artifact(&env.artifacts_dir, &spec.id, &unique, &combined).await?;
    let passed = output.exit_code == Some(0)
        && !output.timed_out
        && tests.is_none_or(|counts| counts.failed == 0);
    tracing::info!(
        check = %spec.id,
        exit_code = ?output.exit_code,
        timed_out = output.timed_out,
        passed,
        duration_ms = output.duration_ms,
        "check finished"
    );
    Ok(CheckRecord {
        record_id: format!("chk_{unique}"),
        check_id: spec.id.clone(),
        label: spec.label.clone(),
        kind: spec.kind,
        command: spec.command.clone(),
        cwd: display_cwd(&env.project_root, &cwd),
        agent_id: env.agent_id.clone(),
        exit_code: output.exit_code,
        passed,
        timed_out: output.timed_out,
        started_at,
        duration_ms: output.duration_ms,
        tests,
        artifact: Some(artifact.display().to_string()),
        fingerprint_before,
        fingerprint_after,
        output_tail: tail(&combined, TAIL_BYTES).to_string(),
    })
}

fn timeout_secs(spec: &CheckSpec) -> u64 {
    if spec.timeout_secs == 0 {
        DEFAULT_CHECK_TIMEOUT_SECS
    } else {
        spec.timeout_secs
    }
}

/// `.` for the root, `web` for a nested root (always `/`-separated), the
/// full path for a directory outside the project.
fn display_cwd(root: &Path, cwd: &Path) -> String {
    relative_display(&resolve(root, "."), cwd).replace('\\', "/")
}
