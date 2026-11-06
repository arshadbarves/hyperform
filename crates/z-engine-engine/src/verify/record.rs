//! Running one check as evidence: the same path for `Verify` calls and
//! harness-initiated checks. The record is kept in the session state,
//! persisted, linked to the work task its agent owns, announced, and the
//! badge is recomputed.

use std::path::PathBuf;

use tokio_util::sync::CancellationToken;
use z_engine_host::OutputSink;
use z_engine_protocol::{AgentId, CheckRecord, Event};
use z_engine_store::LogRecord;
use z_engine_verify::{CheckEnv, CheckSpec, run_check};

use super::outcome::publish_outcome;
use crate::session::SessionCore;
use crate::settings::sandbox;
use crate::work;

/// Who runs a check, where, and where its output streams.
pub(crate) struct CheckRun {
    pub agent_id: AgentId,
    /// The agent's root: the project or its worktree.
    pub root: PathBuf,
    pub cancel: CancellationToken,
    pub progress: Option<OutputSink>,
}

pub(crate) async fn run_recorded(
    core: &SessionCore,
    spec: &CheckSpec,
    run: CheckRun,
) -> Result<CheckRecord, String> {
    let settings = core.settings();
    let sandbox = sandbox::profile_for(&settings, &run.root, &core.additional_dirs());
    let env = CheckEnv {
        shell: settings.shell.clone(),
        env: settings.env.clone(),
        agent_id: run.agent_id,
        artifacts_dir: core.shared.store.artifacts(&core.id).dir().to_path_buf(),
        project_root: run.root,
        sandbox,
    };
    let record = run_check(spec, &env, run.cancel, run.progress)
        .await
        .map_err(|error| format!("check `{}` did not complete: {error}", spec.id))?;
    core.journal.append_or_report(&LogRecord::Check {
        record: record.clone(),
    });
    core.with_state(|state| state.checks.push(record.clone()));
    work::link_check(core, &record).await;
    core.events.emit(Event::CheckRecorded {
        record: record.clone(),
    });
    publish_outcome(core).await;
    Ok(record)
}
