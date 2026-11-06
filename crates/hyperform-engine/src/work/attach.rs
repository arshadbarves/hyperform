//! Attaching a plan to a session: the `attachPlan` / `detachPlan` commands,
//! the plan the log names when the session reopens, and the plan a first
//! task or structured plan starts. The chat that attaches a plan drives it
//! (see `driver`), and opening or attaching a plan resumes it (see
//! `resume`). The GUI gets a fresh snapshot.

use std::sync::Arc;

use hyperform_protocol::work::{Actor, PlanStatus, TaskScope, WorkCause};
use hyperform_protocol::{Event, NoticeLevel, PlanId};
use hyperform_store::LogRecord;
use hyperform_work::{PlanDraft, Submission, assess};

use super::bound::{DETACHED, Halt};
use super::driver::{drive, may_reattach};
use super::facts::{OFF, enabled, instant};
use super::plan::PlanHandle;
use super::resume::resume;
use crate::session::{SessionCore, emit_snapshot};

pub(crate) async fn attach_command(core: &Arc<SessionCore>, plan_id: &PlanId) {
    if !enabled(core) {
        core.events.notice(NoticeLevel::Info, OFF);
        return;
    }
    match core.shared.plans.open(&core.root, plan_id) {
        Ok(plan) => {
            let read_only = plan.read_only();
            if attach(core, Arc::clone(&plan), true) {
                let stopping = drive(core, &plan);
                resume(core, &plan, &stopping).await;
                emit_snapshot(core);
            }
            if let Some(why) = read_only {
                core.events.notice(
                    NoticeLevel::Warn,
                    format!("The plan is read-only ({why}); changes to it are refused."),
                );
            }
        }
        Err(error) => core.events.notice(
            NoticeLevel::Warn,
            format!("The plan was not attached: {error}"),
        ),
    }
}

pub(crate) fn detach_command(core: &SessionCore) {
    if core.work.plan().is_none() || !core.journal.append_or_report(&LogRecord::PlanDetached) {
        return;
    }
    if let Some(previous) = core.work.replace(None, false) {
        let_go(core, &previous);
    }
    emit_snapshot(core);
}

/// The chat no longer holds `plan`: it stops hearing about it, its
/// helpers on it stop with resume pointers, and no chat holds it now.
fn let_go(core: &SessionCore, plan: &PlanHandle) {
    plan.unwatch(&core.events);
    plan.released(core);
    core.work
        .helpers()
        .halt(&plan.id, Halt::Interrupt(DETACHED), |_| true);
}

/// Logs the attachment first; false when the log refused it.
fn attach(core: &SessionCore, plan: Arc<PlanHandle>, resumed: bool) -> bool {
    if core
        .work
        .plan()
        .is_some_and(|current| current.id == plan.id)
    {
        return false;
    }
    let record = LogRecord::PlanAttached {
        plan_id: plan.id.clone(),
    };
    if !core.journal.append_or_report(&record) {
        return false;
    }
    plan.watch(&core.events);
    if let Some(previous) = core.work.replace(Some(plan), resumed) {
        let_go(core, &previous);
    }
    true
}

/// Reattaches the plan the session log names and resumes it (before the
/// first snapshot), unless another chat drives it now.
pub(crate) async fn reattach(core: &Arc<SessionCore>, plan_id: Option<PlanId>) {
    let Some(plan_id) = plan_id.filter(|_| enabled(core)) else {
        return;
    };
    match core.shared.plans.open(&core.root, &plan_id) {
        Ok(plan) => {
            if !may_reattach(core, &plan) {
                return;
            }
            plan.watch(&core.events);
            core.work.replace(Some(Arc::clone(&plan)), true);
            let stopping = drive(core, &plan);
            resume(core, &plan, &stopping).await;
        }
        Err(error) => core.events.notice(
            NoticeLevel::Warn,
            format!("The work plan of this chat could not be opened: {error}"),
        ),
    }
}

/// The attached open plan, or a new one `actor` starts (`true`): a draft
/// while planning, else active. A finished or abandoned plan is replaced.
/// `scope` is the first task's, for the plan's depth.
pub(crate) fn ensure(
    core: &Arc<SessionCore>,
    actor: &Actor,
    cause: &WorkCause,
    goal: &str,
    scope: &TaskScope,
) -> Result<(Arc<PlanHandle>, bool), String> {
    let closed = |plan: &PlanHandle| {
        plan.read(|state| {
            matches!(
                state.header.status,
                PlanStatus::Done | PlanStatus::Abandoned
            )
        })
    };
    if let Some(plan) = core.work.plan().filter(|plan| !closed(plan)) {
        return Ok((plan, false));
    }
    let settings = core.settings();
    let depth = assess([scope], &settings.settings.work.sensitive_paths).suggested;
    let status = if core.planning() {
        PlanStatus::Draft
    } else {
        PlanStatus::Active
    };
    let draft = PlanDraft {
        plan_id: PlanId::new(),
        project_root: core.root.to_string_lossy().into_owned(),
        goal: goal.trim().to_string(),
        document: String::new(),
        depth,
        status,
    };
    let submission = Submission::new(actor.clone(), cause.clone(), Vec::new());
    let plan = core
        .shared
        .plans
        .create(&core.root, &draft, &submission, &instant(core))?;
    if !attach(core, Arc::clone(&plan), false) {
        return Err("the plan was saved but could not be attached to this chat".into());
    }
    drive(core, &plan);
    core.events.emit(Event::WorkUpdated {
        update: Box::new(plan.full_update()),
    });
    Ok((plan, true))
}
