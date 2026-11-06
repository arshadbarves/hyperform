//! `WorkPort`: the work tools against the session's attached plan. The
//! first task (or a structured plan) starts a plan; every change goes to
//! the kernel as the calling agent, and a refusal comes back to the model
//! with each reason.

use std::sync::Arc;

use async_trait::async_trait;
use hyperform_context::{estimate_text, packet_text, task_line};
use hyperform_protocol::TaskId;
use hyperform_protocol::work::{
    EntryStatus, PlanItemStatus, Provenance, TaskScope, TaskStatus, WorkCause,
};
use hyperform_tools::{
    NewTask, PlanNote, PlanOutline, TaskChange, ToolCtx, WorkPort, WorkReply, names,
};
use hyperform_work::{Facts, Rejection, Submission, graph, select};

use crate::run::WebTaint;
use crate::session::SessionCore;
use crate::work::{
    Ids, PlanHandle, Submitted, after_records, agent, baseline, checks_of, current, ensure,
    instant, list_text, proposals, save_report, task_id,
};

#[derive(Debug)]
pub(crate) struct Work {
    core: Arc<SessionCore>,
    /// Set once the calling run read web content: its proposals are
    /// untrusted input.
    web: Arc<WebTaint>,
}

impl Work {
    pub(crate) fn new(core: Arc<SessionCore>, web: Arc<WebTaint>) -> Self {
        Self { core, web }
    }

    fn untrusted(&self, mut facts: Facts) -> Facts {
        facts.untrusted_input |= self.web.is_set();
        facts
    }

    fn plan(&self) -> Result<Arc<PlanHandle>, String> {
        self.core.work.plan().ok_or_else(|| {
            "no work plan is attached to this chat; TaskCreate starts one".to_string()
        })
    }

    fn line(plan: &PlanHandle, id: &TaskId) -> String {
        plan.read(|state| state.task(id).map(task_line))
            .unwrap_or_else(|| id.to_string())
    }
}

const WAITS_FOR_USER: &str = "\nThis turn read web content, so the task is proposed: it waits for the user to accept it in the Plan tab before anyone starts it.";
const REPORT_SAVED: &str = "\nThe report is recorded when your run ends, with the files your change touched and the checks you ran as the engine observed them. A later report replaces it.";

fn cause(ctx: &ToolCtx, tool: &str) -> WorkCause {
    WorkCause::Tool {
        tool: tool.to_string(),
        call_id: Some(ctx.call_id.clone()),
    }
}

fn refused(rejection: &Rejection) -> String {
    let reasons: Vec<String> = rejection
        .messages()
        .iter()
        .map(|reason| format!("- {reason}"))
        .collect();
    format!("The plan refused this change:\n{}", reasons.join("\n"))
}

#[async_trait]
impl WorkPort for Work {
    async fn create_task(&self, ctx: &ToolCtx, task: NewTask) -> Result<WorkReply, String> {
        let core = &self.core;
        let actor = agent(core, &ctx.agent_id);
        let cause = cause(ctx, names::TASK_CREATE);
        let scope = TaskScope {
            files: task.scope.clone(),
            symbols: Vec::new(),
        };
        let (plan, fresh) = ensure(core, &actor, &cause, &task.subject, &scope)?;
        if fresh {
            baseline(core).await;
        }
        let facts = self.untrusted(current(&self.core, &ctx.root).await);
        let mut created = None;
        let result = plan.submit(&facts, |state| {
            let (id, proposal) = proposals::new_task(&mut Ids::new(state), &task)?;
            created = Some(id);
            Ok(Submission::new(actor, cause, vec![proposal]))
        })?;
        let (Submitted::Accepted(_), Some(id)) = (&result, created) else {
            return match result {
                Submitted::Rejected(rejection) => Err(refused(&rejection)),
                _ => Err("the task was not created".to_string()),
            };
        };
        let mut text = format!("Created task {}.", Self::line(&plan, &id));
        let waits = plan.read(|state| {
            state.task(&id).is_some_and(|task| {
                task.provenance == Provenance::Research && task.status == TaskStatus::Proposed
            })
        });
        if waits {
            text.push_str(WAITS_FOR_USER);
        }
        if fresh {
            let (goal, status) = plan.read(|s| (s.header.goal.clone(), s.header.status));
            text.push_str(&format!(
                "\nThis started the work plan \"{goal}\" ({}); later tasks join it.",
                status.label()
            ));
        }
        Ok(WorkReply::new(text, format!("Created {id}")))
    }

    async fn update_task(&self, ctx: &ToolCtx, change: TaskChange) -> Result<WorkReply, String> {
        let core = &self.core;
        let plan = self.plan()?;
        let actor = agent(core, &ctx.agent_id);
        let mut facts = self.untrusted(current(&self.core, &ctx.root).await);
        facts.run.checks = checks_of(core, &ctx.agent_id);
        let max_attempts = core.settings().settings.work.max_attempts;
        let id = task_id(&change.task_id);
        let mut change = change;
        let completes = change.status.as_deref().map(str::trim) == Some("completed");
        let saved = match &change.report {
            Some(draft) if !completes => save_report(core, &ctx.agent_id, &id, draft)
                .transpose()?
                .is_some(),
            _ => false,
        };
        if saved {
            change.report = None;
        }
        if completes && change.report.is_none() {
            change.report = core.work.saved_report(&ctx.agent_id, &id);
        }
        let result = plan.submit(&facts, |state| {
            let proposals = proposals::task_change(state, &change, max_attempts)?;
            Ok(Submission::new(
                actor,
                cause(ctx, names::TASK_UPDATE),
                proposals,
            ))
        })?;
        let note = if saved { REPORT_SAVED } else { "" };
        match result {
            Submitted::Accepted(_) => {
                let mut text = format!("Updated task {}.{note}", Self::line(&plan, &id));
                if completes {
                    let next = plan.read(|state| graph::next_ready(state, None).map(task_line));
                    if let Some(next) = next {
                        text.push_str(&format!("\nNext ready task: {next}"));
                    }
                }
                Ok(WorkReply::new(text, format!("Updated {id}")))
            }
            Submitted::Unchanged if saved => Ok(WorkReply::new(
                format!("Saved your report on task {id}.{note}"),
                format!("Reported {id}"),
            )),
            Submitted::Unchanged => Ok(WorkReply::new(
                format!("Task {id} already matches; nothing changed."),
                format!("Updated {id}"),
            )),
            Submitted::Rejected(rejection) => Err(refused(&rejection)),
        }
    }

    async fn get_task(&self, ctx: &ToolCtx, task: &str) -> Result<WorkReply, String> {
        let plan = self.plan()?;
        let facts = current(&self.core, &ctx.root).await;
        let budget = u64::from(self.core.settings().settings.work.packet_tokens);
        let id = task_id(task);
        let packet = plan
            .read(|state| select(state, &id, budget, &facts, &estimate_text))
            .map_err(|error| error.to_string())?;
        if ctx.agent_id.is_main() {
            self.core.work.mark_sent(&id, packet.revision);
        }
        Ok(WorkReply::new(packet_text(&packet), format!("Task {id}")))
    }

    async fn list_tasks(&self, _ctx: &ToolCtx) -> Result<WorkReply, String> {
        let plan = self.plan()?;
        let (text, count) = plan.read(|state| (list_text(state), state.tasks.len()));
        Ok(WorkReply::new(text, format!("{count} tasks")))
    }

    async fn record(&self, ctx: &ToolCtx, note: PlanNote) -> Result<WorkReply, String> {
        let core = &self.core;
        let plan = self.plan()?;
        let actor = agent(core, &ctx.agent_id);
        let mut facts = self.untrusted(current(&self.core, &ctx.root).await);
        facts.untrusted_input |= note
            .source
            .as_deref()
            .is_some_and(|source| source.starts_with("http://") || source.starts_with("https://"));
        let mut recorded = None;
        let result = plan.submit(&facts, |state| {
            let (proposals, id) = proposals::note(&mut Ids::new(state), &note)?;
            recorded = id;
            Ok(Submission::new(
                actor,
                cause(ctx, names::PLAN_RECORD),
                proposals,
            ))
        })?;
        let records = match result {
            Submitted::Accepted(records) => records,
            Submitted::Unchanged => return Ok(WorkReply::new("Nothing changed.", "Recorded")),
            Submitted::Rejected(rejection) => return Err(refused(&rejection)),
        };
        let kind = note.kind.trim();
        let text = match &recorded {
            Some(id) => {
                let pending = plan.read(|state| {
                    let entry = state.entry(id).map(|e| e.status == EntryStatus::Proposed);
                    entry.or_else(|| {
                        state
                            .item(id)
                            .map(|item| item.status == PlanItemStatus::Proposed)
                    })
                });
                let note = if pending == Some(true) {
                    "; it is unconfirmed until the user accepts it"
                } else {
                    ""
                };
                format!("Recorded {kind} {id}{note}.")
            }
            None => format!("Recorded the {kind}."),
        };
        after_records(core, &plan, &records).await;
        Ok(WorkReply::new(text, format!("Recorded {kind}")))
    }

    async fn outline_plan(&self, ctx: &ToolCtx, outline: PlanOutline) -> Result<(), String> {
        let core = &self.core;
        let actor = agent(core, &ctx.agent_id);
        let cause = cause(ctx, names::EXIT_PLAN_MODE);
        let (plan, fresh) = ensure(core, &actor, &cause, &outline.goal, &TaskScope::default())?;
        if fresh {
            baseline(core).await;
        }
        let result = plan.submit(&self.untrusted(instant(&self.core)), |state| {
            let proposals = proposals::outline(&mut Ids::new(state), &outline);
            Ok(Submission::new(actor, cause, proposals))
        })?;
        match result {
            Submitted::Rejected(rejection) => Err(refused(&rejection)),
            _ => Ok(()),
        }
    }
}
