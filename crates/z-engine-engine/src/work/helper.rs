//! A task's helper as the plan sees it: the task is bound when the helper
//! starts, its report is recorded when it ends (an `Unreported` one when
//! it saved none) with what the engine observed, and how its worktree
//! change came back is integrated. The engine only reports what it saw;
//! the kernel decides every change of the task.

use z_engine_protocol::work::{
    Actor, BlockReason, EvidenceRef, Integration, ReportDraft, Task, TaskDispatch, TaskStatus,
    WorkCause,
};
use z_engine_protocol::{AgentId, CallId, CheckRecord, TaskId};
use z_engine_tools::names;
use z_engine_work::{Proposal, Rejection, RunFacts, Submission};

use super::facts::{agent, instant};
use super::lifecycle::{attached, report};
use super::plan::{PlanHandle, Submitted};
use crate::session::SessionCore;

/// A check record as plan evidence.
pub(crate) fn check_evidence(core: &SessionCore, record: &CheckRecord) -> EvidenceRef {
    EvidenceRef::Check {
        session_id: core.id.clone(),
        record_id: record.record_id.clone(),
        passed: record.passed,
        fingerprint: record.fingerprint_after.clone(),
        kind: Some(record.kind),
        label: record.label.clone(),
        at: record.started_at.saturating_add(record.duration_ms),
    }
}

/// The checks `helper` ran, as evidence.
pub(crate) fn checks_of(core: &SessionCore, helper: &AgentId) -> Vec<EvidenceRef> {
    let records: Vec<CheckRecord> = core.with_state(|state| {
        let ran = state
            .checks
            .iter()
            .filter(|record| record.agent_id == *helper);
        ran.cloned().collect()
    });
    records
        .iter()
        .map(|record| check_evidence(core, record))
        .collect()
}

/// Binds `task` to `helper`, started by `caller`; the kernel checks the
/// dispatch again under the plan lock.
pub(crate) fn bind(
    core: &SessionCore,
    plan: &PlanHandle,
    caller: &AgentId,
    call_id: &CallId,
    task: &TaskId,
    helper: &AgentId,
    worktree: Option<String>,
) -> Result<(), String> {
    let dispatch = TaskDispatch {
        session_id: core.id.clone(),
        agent_id: helper.clone(),
        worktree,
        attempt: 0,
        at: 0,
    };
    let cause = WorkCause::Tool {
        tool: names::AGENT.to_string(),
        call_id: Some(call_id.clone()),
    };
    let proposals = vec![Proposal::DispatchTask {
        task_id: task.clone(),
        dispatch,
    }];
    let actor = agent(core, caller);
    match plan.submit(&instant(core), |_| {
        Ok(Submission::new(actor, cause, proposals))
    })? {
        Submitted::Accepted(_) => Ok(()),
        Submitted::Unchanged => Err(format!("task {task} was not dispatched")),
        Submitted::Rejected(rejection) => Err(refusal(task, &rejection)),
    }
}

pub(crate) fn refusal(task: &TaskId, rejection: &Rejection) -> String {
    let reasons: Vec<String> = rejection
        .messages()
        .iter()
        .map(|reason| format!("- {reason}"))
        .collect();
    format!("Task {task} cannot be dispatched:\n{}", reasons.join("\n"))
}

/// A report `helper` sent for the task it runs, saved until its run ends;
/// `None` when it does not run `task` (the report then goes to the kernel
/// at once).
pub(crate) fn save_report(
    core: &SessionCore,
    helper: &AgentId,
    task: &TaskId,
    draft: &ReportDraft,
) -> Option<Result<(), String>> {
    let plan = attached(core)?;
    running(&plan, task, helper).then_some(())?;
    let checked = z_engine_work::report::fold(Some(draft), &RunFacts::default(), 0, 0);
    Some(match checked {
        Ok(_) => {
            core.work.save_report(helper, task, draft.clone());
            Ok(())
        }
        Err(reason) => Err(reason.to_string()),
    })
}

/// `helper`'s run on `task` ended: its saved report (or an `Unreported`
/// one) is recorded with what the engine observed. `true` when the report
/// was recorded and the task still runs, so its change comes back next.
pub(crate) fn conclude(
    core: &SessionCore,
    helper: &AgentId,
    task: &TaskId,
    run: RunFacts,
    untrusted: bool,
) -> bool {
    let saved = core.work.take_report(helper, task);
    let Some(plan) = attached(core).filter(|plan| running(plan, task, helper)) else {
        return false;
    };
    let mut facts = instant(core);
    facts.run = run;
    facts.bound_task = Some(task.clone());
    if let Some(draft) = saved {
        let reported = Proposal::Report {
            task_id: task.clone(),
            report: Some(Box::new(draft)),
        };
        let as_helper = z_engine_work::Facts {
            untrusted_input: untrusted,
            ..facts.clone()
        };
        match plan.submit(&as_helper, |_| {
            Ok(Submission::new(
                agent(core, helper),
                ended(),
                vec![reported],
            ))
        }) {
            Ok(Submitted::Accepted(_)) => return true,
            result => report(core, result, "The helper's report was not recorded"),
        }
    }
    let missing = Proposal::Report {
        task_id: task.clone(),
        report: None,
    };
    let result = plan.submit(&facts, |_| {
        Ok(Submission::new(Actor::Kernel, ended(), vec![missing]))
    });
    report(core, result, "The helper's missing report was not recorded");
    false
}

/// How `helper`'s change came back, when it is the change its task waits
/// for; returns the task.
pub(crate) fn integrate(
    core: &SessionCore,
    helper: &AgentId,
    integration: Integration,
) -> Option<TaskId> {
    let plan = attached(core)?;
    let task = plan.read(|state| {
        state
            .tasks
            .values()
            .find(|task| awaits(task, helper, &integration))
            .map(|task| task.id.clone())
    })?;
    let proposals = vec![Proposal::Integrate {
        task_id: task.clone(),
        integration,
    }];
    let result = plan.submit(&instant(core), |_| {
        Ok(Submission::new(Actor::Kernel, ended(), proposals))
    });
    let accepted = matches!(result, Ok(Submitted::Accepted(_)));
    report(
        core,
        result,
        "The helper's change was not recorded on its task",
    );
    accepted.then_some(task)
}

/// The task's current run is `helper`'s and its change has not come back;
/// after a conflict only the kept worktree's apply or discard still counts.
pub(super) fn awaits(task: &Task, helper: &AgentId, next: &Integration) -> bool {
    let run = &task.run;
    let mine = run
        .dispatch
        .as_ref()
        .is_some_and(|dispatch| dispatch.agent_id == *helper);
    let conflicted = matches!(
        task.status,
        TaskStatus::Blocked {
            reason: BlockReason::MergeConflict { .. }
        }
    );
    mine && match &run.integration {
        None | Some(Integration::PendingApply { .. }) => task.status == TaskStatus::Active,
        Some(Integration::Conflict { .. }) => {
            conflicted && matches!(next, Integration::Applied { .. } | Integration::Discarded)
        }
        Some(Integration::Applied { .. } | Integration::Discarded) => false,
    }
}

/// `helper` still runs `task`: Active, owned by it, and its current run.
fn running(plan: &PlanHandle, task: &TaskId, helper: &AgentId) -> bool {
    plan.read(|state| {
        state.task(task).is_some_and(|task| {
            task.status == TaskStatus::Active
                && task
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.agent_id == *helper)
                && task
                    .run
                    .dispatch
                    .as_ref()
                    .is_some_and(|dispatch| dispatch.agent_id == *helper)
        })
    })
}

pub(super) fn ended() -> WorkCause {
    WorkCause::Tool {
        tool: names::AGENT.to_string(),
        call_id: None,
    }
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::work::{PlanDepth, PlanStatus};
    use z_engine_protocol::{PlanId, SessionId};
    use z_engine_work::{Facts, PlanDraft, PlanState, TaskDraft};

    use super::*;

    fn dispatched(status: TaskStatus, integration: Option<Integration>) -> Task {
        let draft = PlanDraft {
            plan_id: PlanId::new(),
            project_root: "/p".into(),
            goal: "Ship retries".into(),
            document: String::new(),
            depth: PlanDepth::Standard,
            status: PlanStatus::Active,
        };
        let task = TaskDraft {
            id: Some(TaskId("t1".into())),
            title: "Parse".into(),
            ..TaskDraft::default()
        };
        let create = vec![Proposal::CreateTask(Box::new(task))];
        let submission = Submission::new(Actor::User, WorkCause::Edit, create);
        let records = z_engine_work::create(&draft, &submission, &Facts::at(1)).unwrap();
        let state = PlanState::fold(&records).unwrap();
        let mut task = state.task(&TaskId("t1".into())).unwrap().clone();
        task.status = status;
        task.run.integration = integration;
        task.run.dispatch = Some(TaskDispatch {
            session_id: SessionId::new(),
            agent_id: AgentId::from("agt_1"),
            worktree: Some("/wt/agt_1".into()),
            attempt: 1,
            at: 1,
        });
        task
    }

    #[test]
    fn only_the_helpers_unsettled_change_reaches_its_task() {
        let helper = AgentId::from("agt_1");
        let applied = Integration::Applied { files: vec![] };
        let running = dispatched(TaskStatus::Active, None);
        assert!(awaits(&running, &helper, &applied));
        assert!(!awaits(&running, &AgentId::from("agt_2"), &applied));
        let pending = Integration::PendingApply {
            worktree: "/wt/agt_1".into(),
        };
        assert!(awaits(
            &dispatched(TaskStatus::Active, Some(pending)),
            &helper,
            &applied
        ));
        let done = dispatched(TaskStatus::Verifying, Some(applied.clone()));
        assert!(!awaits(&done, &helper, &Integration::Discarded));
    }

    #[test]
    fn a_conflicted_task_takes_only_its_kept_worktrees_apply_or_discard() {
        let helper = AgentId::from("agt_1");
        let conflict = Integration::Conflict {
            paths: vec!["src/a.rs".into()],
            worktree: "/wt/agt_1".into(),
        };
        let blocked = TaskStatus::Blocked {
            reason: BlockReason::MergeConflict {
                detail: "src/a.rs".into(),
            },
        };
        let task = dispatched(blocked, Some(conflict.clone()));
        assert!(awaits(
            &task,
            &helper,
            &Integration::Applied { files: vec![] }
        ));
        assert!(awaits(&task, &helper, &Integration::Discarded));
        assert!(!awaits(&task, &helper, &conflict));
    }
}
