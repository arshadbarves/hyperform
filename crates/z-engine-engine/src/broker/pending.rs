//! The pending interaction tables. Each entry owns the reply channel of the
//! agent waiting on it; removing an entry without replying resolves the
//! waiter as denied or dismissed.

use tokio::sync::oneshot;
use z_engine_protocol::{
    ApprovalDecision, ApprovalRequest, PendingPlan, PendingQuestion, PlanDecision, QuestionAnswer,
    RequestId,
};

pub(super) type Answers = Option<Vec<QuestionAnswer>>;

#[derive(Debug, Default)]
pub(super) struct Pending {
    pub approvals: Vec<(ApprovalRequest, oneshot::Sender<ApprovalDecision>)>,
    pub questions: Vec<(PendingQuestion, oneshot::Sender<Answers>)>,
    pub plans: Vec<(PendingPlan, oneshot::Sender<PlanDecision>)>,
}

impl Pending {
    pub fn count(&self) -> usize {
        self.approvals.len() + self.questions.len() + self.plans.len()
    }

    pub fn take_approval(
        &mut self,
        id: &RequestId,
    ) -> Option<(ApprovalRequest, oneshot::Sender<ApprovalDecision>)> {
        take(&mut self.approvals, |(request, _)| {
            request.request_id == *id
        })
    }

    pub fn take_question(
        &mut self,
        id: &RequestId,
    ) -> Option<(PendingQuestion, oneshot::Sender<Answers>)> {
        take(&mut self.questions, |(question, _)| {
            question.request_id == *id
        })
    }

    pub fn take_plan(
        &mut self,
        id: &RequestId,
    ) -> Option<(PendingPlan, oneshot::Sender<PlanDecision>)> {
        take(&mut self.plans, |(plan, _)| plan.request_id == *id)
    }

    /// Ids of approvals, questions and plans whose waiter dropped its reply.
    pub fn abandoned(&self) -> (Vec<RequestId>, Vec<RequestId>, Vec<RequestId>) {
        (
            self.approvals
                .iter()
                .filter(|(_, tx)| tx.is_closed())
                .map(|(request, _)| request.request_id.clone())
                .collect(),
            self.questions
                .iter()
                .filter(|(_, tx)| tx.is_closed())
                .map(|(question, _)| question.request_id.clone())
                .collect(),
            self.plans
                .iter()
                .filter(|(_, tx)| tx.is_closed())
                .map(|(plan, _)| plan.request_id.clone())
                .collect(),
        )
    }
}

fn take<T>(list: &mut Vec<T>, matches: impl Fn(&T) -> bool) -> Option<T> {
    let index = list.iter().position(matches)?;
    Some(list.remove(index))
}
