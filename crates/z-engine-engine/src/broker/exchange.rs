//! The broker: approvals, questions and plan reviews waiting on the user.
//! Agents wait on oneshot replies, so the session actor keeps handling
//! steering, mode changes and cancellation meanwhile. The request kinds
//! live in `approvals.rs` and `reviews.rs`.

use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use z_engine_protocol::{ApprovalRequest, PendingPlan, PendingQuestion};

use super::pending::Pending;
use crate::session::{Journal, StatusTracker};
use crate::sync::lock;

#[derive(Debug)]
pub(crate) struct Broker {
    pub(super) journal: Arc<Journal>,
    status: Arc<StatusTracker>,
    pub(super) pending: Mutex<Pending>,
}

/// Everything pending, for snapshots.
type PendingView = (Vec<ApprovalRequest>, Vec<PendingQuestion>, Vec<PendingPlan>);

impl Broker {
    pub(crate) fn new(journal: Arc<Journal>, status: Arc<StatusTracker>) -> Self {
        Self {
            journal,
            status,
            pending: Mutex::new(Pending::default()),
        }
    }

    /// Waits for a reply; `None` when cancelled or the request was dropped.
    pub(crate) async fn wait<T>(rx: oneshot::Receiver<T>, cancel: &CancellationToken) -> Option<T> {
        tokio::select! {
            biased;
            reply = rx => reply.ok(),
            () = cancel.cancelled() => None,
        }
    }

    pub(crate) fn snapshot(&self) -> PendingView {
        let pending = lock(&self.pending);
        (
            pending.approvals.iter().map(|(r, _)| r.clone()).collect(),
            pending.questions.iter().map(|(q, _)| q.clone()).collect(),
            pending.plans.iter().map(|(p, _)| p.clone()).collect(),
        )
    }

    /// Withdraws requests whose waiter is gone (a tool dropped its wait on
    /// cancellation), so no card stays open without an agent behind it.
    pub(crate) fn withdraw_abandoned(&self) {
        let (approvals, questions, plans) = lock(&self.pending).abandoned();
        approvals.iter().for_each(|id| self.withdraw_approval(id));
        questions.iter().for_each(|id| self.withdraw_question(id));
        plans.iter().for_each(|id| self.withdraw_plan(id));
    }

    /// Resolves everything as denied or dismissed (session shutdown).
    pub(crate) fn withdraw_all(&self) {
        let (approvals, questions, plans) = self.snapshot();
        for request in approvals {
            self.withdraw_approval(&request.request_id);
        }
        for question in questions {
            self.withdraw_question(&question.request_id);
        }
        for plan in plans {
            self.withdraw_plan(&plan.request_id);
        }
    }

    /// Waiting while anything is pending.
    pub(super) fn refresh_status(&self) {
        let count = lock(&self.pending).count();
        self.status.set_waiting(count);
    }
}
