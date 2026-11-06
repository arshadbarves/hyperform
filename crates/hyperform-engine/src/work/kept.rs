//! The worktree of a task helper whose task no longer waits for its change
//! (the helper was interrupted: paused, held, its chat closed, the plan
//! moved or was detached). The work is the task's to resume, so applying
//! it by hand is refused; discarding it drops it from the task's resume
//! pointer, and the next run starts clean.

use hyperform_protocol::work::{Actor, Integration, Task};
use hyperform_protocol::{AgentId, TaskId};
use hyperform_work::{Proposal, Submission};

use super::facts::{enabled, instant};
use super::helper::{awaits, ended};
use super::lifecycle::{attached, report};
use crate::session::SessionCore;

/// Why applying `agent`'s change (a helper of `task`) by hand is refused:
/// its task does not wait for that change (or this chat no longer holds
/// its plan). With work plans off nothing is refused.
pub(crate) fn foreign_apply(core: &SessionCore, agent: &AgentId, task: &TaskId) -> Option<String> {
    if !enabled(core) {
        return None;
    }
    let waits =
        attached(core).is_some_and(|plan| plan.read(|state| waits_for(state.task(task), agent)));
    (!waits).then(|| {
        format!("This helper's work belongs to task {task}. Resume the task or discard its work.")
    })
}

/// `agent`'s kept worktree was discarded: the resume pointer of `task`
/// drops it (when the pointer names `agent`).
pub(crate) fn kept_discarded(core: &SessionCore, agent: &AgentId, task: &TaskId) {
    let Some(plan) = attached(core) else {
        return;
    };
    let proposals = vec![Proposal::DiscardResumeWorktree {
        task_id: task.clone(),
        agent_id: agent.clone(),
    }];
    let result = plan.submit(&instant(core), |_| {
        Ok(Submission::new(Actor::Kernel, ended(), proposals))
    });
    report(
        core,
        result,
        "The discarded worktree was not dropped from its task",
    );
}

/// The task's current run is `agent`'s and still waits for its change
/// (its apply card, or a conflict the kept worktree resolves).
fn waits_for(task: Option<&Task>, agent: &AgentId) -> bool {
    let applied = Integration::Applied { files: Vec::new() };
    task.is_some_and(|task| awaits(task, agent, &applied))
}

#[cfg(test)]
mod tests {
    use hyperform_protocol::work::{
        BlockReason, PlanDepth, PlanStatus, ResumePointer, TaskDispatch, TaskStatus, WorkCause,
    };
    use hyperform_protocol::{PlanId, SessionId};
    use hyperform_work::{Facts, PlanDraft, PlanState, TaskDraft};

    use super::*;

    fn run(status: TaskStatus, integration: Option<Integration>) -> Task {
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
        let records = hyperform_work::create(&draft, &submission, &Facts::at(1)).unwrap();
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
    fn only_a_change_its_task_waits_for_may_be_applied_by_hand() {
        let helper = AgentId::from("agt_1");
        let pending = Integration::PendingApply {
            worktree: "/wt/agt_1".into(),
        };
        let waiting = run(TaskStatus::Active, Some(pending));
        assert!(waits_for(Some(&waiting), &helper));
        assert!(!waits_for(Some(&waiting), &AgentId::from("agt_2")));
        let conflict = Integration::Conflict {
            paths: vec!["src/a.rs".into()],
            worktree: "/wt/agt_1".into(),
        };
        let blocked = TaskStatus::Blocked {
            reason: BlockReason::MergeConflict {
                detail: "src/a.rs".into(),
            },
        };
        assert!(waits_for(Some(&run(blocked, Some(conflict))), &helper));

        let mut interrupted = run(TaskStatus::Pending, None);
        interrupted.resume = Some(ResumePointer {
            session_id: SessionId::new(),
            agent_id: helper.clone(),
            worktree: Some("/wt/agt_1".into()),
            note: "the plan was paused".into(),
        });
        assert!(!waits_for(Some(&interrupted), &helper));
        assert!(!waits_for(None, &helper), "no attached plan knows the task");
    }
}
