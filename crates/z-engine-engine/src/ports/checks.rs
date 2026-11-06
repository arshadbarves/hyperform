//! `CheckPort`: the session's checks for `Verify`; runs are recorded as
//! evidence attributed to the calling agent, in the agent's root.

use std::sync::Arc;

use async_trait::async_trait;
use z_engine_protocol::CheckRecord;
use z_engine_tools::{CheckPort, CheckSummary, ToolCtx};

use crate::session::SessionCore;
use crate::verify::{CheckRun, run_recorded};

#[derive(Debug)]
pub(crate) struct Checks {
    core: Arc<SessionCore>,
}

impl Checks {
    pub(crate) fn new(core: Arc<SessionCore>) -> Self {
        Self { core }
    }
}

#[async_trait]
impl CheckPort for Checks {
    fn list(&self, _ctx: &ToolCtx) -> Vec<CheckSummary> {
        self.core
            .checks
            .profile()
            .checks
            .iter()
            .map(|check| CheckSummary {
                id: check.id.clone(),
                label: check.label.clone(),
                kind: check.kind,
                command: check.command.clone(),
            })
            .collect()
    }

    async fn run(&self, ctx: &ToolCtx, check_id: &str) -> Result<CheckRecord, String> {
        let spec = self
            .core
            .checks
            .find(check_id)
            .ok_or_else(|| format!("unknown check `{check_id}`"))?;
        let run = CheckRun {
            agent_id: ctx.agent_id.clone(),
            root: ctx.root.clone(),
            cancel: ctx.cancel.clone(),
            progress: ctx.progress.clone(),
        };
        run_recorded(&self.core, &spec, run).await
    }
}
