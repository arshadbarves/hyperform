//! The user's side of a plan: approving it (the plan review or the Plan
//! tab), turning planning on over an active plan, and Plan-tab edits
//! (audits, findings and serial retries included; pausing, holding,
//! stopping and abandoning stop the helpers concerned). A refused change
//! becomes a notice with the kernel's reasons.

use std::sync::Arc;

use hyperform_protocol::work::{
    Actor, ChangeCause, EntryBody, EvidenceRef, Integration, PlanChange, PlanItemStatus,
    PlanStatus, WorkCause, WorkEdit,
};
use hyperform_protocol::{AgentId, NoticeLevel, PlanId, TaskId};
use hyperform_work::{EntryDraft, PlanState, Proposal, Submission, assess_plan};

use super::facts::{OFF, enabled, instant};
use super::ids::Ids;
use super::interrupt::halt_for;
use super::plan::{PlanHandle, Submitted};
use super::{audit, findings, interrupt, merged};
use crate::orchestration;
use crate::session::SessionCore;

/// The attached plan while `work_plans` is On.
pub(crate) fn attached(core: &SessionCore) -> Option<Arc<PlanHandle>> {
    enabled(core).then(|| core.work.plan()).flatten()
}

/// The plan review was approved: proposed requirements and invariants are
/// accepted, an edited plan replaces the document, and a Draft or
/// Replanning plan becomes Active.
pub(crate) fn approve(core: &SessionCore, edited_plan: Option<&str>) {
    let Some(plan) = attached(core) else {
        return;
    };
    reassess_depth(core, &plan, WorkCause::Approval);
    let result = plan.submit(&instant(core), |state| {
        let proposals = approval(state, edited_plan);
        Ok(Submission::new(Actor::User, WorkCause::Approval, proposals))
    });
    report(core, result, "The plan was not approved");
}

/// The plan's depth assessed again from all its open tasks, recorded by
/// the kernel when it changed.
pub(crate) fn reassess_depth(core: &SessionCore, plan: &PlanHandle, cause: WorkCause) {
    let settings = core.settings();
    let sensitive = &settings.settings.work.sensitive_paths;
    let result = plan.submit(&instant(core), |state| {
        let depth = assess_plan(state, sensitive).suggested;
        let proposals = vec![Proposal::SetDepth { depth }];
        Ok(Submission::new(Actor::Kernel, cause, proposals))
    });
    report(core, result, "The plan depth was not updated");
}

/// Planning was turned on: an Active plan goes back to Replanning.
pub(crate) fn planning_on(core: &SessionCore) {
    let Some(plan) = attached(core) else {
        return;
    };
    let result = plan.submit(&instant(core), |state| {
        let proposals = if state.header.status == PlanStatus::Active {
            let reason = "The user turned planning on";
            replan(
                &mut Ids::new(state),
                ChangeCause::User,
                reason,
                &[PlanStatus::Active],
                Vec::new(),
                Vec::new(),
            )
        } else {
            Vec::new()
        };
        Ok(Submission::new(Actor::User, WorkCause::Edit, proposals))
    });
    report(core, result, "The plan was not reopened for replanning");
}

/// A Plan-tab edit (`Command::EditWork`), made as the user. An audit
/// request runs the audit agent; adding a finding creates its task; a
/// serial retry removes the worktree its conflict kept; a recheck runs a
/// Verifying task's checks again; approving re-assesses the depth first.
pub(crate) fn edit(core: &Arc<SessionCore>, plan_id: &PlanId, edit: WorkEdit) {
    if !enabled(core) {
        core.events.notice(NoticeLevel::Info, OFF);
        return;
    }
    let Some(plan) = attached(core).filter(|plan| plan.id == *plan_id) else {
        core.events
            .notice(NoticeLevel::Warn, "That plan is not attached to this chat.");
        return;
    };
    let kept = match &edit {
        WorkEdit::RequestAudit => return audit::request(core, true),
        WorkEdit::AddFinding { finding_id } => return findings::add(core, &plan, finding_id),
        WorkEdit::RecheckTask { task_id } => return merged::recheck(core, &plan, task_id),
        WorkEdit::RetrySerially { task_id } => {
            plan.read(|state| kept_worktree(core, state, task_id))
        }
        WorkEdit::Approve => {
            reassess_depth(core, &plan, WorkCause::Approval);
            None
        }
        _ => None,
    };
    let halt = halt_for(&edit);
    let result = plan.submit(&instant(core), |state| {
        Ok(match edit {
            WorkEdit::Approve => {
                Submission::new(Actor::User, WorkCause::Approval, approval(state, None))
            }
            WorkEdit::RequestReplan => {
                let reason = "The user asked to replan";
                let proposals = replan(
                    &mut Ids::new(state),
                    ChangeCause::User,
                    reason,
                    &[],
                    Vec::new(),
                    Vec::new(),
                );
                Submission::new(Actor::User, WorkCause::Edit, proposals)
            }
            edit => Submission::edit(edit),
        })
    });
    let accepted = matches!(result, Ok(Submitted::Accepted(_)));
    report(core, result, "The plan change was not applied");
    if let (true, Some(halt)) = (accepted, halt) {
        interrupt::halt(core, &plan, halt);
    }
    if let (true, Some(agent)) = (accepted, kept) {
        let core = Arc::clone(core);
        tokio::spawn(async move { orchestration::drop_kept(&core, &agent).await });
    }
}

/// The helper whose worktree a merge conflict on `task` kept.
fn kept_worktree(core: &SessionCore, state: &PlanState, task: &TaskId) -> Option<AgentId> {
    let run = &state.task(task)?.run;
    let Some(Integration::Conflict { worktree, .. }) = &run.integration else {
        return None;
    };
    let agent = run.dispatch.as_ref()?.agent_id.clone();
    let same = core.with_state(|s| {
        s.agents.iter().any(|info| {
            info.agent_id == agent && info.worktree.as_ref().is_some_and(|w| w.path == *worktree)
        })
    });
    same.then_some(agent)
}

fn approval(state: &PlanState, edited_plan: Option<&str>) -> Vec<Proposal> {
    let mut proposals = Vec::new();
    if let Some(document) = edited_plan.filter(|text| *text != state.header.document) {
        proposals.push(Proposal::EditPlan {
            goal: None,
            document: Some(document.to_string()),
        });
    }
    proposals.extend(
        state
            .items()
            .filter(|item| item.status == PlanItemStatus::Proposed)
            .map(|item| Proposal::SetItemStatus {
                item_id: item.id.clone(),
                status: PlanItemStatus::Active,
            }),
    );
    if matches!(
        state.header.status,
        PlanStatus::Draft | PlanStatus::Replanning
    ) {
        proposals.push(Proposal::SetPlanStatus {
            status: PlanStatus::Active,
            only_from: vec![PlanStatus::Draft, PlanStatus::Replanning],
        });
    }
    proposals
}

/// Moves the plan to Replanning (from `only_from`; any status when empty)
/// and records the plan change that later revisions group under.
pub(crate) fn replan(
    ids: &mut Ids,
    cause: ChangeCause,
    reason: &str,
    only_from: &[PlanStatus],
    evidence: Vec<EvidenceRef>,
    affected: Vec<TaskId>,
) -> Vec<Proposal> {
    let change = PlanChange {
        cause,
        source: None,
        original: format!("revision {}", ids.state().revision()),
        new: String::new(),
        reason: reason.to_string(),
        evidence,
        affected,
    };
    vec![
        Proposal::SetPlanStatus {
            status: PlanStatus::Replanning,
            only_from: only_from.to_vec(),
        },
        Proposal::Record(Box::new(EntryDraft {
            id: Some(ids.entry()),
            ..EntryDraft::new(EntryBody::PlanChange(change))
        })),
    ]
}

/// A refusal or a failed write becomes a warning notice.
pub(crate) fn report(core: &SessionCore, result: Result<Submitted, String>, what: &str) {
    let detail = match result {
        Ok(Submitted::Rejected(rejection)) => rejection.messages().join("; "),
        Err(error) => error,
        Ok(_) => return,
    };
    core.events
        .notice(NoticeLevel::Warn, format!("{what}: {detail}"));
}
