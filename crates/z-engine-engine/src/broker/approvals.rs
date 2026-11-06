//! Approval requests: announced together, answered in any order, audited
//! as `LogRecord::Approval` when the user decides.

use tokio::sync::oneshot;
use z_engine_protocol::{ApprovalDecision, ApprovalRequest, Event, RequestId};
use z_engine_store::LogRecord;

use super::exchange::Broker;
use crate::error::EngineError;
use crate::sync::lock;

impl Broker {
    /// Announces every request at once; the user may answer in any order.
    pub(crate) fn request_approvals(
        &self,
        requests: Vec<ApprovalRequest>,
    ) -> Vec<oneshot::Receiver<ApprovalDecision>> {
        let receivers = {
            let mut pending = lock(&self.pending);
            requests
                .into_iter()
                .map(|request| {
                    let (tx, rx) = oneshot::channel();
                    let event = Event::ApprovalRequested {
                        request: request.clone(),
                    };
                    pending.approvals.push((request, tx));
                    self.journal.events().emit(event);
                    rx
                })
                .collect()
        };
        self.refresh_status();
        receivers
    }

    /// Applies the user's decision. `before_reply` runs once the request is
    /// taken and before the waiting agent resumes, so granted rules are in
    /// place for its next call. Unknown ids are `NotFound`.
    pub(crate) fn resolve_approval(
        &self,
        id: &RequestId,
        decision: ApprovalDecision,
        before_reply: impl FnOnce(&ApprovalRequest, &ApprovalDecision),
    ) -> Result<(), EngineError> {
        let (request, tx) = lock(&self.pending)
            .take_approval(id)
            .ok_or_else(|| EngineError::NotFound(format!("approval request {id}")))?;
        before_reply(&request, &decision);
        let allowed = decision.allows();
        let rule = match &decision {
            ApprovalDecision::AllowSession { rule } | ApprovalDecision::AllowProject { rule } => {
                Some(rule.clone())
            }
            ApprovalDecision::AllowOnce | ApprovalDecision::Deny { .. } => None,
        };
        self.journal.append_or_report(&LogRecord::Approval {
            request_id: id.clone(),
            tool: request.tool.clone(),
            title: request.title.clone(),
            allowed,
            rule,
        });
        self.journal.events().emit(Event::ApprovalResolved {
            request_id: id.clone(),
            allowed,
        });
        if tx.send(decision).is_err() {
            tracing::debug!(request = %id, "approval answered after its waiter stopped");
        }
        self.refresh_status();
        Ok(())
    }

    /// Removes a request whose waiter gave up; it resolves as not allowed.
    pub(crate) fn withdraw_approval(&self, id: &RequestId) {
        if lock(&self.pending).take_approval(id).is_none() {
            return;
        }
        self.journal.events().emit(Event::ApprovalResolved {
            request_id: id.clone(),
            allowed: false,
        });
        self.refresh_status();
    }
}
