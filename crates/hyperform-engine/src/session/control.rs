//! Quick command handlers: approvals, questions, plans, mode, model,
//! effort, the steering queue, and job kills. Each persists its change
//! before announcing it.

use std::sync::Arc;

use hyperform_config::{RuleKind, add_permission_rule, project_local_file};
use hyperform_protocol::{
    ApprovalDecision, Effort, Event, JobId, NoticeLevel, PermissionMode, PlanDecision,
    QuestionAnswer, RequestId,
};
use hyperform_store::LogRecord;

use super::meta::write_meta;
use crate::session::SessionCore;
use crate::settings::models;
use crate::sync::lock;
use crate::work;

pub(crate) fn resolve_approval(core: &SessionCore, id: &RequestId, decision: ApprovalDecision) {
    let result = core
        .broker
        .resolve_approval(id, decision, |request, decision| match decision {
            ApprovalDecision::AllowSession { rule } => grant(core, rule, false),
            ApprovalDecision::AllowProject { rule } => grant(core, rule, request.can_persist),
            ApprovalDecision::AllowOnce | ApprovalDecision::Deny { .. } => {}
        });
    if let Err(error) = result {
        core.events
            .notice(NoticeLevel::Warn, format!("approval not applied: {error}"));
    }
}

/// Adds the rule for this session and, when allowed, to the project's
/// local settings file.
fn grant(core: &SessionCore, rule: &str, persist: bool) {
    if let Err(error) = lock(&core.policy).add_session_rule(rule) {
        core.events.notice(
            NoticeLevel::Warn,
            format!("the rule `{rule}` was not added: {error}"),
        );
        return;
    }
    if !persist {
        return;
    }
    let file = project_local_file(&core.root);
    if let Err(error) = add_permission_rule(&file, RuleKind::Allow, rule) {
        core.events.notice(
            NoticeLevel::Warn,
            format!("the rule `{rule}` applies to this session only: {error}"),
        );
    }
}

pub(crate) fn answer_question(
    core: &SessionCore,
    id: &RequestId,
    answers: Option<Vec<QuestionAnswer>>,
) {
    if let Err(error) = core.broker.answer(id, answers) {
        core.events
            .notice(NoticeLevel::Warn, format!("answer not applied: {error}"));
    }
}

/// Approving a plan ends plan mode and makes the attached work plan
/// Active; the level changes only when the decision names one.
pub(crate) fn resolve_plan(core: &SessionCore, id: &RequestId, decision: PlanDecision) {
    let result = core.broker.resolve_plan(id, decision, |decision| {
        if let PlanDecision::Approve {
            mode, edited_plan, ..
        } = decision
        {
            if let Some(mode) = *mode {
                set_mode(core, mode);
            }
            work::approve(core, edited_plan.as_deref());
            set_planning(core, false);
        }
    });
    if let Err(error) = result {
        core.events.notice(
            NoticeLevel::Warn,
            format!("plan decision not applied: {error}"),
        );
    }
}

pub(crate) fn set_mode(core: &SessionCore, mode: PermissionMode) {
    if core.mode() == mode {
        return;
    }
    let record = LogRecord::ModeChanged {
        mode: mode.into(),
        planning: Some(core.planning()),
    };
    if core.journal.append_or_report(&record) {
        core.with_state(|state| state.mode = mode);
        core.events.emit(Event::ModeChanged { mode });
    }
}

pub(crate) fn set_planning(core: &SessionCore, planning: bool) {
    if core.planning() == planning {
        return;
    }
    if core
        .journal
        .append_or_report(&LogRecord::PlanningChanged { planning })
    {
        core.with_state(|state| state.planning = planning);
        core.events.emit(Event::PlanningChanged { planning });
        if planning {
            work::planning_on(core);
        }
    }
}

pub(crate) fn set_model(core: &SessionCore, model: String) {
    let model = model.trim().to_string();
    if model.is_empty() {
        core.events
            .notice(NoticeLevel::Warn, "choose a model id to switch models");
        return;
    }
    let record = LogRecord::ModelChanged {
        model: model.clone(),
    };
    if !core.journal.append_or_report(&record) {
        return;
    }
    let settings = core.settings();
    let catalog = core.catalog();
    let limit = models::context_window(&settings.settings, catalog.as_deref(), &model);
    core.with_state(|state| {
        state.model = model.clone();
        state.context_limit = limit;
        state.meter.reset();
    });
    core.events.emit(Event::ModelChanged { model });
    write_meta(core);
}

pub(crate) fn set_effort(core: &SessionCore, effort: Option<Effort>) {
    if core
        .journal
        .append_or_report(&LogRecord::EffortChanged { effort })
    {
        core.with_state(|state| state.effort = effort);
        core.events.emit(Event::EffortChanged { effort });
    }
}

pub(crate) fn steer(core: &SessionCore, text: String) {
    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }
    let queued = core.with_state(|state| {
        state.queue.push(text);
        state.queue.clone()
    });
    core.events.emit(Event::QueueChanged { queued });
}

/// A prompt command sent while a turn runs is not queued.
pub(crate) fn command_waits(core: &SessionCore, name: &str) {
    core.events.notice(
        NoticeLevel::Warn,
        format!("/{name} waits for an idle session; run it when this turn ends."),
    );
}

pub(crate) fn edit_queue(core: &SessionCore, queued: Vec<String>) {
    let queued: Vec<String> = queued
        .into_iter()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect();
    core.with_state(|state| state.queue = queued.clone());
    core.events.emit(Event::QueueChanged { queued });
}

/// Kills in the background: a kill waits for the process tree to exit.
pub(crate) fn kill_job(core: &Arc<SessionCore>, job_id: JobId) {
    let core = Arc::clone(core);
    tokio::spawn(async move {
        if let Err(error) = core.jobs.kill(&job_id).await {
            core.events.notice(
                NoticeLevel::Warn,
                format!("could not stop job {job_id}: {error}"),
            );
        }
    });
}
