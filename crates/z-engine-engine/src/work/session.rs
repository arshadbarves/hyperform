//! `WorkSession`: the plan attached to one session, what its main agent
//! was last given about it, what resuming it found, the reports its
//! helpers saved, and its task-bound helpers that still run.

use std::sync::{Arc, Mutex};

use z_engine_protocol::work::ReportDraft;
use z_engine_protocol::{AgentId, TaskId};

use super::bound::BoundHelpers;
use super::plan::PlanHandle;
use crate::sync::lock;

#[derive(Debug, Default)]
pub(crate) struct WorkSession {
    inner: Mutex<Attachment>,
    helpers: BoundHelpers,
}

#[derive(Debug, Default)]
struct Attachment {
    plan: Option<Arc<PlanHandle>>,
    /// The task and plan revision of the last full packet.
    sent: Option<(TaskId, u64)>,
    /// Why the next full packet is due regardless of the task.
    due: Option<&'static str>,
    /// The workspace fingerprint seen at the last turn start.
    fingerprint: Option<String>,
    /// Reports helpers sent with `TaskUpdate`, recorded when they end.
    reports: Vec<(AgentId, TaskId, ReportDraft)>,
    /// What resuming the plan found, for the next turn's resume reminder.
    resumed: Option<String>,
}

impl WorkSession {
    pub(crate) fn plan(&self) -> Option<Arc<PlanHandle>> {
        lock(&self.inner).plan.clone()
    }

    pub(crate) fn helpers(&self) -> &BoundHelpers {
        &self.helpers
    }

    /// The next turn starts with the resume reminder; `summary` is what
    /// resuming found.
    pub(crate) fn resume_due(&self, summary: String) {
        lock(&self.inner).resumed = Some(summary);
    }

    pub(crate) fn take_resumed(&self) -> Option<String> {
        lock(&self.inner).resumed.take()
    }

    /// Attaches `plan` (or detaches); returns the plan it replaces. A
    /// `resumed` plan owes the agent a full packet.
    pub(crate) fn replace(
        &self,
        plan: Option<Arc<PlanHandle>>,
        resumed: bool,
    ) -> Option<Arc<PlanHandle>> {
        let mut inner = lock(&self.inner);
        let due = (plan.is_some() && resumed).then_some("resume");
        let previous = std::mem::replace(
            &mut *inner,
            Attachment {
                plan,
                due,
                ..Attachment::default()
            },
        );
        previous.plan
    }

    /// The next packet is due whatever was sent before.
    pub(crate) fn packet_due(&self, reason: &'static str) {
        let mut inner = lock(&self.inner);
        inner.sent = None;
        inner.due = Some(reason);
    }

    /// Why a full packet for `task` at `revision` is due now, if it is; the
    /// packet then counts as sent. The same task at the same revision is
    /// never sent twice, and a new revision alone does not resend it.
    pub(crate) fn take_due(&self, task: &TaskId, revision: u64) -> Option<&'static str> {
        let mut inner = lock(&self.inner);
        let same = |sent: &(TaskId, u64)| sent.0 == *task;
        let reason = match (&inner.sent, inner.due) {
            (Some(sent), _) if same(sent) && sent.1 == revision => None,
            (_, Some(reason)) => Some(reason),
            (None, None) => Some("turn"),
            (Some(sent), None) if !same(sent) => Some("taskChanged"),
            (Some(_), None) => None,
        };
        if reason.is_some() || inner.due.is_some() {
            inner.due = None;
        }
        if reason.is_some() {
            inner.sent = Some((task.clone(), revision));
        }
        reason
    }

    /// The agent fetched `task` at `revision` itself.
    pub(crate) fn mark_sent(&self, task: &TaskId, revision: u64) {
        lock(&self.inner).sent = Some((task.clone(), revision));
    }

    /// Stores the fingerprint of this turn start; returns the previous one.
    pub(crate) fn swap_fingerprint(&self, fingerprint: Option<String>) -> Option<String> {
        std::mem::replace(&mut lock(&self.inner).fingerprint, fingerprint)
    }

    /// Keeps `helper`'s latest report on `task` until its run ends.
    pub(crate) fn save_report(&self, helper: &AgentId, task: &TaskId, draft: ReportDraft) {
        let mut inner = lock(&self.inner);
        inner.reports.retain(|(agent, ..)| agent != helper);
        inner.reports.push((helper.clone(), task.clone(), draft));
    }

    /// The report `helper` saved on `task`, removed.
    pub(crate) fn take_report(&self, helper: &AgentId, task: &TaskId) -> Option<ReportDraft> {
        let mut inner = lock(&self.inner);
        let at = inner
            .reports
            .iter()
            .position(|(agent, saved, _)| agent == helper && saved == task)?;
        Some(inner.reports.remove(at).2)
    }

    /// The report `helper` saved on `task`, kept.
    pub(crate) fn saved_report(&self, helper: &AgentId, task: &TaskId) -> Option<ReportDraft> {
        let inner = lock(&self.inner);
        inner
            .reports
            .iter()
            .find(|(agent, saved, _)| agent == helper && saved == task)
            .map(|(.., draft)| draft.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_are_due_on_new_tasks_and_triggers_only() {
        let work = WorkSession::default();
        let (t1, t2) = (TaskId("t1".into()), TaskId("t2".into()));
        assert_eq!(work.take_due(&t1, 3), Some("turn"));
        assert_eq!(work.take_due(&t1, 3), None);
        assert_eq!(
            work.take_due(&t1, 4),
            None,
            "a revision alone does not resend"
        );
        assert_eq!(work.take_due(&t2, 4), Some("taskChanged"));
        work.packet_due("compaction");
        assert_eq!(work.take_due(&t2, 4), Some("compaction"));
        work.packet_due("resume");
        work.mark_sent(&t2, 5);
        assert_eq!(work.take_due(&t2, 5), None, "already sent at this revision");
        assert_eq!(work.take_due(&t2, 6), None, "the trigger was used up");
    }

    #[test]
    fn a_helper_keeps_only_its_latest_report_per_task() {
        let work = WorkSession::default();
        let (helper, task) = (AgentId::from("agt_1"), TaskId("t1".into()));
        let draft = |summary: &str| ReportDraft {
            summary: summary.into(),
            ..ReportDraft::default()
        };
        work.save_report(&helper, &task, draft("first"));
        work.save_report(&helper, &task, draft("second"));
        assert_eq!(work.take_report(&helper, &TaskId("t2".into())), None);
        let taken = work.take_report(&helper, &task).map(|draft| draft.summary);
        assert_eq!(taken.as_deref(), Some("second"));
        assert_eq!(work.take_report(&helper, &task), None);
    }
}
