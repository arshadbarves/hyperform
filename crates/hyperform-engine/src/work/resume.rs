//! Resuming the attached plan when this chat opens with it (after an app
//! restart too) or attaches it. Active tasks whose helper is gone go back
//! to Pending with resume pointers (the helpers of the chat the plan moved
//! from record their own as they stop); files changed outside the plan since
//! its last linked checkpoint are reconciled; one `PlanChange` (cause
//! `resume`) records what was found (the tasks interrupted since the last
//! resume, a closing chat's included); and the first turn starts with the
//! resume reminder, followed by the task packet (reason `resume`).

use std::path::Path;

use hyperform_context::{ResumeBrief, work_resume};
use hyperform_protocol::work::{
    Actor, ChangeCause, EntryBody, EvidenceRef, PlanChange, PlanStatus, ReconcileFinding,
    ResumePointer, Task, WorkCause,
};
use hyperform_protocol::{AgentId, SessionId, TaskId, WorktreeState};
use hyperform_work::resume::{Alive, interrupted, interrupted_since_resume};
use hyperform_work::{EntryDraft, Facts, PlanState, Proposal, Submission, findings, graph};

use super::checkpoint::{last_checkpoint, recorded_fingerprint, snapshot};
use super::facts::{current, diff_since};
use super::ids::Ids;
use super::plan::{PlanHandle, Submitted};
use crate::session::SessionCore;

/// What resuming found in the workspace.
#[derive(Debug, Default)]
struct Outside {
    /// The tree differs from the one the plan saw last.
    changed: bool,
    /// Files changed or deleted since the plan's last linked checkpoint.
    files: Vec<String>,
    /// The checkpoint the diff started from.
    base: Option<String>,
}

/// `stopping`: helpers of the chat the plan moved from, still stopping.
pub(crate) async fn resume(
    core: &SessionCore,
    plan: &PlanHandle,
    stopping: &[(SessionId, AgentId)],
) {
    let closed = plan.read(|state| {
        matches!(
            state.header.status,
            PlanStatus::Done | PlanStatus::Abandoned
        )
    });
    if closed {
        return;
    }
    let stopped = stopped_tasks(core, plan, stopping);
    let mut facts = current(core, &core.root).await;
    let outside = outside(core, plan, &mut facts).await;
    let state = plan.read(Clone::clone);
    let found = if outside.changed {
        findings(&state, &facts)
    } else {
        Vec::new()
    };
    let new = found
        .iter()
        .any(|finding| !state.findings.contains(finding));
    let mut waiting = plan.read_log(interrupted_since_resume);
    waiting.extend(stopped.iter().map(|(task_id, _)| task_id.clone()));
    waiting.sort();
    waiting.dedup();
    let summary = summary(waiting.len(), outside.files.len(), outside.changed);
    let link = link_proposal(core, &state, &facts, &outside).await;
    let mut proposals: Vec<Proposal> = stopped
        .iter()
        .map(|(task_id, resume)| Proposal::InterruptTask {
            task_id: task_id.clone(),
            resume: resume.clone(),
        })
        .collect();
    if new {
        proposals.push(Proposal::Reconcile {
            findings: found.clone(),
        });
    }
    proposals.extend(link);
    let result = plan.submit(&facts, |state| {
        if let Some(reason) = summary.as_deref() {
            let change = change(state, reason, &waiting, &found, &outside);
            proposals.push(Proposal::Record(Box::new(EntryDraft {
                id: Some(Ids::new(state).entry()),
                ..EntryDraft::new(EntryBody::PlanChange(change))
            })));
        }
        Ok(Submission::new(Actor::Kernel, WorkCause::Resume, proposals))
    });
    match result {
        Ok(Submitted::Rejected(rejection)) => {
            tracing::warn!(reasons = ?rejection.messages(), "resuming the plan was not recorded")
        }
        Err(error) => tracing::warn!(%error, "resuming the plan was not recorded"),
        Ok(_) => {}
    }
    let summary = summary.unwrap_or_else(|| {
        "Resumed: nothing was interrupted and nothing changed outside the plan".into()
    });
    core.work.resume_due(summary);
    core.work.swap_fingerprint(facts.fingerprint);
}

/// The first turn after resuming hears where the plan stands.
pub(crate) fn remind(core: &SessionCore, plan: &PlanHandle) {
    let Some(summary) = core.work.take_resumed() else {
        return;
    };
    let text = plan.read(|state| {
        let tasks: Vec<&Task> = graph::tree_order(state)
            .iter()
            .filter_map(|id| state.task(id))
            .collect();
        work_resume(&ResumeBrief {
            goal: &state.header.goal,
            status: state.header.status,
            summary: &summary,
            tasks: &tasks,
            findings: &state.findings,
            next: graph::next_ready(state, None),
        })
    });
    core.reminders.push(&AgentId::main(), text);
}

/// Active tasks whose helper is gone, with where each can pick up (a
/// worktree only while it still exists).
fn stopped_tasks(
    core: &SessionCore,
    plan: &PlanHandle,
    stopping: &[(SessionId, AgentId)],
) -> Vec<(TaskId, ResumePointer)> {
    let (running, waiting) = core.with_state(|state| {
        let running: Vec<AgentId> = state
            .agents
            .iter()
            .filter(|info| !info.status.is_terminal())
            .map(|info| info.agent_id.clone())
            .collect();
        let waiting: Vec<AgentId> = state
            .agents
            .iter()
            .filter(|info| info.status.is_terminal())
            .filter(|info| {
                info.worktree
                    .as_ref()
                    .is_some_and(|tree| tree.state == WorktreeState::Pending)
            })
            .map(|info| info.agent_id.clone())
            .collect();
        (running, waiting)
    });
    let alive = Alive {
        session: &core.id,
        running: &running,
        waiting: &waiting,
        stopping,
    };
    let mut found = plan.read(|state| interrupted(state, &alive));
    for (_, pointer) in &mut found {
        pointer.worktree = pointer
            .worktree
            .take()
            .filter(|path| Path::new(path).is_dir());
    }
    found
}

/// Whether the tree changed since the plan saw it, and which files
/// changed since its last linked checkpoint (added to `facts`).
async fn outside(core: &SessionCore, plan: &PlanHandle, facts: &mut Facts) -> Outside {
    let Some(now) = facts.fingerprint.clone() else {
        return Outside::default();
    };
    let recorded = plan.read(recorded_fingerprint);
    if recorded.as_ref().is_none_or(|recorded| *recorded == now) {
        return Outside::default();
    }
    let mut outside = Outside {
        changed: true,
        ..Outside::default()
    };
    if let Some((commit, at)) = last_checkpoint(plan) {
        facts.changed_since = Some(at);
        if diff_since(core, &commit, facts).await {
            let files = facts.changed_files.iter().chain(&facts.deleted_files);
            outside.files = files.cloned().collect();
            outside.files.sort();
            outside.files.dedup();
            outside.base = Some(commit);
        }
    }
    outside
}

/// Links the tree as it is now, with a checkpoint for the next resume,
/// unless the plan already saw it with one.
async fn link_proposal(
    core: &SessionCore,
    state: &PlanState,
    facts: &Facts,
    outside: &Outside,
) -> Option<Proposal> {
    let fingerprint = facts.fingerprint.clone()?;
    let seen = state.fingerprint.as_deref() == Some(fingerprint.as_str());
    if seen && !outside.changed {
        return None;
    }
    Some(Proposal::LinkCheckpoint {
        fingerprint,
        checkpoint: snapshot(core).await,
    })
}

/// `Resumed: 2 tasks were interrupted, 1 file changed outside the plan`.
fn summary(interrupted: usize, files: usize, changed: bool) -> Option<String> {
    let mut parts = Vec::new();
    match interrupted {
        0 => {}
        1 => parts.push("1 task was interrupted".to_string()),
        n => parts.push(format!("{n} tasks were interrupted")),
    }
    match files {
        0 if changed => parts.push("the workspace changed outside the plan".to_string()),
        0 => {}
        1 => parts.push("1 file changed outside the plan".to_string()),
        n => parts.push(format!("{n} files changed outside the plan")),
    }
    (!parts.is_empty()).then(|| format!("Resumed: {}", parts.join(", ")))
}

fn change(
    state: &PlanState,
    reason: &str,
    interrupted: &[TaskId],
    found: &[ReconcileFinding],
    outside: &Outside,
) -> PlanChange {
    let mut affected = interrupted.to_vec();
    affected.extend(found.iter().flat_map(|finding| finding.tasks()).cloned());
    affected.sort();
    affected.dedup();
    let evidence = (!outside.files.is_empty())
        .then(|| EvidenceRef::Diff {
            files: outside.files.clone(),
            checkpoint: outside.base.clone(),
            summary: "changed outside the plan".to_string(),
        })
        .into_iter()
        .collect();
    PlanChange {
        cause: ChangeCause::Resume,
        source: None,
        original: format!("revision {}", state.revision()),
        new: String::new(),
        reason: reason.to_string(),
        evidence,
        affected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_counts_what_resuming_found() {
        assert_eq!(
            summary(2, 1, true).as_deref(),
            Some("Resumed: 2 tasks were interrupted, 1 file changed outside the plan")
        );
        assert_eq!(
            summary(1, 0, false).as_deref(),
            Some("Resumed: 1 task was interrupted")
        );
        assert_eq!(
            summary(0, 0, true).as_deref(),
            Some("Resumed: the workspace changed outside the plan")
        );
        assert_eq!(
            summary(0, 3, true).as_deref(),
            Some("Resumed: 3 files changed outside the plan")
        );
        assert_eq!(summary(0, 0, false), None);
    }
}
