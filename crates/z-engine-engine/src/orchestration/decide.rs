//! The user's decision on a worktree agent (`ApplyAgentChanges` /
//! `DiscardAgentChanges` commands). A merged change counts as a change of
//! the tree for the next turn, like a `!cmd`; a work-plan task's helper's
//! change goes to its task as well (an interrupted helper's is its task's
//! to resume, so it is not applied by hand).

use std::sync::Arc;

use z_engine_protocol::{AgentId, NoticeLevel};

use super::task_run;
use crate::session::SessionCore;

pub(crate) fn apply_command(core: &Arc<SessionCore>, agent: AgentId) {
    let core = Arc::clone(core);
    tokio::spawn(async move {
        if let Some(refusal) = task_run::apply_refusal(&core, &agent) {
            core.events.notice(NoticeLevel::Warn, refusal);
            return;
        }
        match task_run::apply(&core, &agent).await {
            Ok(applied) if applied.merged => {
                core.with_state(|state| state.external_mutation = true);
                core.events.notice(NoticeLevel::Info, applied.summary);
            }
            Ok(applied) => core.events.notice(NoticeLevel::Warn, applied.summary),
            Err(error) => core.events.notice(
                NoticeLevel::Warn,
                format!("Could not apply agent {agent}'s changes: {error}"),
            ),
        }
    });
}

pub(crate) fn discard_command(core: &Arc<SessionCore>, agent: AgentId) {
    let core = Arc::clone(core);
    tokio::spawn(async move {
        if let Err(error) = task_run::discard(&core, &agent).await {
            core.events.notice(
                NoticeLevel::Warn,
                format!("Could not discard agent {agent}'s changes: {error}"),
            );
        }
    });
}
