//! What the engine tells the kernel: the time, the session's plan mode,
//! the `[work]` limits, what the deterministic audit needs, the acting
//! agent's tree fingerprint, and files changed behind the plan's back.

use std::path::Path;

use z_engine_config::FeatureId;
use z_engine_host::{ChangeKind, workspace_fingerprint};
use z_engine_protocol::work::{Actor, Task, TaskStatus};
use z_engine_protocol::{AgentId, TaskId, now_ms};
use z_engine_work::{AuditScope, Facts, Limits, PlanState, RISKY_PATHS, graph};

use crate::session::SessionCore;

/// The answer to a plan command while `work_plans` is Off.
pub(crate) const OFF: &str = "Work plans are off; turn on work_plans in Settings > Experimental.";

/// The `work_plans` feature is On for this session.
pub(crate) fn enabled(core: &SessionCore) -> bool {
    core.settings()
        .settings
        .feature(FeatureId::WorkPlans)
        .acts()
}

pub(crate) fn agent(core: &SessionCore, agent_id: &AgentId) -> Actor {
    Actor::Agent {
        session_id: core.id.clone(),
        agent_id: agent_id.clone(),
    }
}

/// Facts without a fingerprint: no check evidence counts as fresh.
pub(crate) fn instant(core: &SessionCore) -> Facts {
    let settings = core.settings();
    let work = &settings.settings.work;
    Facts {
        planning: core.planning(),
        limits: Limits {
            max_parallel: settings.settings.max_parallel_tasks(),
            max_attempts: Some(work.max_attempts),
            serial_paths: work.serial_paths.clone(),
        },
        audit: Some(AuditScope {
            check_kinds: Some(check_kinds(core)),
            risky_paths: RISKY_PATHS
                .iter()
                .map(|path| path.to_string())
                .chain(work.sensitive_paths.iter().cloned())
                .collect(),
        }),
        ..Facts::at(now_ms())
    }
}

/// Facts with the fingerprint of the tree at `root`.
pub(crate) async fn current(core: &SessionCore, root: &Path) -> Facts {
    let mut facts = instant(core);
    facts.fingerprint = match workspace_fingerprint(root).await {
        Ok(fingerprint) => Some(fingerprint),
        Err(error) => {
            tracing::debug!(%error, "no workspace fingerprint for the plan");
            None
        }
    };
    facts
}

fn check_kinds(core: &SessionCore) -> Vec<z_engine_protocol::CheckKind> {
    let mut kinds = Vec::new();
    for check in &core.checks.profile().checks {
        if !kinds.contains(&check.kind) {
            kinds.push(check.kind);
        }
    }
    kinds
}

/// Files changed and deleted since the session's previous checkpoint. A
/// scope path that does not exist is not a deletion: it may be a file its
/// task will create; only the diff says what was deleted.
pub(crate) async fn changed_files(core: &SessionCore, facts: &mut Facts) {
    let since = core.with_state(|s| {
        let last = s.checkpoints.last();
        last.map(|(info, commit)| (info.created_at, commit.clone()))
    });
    if let Some((taken_at, commit)) = since {
        facts.changed_since = Some(taken_at);
        diff_since(core, &commit, facts).await;
    }
}

/// Adds the files changed and deleted since the shadow checkpoint
/// `commit`; `false` when the diff could not be made.
pub(crate) async fn diff_since(core: &SessionCore, commit: &str, facts: &mut Facts) -> bool {
    let repo = match core.checkpoints.repo(core).await {
        Ok(repo) => repo,
        Err(error) => {
            tracing::debug!(%error, "no checkpoint repository for the plan");
            return false;
        }
    };
    match repo.diff_worktree(commit).await {
        Ok(changes) => {
            for change in changes {
                let set = match change.kind {
                    ChangeKind::Deleted => &mut facts.deleted_files,
                    ChangeKind::Added | ChangeKind::Modified => &mut facts.changed_files,
                };
                set.insert(change.path);
            }
            true
        }
        Err(error) => {
            tracing::debug!(%error, "plan reconciliation diff failed");
            false
        }
    }
}

/// The task the main agent works on: the one it started, else the next
/// ready task (none while the plan is not active, except the tasks
/// reconciliation findings leave free while it replans).
pub(crate) fn focus<'a>(state: &'a PlanState, actor: &Actor) -> Option<&'a Task> {
    owned_active(state, actor).or_else(|| graph::next_ready(state, None))
}

/// The active task `actor` started.
pub(crate) fn owned_active<'a>(state: &'a PlanState, actor: &Actor) -> Option<&'a Task> {
    let Actor::Agent {
        session_id,
        agent_id,
    } = actor
    else {
        return None;
    };
    state.tasks.values().find(|task| {
        task.status == TaskStatus::Active
            && task
                .owner
                .as_ref()
                .is_some_and(|owner| owner.session_id == *session_id && owner.agent_id == *agent_id)
    })
}

/// A task id the model wrote, trimmed.
pub(crate) fn task_id(raw: &str) -> TaskId {
    TaskId(raw.trim().to_string())
}
