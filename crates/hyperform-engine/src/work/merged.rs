//! A helper's change in the project tree: the task's check criteria run
//! there (each kind once, recorded as the helper's checks), the records
//! become the criteria's evidence, and the completion gate decides: Done,
//! Failed when a check failed (the kernel propagates it), or still
//! Verifying while other criteria wait. The user can run them again for a
//! task still Verifying (`recheckTask`).

use std::sync::Arc;

use hyperform_protocol::work::{Actor, CriterionMethod, Task, TaskStatus, WorkCause};
use hyperform_protocol::{AgentId, CheckKind, CheckRecord, NoticeLevel, TaskId};
use hyperform_tools::names;
use hyperform_work::{Proposal, Submission};

use super::coordinate::{News, tell_coordinator};
use super::evidence::{self, check_links, helper_of};
use super::facts::{current, instant};
use super::lifecycle::{attached, report};
use super::plan::{PlanHandle, Submitted};
use crate::session::SessionCore;
use crate::verify::{CheckRun, run_recorded};

/// Verifies `task` after `helper`'s change was applied, then tells the
/// coordinator.
pub(crate) async fn verify_merged(core: Arc<SessionCore>, helper: AgentId, task: TaskId) {
    let Some(plan) = attached(&core) else {
        return;
    };
    let kinds = plan.read(|state| state.task(&task).map(check_kinds).unwrap_or_default());
    let profile = core.checks.profile();
    let mut records = Vec::new();
    for kind in kinds {
        let Some(spec) = profile.checks.iter().find(|spec| spec.kind == kind) else {
            let label = kind.label();
            core.events.notice(
                NoticeLevel::Warn,
                format!(
                    "Task {task}'s integration check did not run: this project has no {label} check"
                ),
            );
            continue;
        };
        let run = CheckRun {
            agent_id: helper.clone(),
            root: core.root.clone(),
            cancel: core.cancel.child_token(),
            progress: None,
        };
        match run_recorded(&core, spec, run).await {
            Ok(record) => records.push(record),
            Err(error) => core.events.notice(
                NoticeLevel::Warn,
                format!("Task {task}'s integration check did not run: {error}"),
            ),
        }
    }
    link(&core, &plan, &task, &records);
    let news = complete(&core, &plan, &task, &records).await;
    tell_coordinator(&core, &helper, &task, news).await;
}

/// The user's `recheckTask`: the kernel confirms the task is Verifying,
/// then its checks run again in the project tree.
pub(crate) fn recheck(core: &Arc<SessionCore>, plan: &PlanHandle, task: &TaskId) {
    let proposals = vec![Proposal::Recheck {
        task_id: task.clone(),
    }];
    let result = plan.submit(&instant(core), |_| {
        Ok(Submission::new(Actor::User, WorkCause::Edit, proposals))
    });
    if !matches!(result, Ok(Submitted::Unchanged | Submitted::Accepted(_))) {
        report(core, result, "The checks were not run again");
        return;
    }
    let helper = plan.read(|state| state.task(task).map(helper_of));
    let helper = helper.unwrap_or_else(AgentId::main);
    tokio::spawn(verify_merged(Arc::clone(core), helper, task.clone()));
}

/// Each check kind the task's open criteria name, once.
fn check_kinds(task: &Task) -> Vec<CheckKind> {
    let mut kinds = Vec::new();
    for criterion in &task.acceptance {
        if let CriterionMethod::Check { kind } = criterion.method
            && !kinds.contains(&kind)
        {
            kinds.push(kind);
        }
    }
    kinds
}

fn link(core: &SessionCore, plan: &PlanHandle, task: &TaskId, records: &[CheckRecord]) {
    let Some(last) = records.last() else {
        return;
    };
    let mut facts = instant(core);
    facts.fingerprint = Some(last.fingerprint_after.clone());
    let result = plan.submit(&facts, |state| {
        let proposals = state.task(task).map_or_else(Vec::new, |task| {
            records
                .iter()
                .flat_map(|record| check_links(core, task, record))
                .collect()
        });
        Ok(Submission::new(Actor::Kernel, evidence::cause(), proposals))
    });
    report(
        core,
        result,
        "The integration checks were not linked to the task",
    );
}

/// The completion gate for a Verifying `task` after `records` ran.
pub(crate) async fn complete(
    core: &SessionCore,
    plan: &PlanHandle,
    task: &TaskId,
    records: &[CheckRecord],
) -> News {
    let facts = current(core, &core.root).await;
    let status = |status: TaskStatus| {
        let proposals = vec![Proposal::SetTaskStatus {
            task_id: task.clone(),
            status,
        }];
        plan.submit(&facts, |_| {
            Ok(Submission::new(Actor::Kernel, cause(), proposals))
        })
    };
    match status(TaskStatus::Done) {
        Ok(Submitted::Accepted(_)) => return News::Verified,
        Ok(Submitted::Unchanged | Submitted::Rejected(_)) => {}
        Err(error) => tracing::warn!(%error, "the merged task was not completed"),
    }
    let Some(failed) = records.iter().find(|record| !record.passed) else {
        return News::StillVerifying;
    };
    let reason = format!(
        "the integration check {} failed in the project tree",
        failed.label
    );
    let result = status(TaskStatus::Failed { reason });
    let failed = matches!(result, Ok(Submitted::Accepted(_)));
    report(core, result, "The failed integration was not recorded");
    if failed {
        News::CheckFailed
    } else {
        News::StillVerifying
    }
}

fn cause() -> WorkCause {
    WorkCause::Tool {
        tool: names::AGENT.to_string(),
        call_id: None,
    }
}
