//! A work-plan task's helper from its dispatch to its change in the
//! project: the task is cleared and bound (forcing a worktree beside other
//! running tasks, and asking for one once parallel work is approved); at
//! the end the report is recorded with the files its
//! worktree diff touched and the
//! checks it ran, then the change comes back as `work.merge` says (`ask`:
//! the apply card; `auto`: applied at once, one at a time under the merge
//! lock), and the project tree verifies it. The user's apply or discard of
//! a task's worktree goes to the task too. A conflict leaves the project
//! tree untouched and keeps the worktree for a serial retry. A helper
//! stopped by a pause, a hold, a closing chat or a moved plan leaves its
//! task waiting with a resume pointer instead (see `work::interrupt`).

use std::path::PathBuf;
use std::sync::Arc;

use hyperform_config::MergePolicy;
use hyperform_protocol::work::{BlockReason, Integration, TaskStatus};
use hyperform_protocol::{
    AgentId, AgentInfo, AgentStatus, CallId, Isolation, NoticeLevel, TaskId, WorktreeState,
};
use hyperform_tools::SpawnRequest;
use hyperform_work::RunFacts;

use super::launch::Child;
use super::worktree::{self, Applied, changed_files};
use crate::run::RunContext;
use crate::session::SessionCore;
use crate::work::{self, Admission, News, tell_coordinator, verify_merged};

/// `Agent { task_id }`: the task is cleared for a helper, which then works
/// in its own worktree beside other running tasks (or in any approved
/// parallel plan) and starts from the task's packet and the result
/// contract. With `resume`, the interrupted helper the task's resume
/// pointer names continues it (in its kept worktree).
pub(super) async fn admit(
    parent: &RunContext,
    mut req: SpawnRequest,
) -> Result<(SpawnRequest, Option<Admission>), String> {
    let Some(raw) = req.task_id.clone() else {
        return Ok((req, None));
    };
    let caller = &parent.spec.agent_id;
    let resume = req.resume.as_ref();
    let admission = work::admit(&parent.core, caller, &raw, &req.prompt, resume).await?;
    if admission.worktree_wanted {
        req.isolation = Some(Isolation::Worktree);
    }
    req.prompt = admission.brief.clone();
    req.task_id = Some(admission.task.to_string());
    Ok((req, Some(admission)))
}

/// Binds the task to the prepared helper; a helper the plan refuses (or
/// that needed a worktree it could not get) stops before it runs.
pub(super) async fn bind(
    parent: &RunContext,
    call_id: &CallId,
    child: &Child,
    admission: &Admission,
) -> Result<(), String> {
    let core = &parent.core;
    let helper = &child.ctx.spec.agent_id;
    let worktree = child.worktree.as_ref().map(|tree| tree.path.clone());
    let bound = if admission.worktree_required && worktree.is_none() {
        Err(format!(
            "task {} runs beside other tasks, so its helper needs its own worktree, and none could be created",
            admission.task
        ))
    } else {
        admission.bind(core, &parent.spec.agent_id, call_id, helper, worktree)
    };
    let Err(error) = bound else {
        let (tracker, cancel) = (Arc::clone(&child.tracker), child.ctx.cancel.clone());
        admission.watch(core, helper, tracker, cancel);
        return Ok(());
    };
    if let Some(tree) = &child.worktree {
        worktree::abandon(core, tree).await;
    }
    child.tracker.finish(core, |info| {
        info.status = AgentStatus::Failed;
        info.error = Some(error.clone());
        info.task_id = None;
        info.worktree = info.worktree.take().map(|mut tree| {
            tree.state = WorktreeState::Discarded;
            tree
        });
    });
    Err(error)
}

/// After a task-bound helper's run ended (its final `AgentInfo`).
pub(super) async fn settle(
    core: &Arc<SessionCore>,
    info: &AgentInfo,
    written: &[PathBuf],
    read_web: bool,
) {
    let Some(task) = info.task_id.clone() else {
        return;
    };
    let helper = &info.agent_id;
    if let Some(tell) = work::settle_halted(core, info, &task) {
        if tell {
            tell_coordinator(core, helper, &task, News::Interrupted).await;
        }
        return;
    }
    let files = match &info.worktree {
        Some(tree) => changed_files(core, tree).await,
        None => None,
    };
    let run = RunFacts {
        files_changed: files,
        checks: work::checks_of(core, helper),
    };
    if !work::conclude(core, helper, &task, run, read_web) {
        let news = if blocked_unreported(core, &task) {
            News::Unreported
        } else {
            News::Ended
        };
        tell_coordinator(core, helper, &task, news).await;
        return;
    }
    let merge = core.settings().settings.work.merge;
    match &info.worktree {
        None => {
            let files = relative(core, written);
            merged(core, helper, Integration::Applied { files }).await;
        }
        Some(tree) if tree.state == WorktreeState::Empty => {
            let files = Vec::new();
            merged(core, helper, Integration::Applied { files }).await;
        }
        Some(_) if merge == MergePolicy::Auto => {
            if let Err(error) = apply(core, helper).await {
                core.events.notice(
                    NoticeLevel::Warn,
                    format!("Could not apply agent {helper}'s changes: {error}"),
                );
                waits(core, helper, info).await;
            }
        }
        Some(_) => waits(core, helper, info).await,
    }
}

/// Why the user may not apply `agent`'s change by hand: it is a task
/// helper's whose task does not wait for it (an interrupted helper's).
pub(crate) fn apply_refusal(core: &SessionCore, agent: &AgentId) -> Option<String> {
    let task = task_of(core, agent)?;
    work::foreign_apply(core, agent, &task)
}

/// Applies a worktree agent's change (the apply card, `ApplyAgentChanges`,
/// or `merge = "auto"`); a task's helper's change goes to its task.
pub(crate) async fn apply(core: &Arc<SessionCore>, agent: &AgentId) -> Result<Applied, String> {
    let applied = worktree::apply(core, agent).await?;
    if task_of(core, agent).is_some() {
        if applied.merged {
            let files = applied.files.clone();
            merged(core, agent, Integration::Applied { files }).await;
        } else {
            let paths = if applied.conflicts.is_empty() {
                applied.files.clone()
            } else {
                applied.conflicts.clone()
            };
            let conflict = Integration::Conflict {
                paths,
                worktree: applied.worktree.clone(),
            };
            if let Some(task) = work::integrate(core, agent, conflict) {
                tell_coordinator(core, agent, &task, News::Conflict).await;
            }
        }
    }
    Ok(applied)
}

/// Discards a worktree agent's change; a task's helper's change is
/// discarded on its task too, which blocks it until it is retried. An
/// interrupted helper's worktree is dropped from its task's resume pointer.
pub(crate) async fn discard(core: &Arc<SessionCore>, agent: &AgentId) -> Result<(), String> {
    worktree::discard(core, agent).await?;
    let Some(task) = task_of(core, agent) else {
        return Ok(());
    };
    match work::integrate(core, agent, Integration::Discarded) {
        Some(task) => tell_coordinator(core, agent, &task, News::Discarded).await,
        None => work::kept_discarded(core, agent, &task),
    }
    Ok(())
}

/// The worktree a serial retry leaves behind (the task's run was reset).
pub(crate) async fn drop_kept(core: &Arc<SessionCore>, agent: &AgentId) {
    if let Err(error) = worktree::discard(core, agent).await {
        tracing::debug!(%error, %agent, "no kept worktree to remove for a serial retry");
    }
}

async fn waits(core: &Arc<SessionCore>, helper: &AgentId, info: &AgentInfo) {
    let Some(tree) = &info.worktree else {
        return;
    };
    let pending = Integration::PendingApply {
        worktree: tree.path.clone(),
    };
    if let Some(task) = work::integrate(core, helper, pending) {
        tell_coordinator(core, helper, &task, News::WaitsForApply).await;
    }
}

/// The change is in the project tree: the plan links the tree, and the
/// task verifies there.
async fn merged(core: &Arc<SessionCore>, helper: &AgentId, applied: Integration) {
    let Some(task) = work::integrate(core, helper, applied) else {
        return;
    };
    work::link_checkpoint(core).await;
    tokio::spawn(verify_merged(Arc::clone(core), helper.clone(), task));
}

/// `agent`'s info as published last (an applied worktree included).
pub(super) fn latest(core: &SessionCore, agent: &AgentId) -> Option<AgentInfo> {
    core.with_state(|state| {
        let found = state.agents.iter().find(|info| info.agent_id == *agent);
        found.cloned()
    })
}

/// The task `agent` was bound to, if it was a task helper.
fn task_of(core: &SessionCore, agent: &AgentId) -> Option<TaskId> {
    core.with_state(|state| {
        let info = state.agents.iter().find(|info| info.agent_id == *agent)?;
        info.task_id.clone()
    })
}

fn blocked_unreported(core: &SessionCore, task: &TaskId) -> bool {
    work::attached(core).is_some_and(|plan| {
        plan.read(|state| {
            state.task(task).is_some_and(|task| {
                task.status
                    == TaskStatus::Blocked {
                        reason: BlockReason::Unreported,
                    }
            })
        })
    })
}

fn relative(core: &SessionCore, written: &[PathBuf]) -> Vec<String> {
    let mut files: Vec<String> = written
        .iter()
        .map(|path| path.strip_prefix(&core.root).unwrap_or(path))
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect();
    files.sort();
    files.dedup();
    files
}
