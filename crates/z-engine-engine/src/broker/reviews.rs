//! Structured questions and plan reviews: persisted when asked and when
//! resolved (answered, dismissed, approved, revised, or withdrawn).

use tokio::sync::oneshot;
use z_engine_protocol::{
    AgentId, Event, PendingPlan, PendingQuestion, PlanDecision, Question, RequestId,
};
use z_engine_store::LogRecord;

use super::exchange::Broker;
use super::pending::Answers;
use crate::error::EngineError;
use crate::sync::lock;

impl Broker {
    pub(crate) fn ask(
        &self,
        agent_id: AgentId,
        questions: Vec<Question>,
    ) -> Result<(RequestId, oneshot::Receiver<Answers>), EngineError> {
        let request_id = RequestId::new();
        self.journal.append(&LogRecord::QuestionAsked {
            request_id: request_id.clone(),
            agent_id: agent_id.clone(),
            questions: questions.clone(),
        })?;
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = lock(&self.pending);
            let question = PendingQuestion {
                request_id: request_id.clone(),
                agent_id: agent_id.clone(),
                questions: questions.clone(),
            };
            pending.questions.push((question, tx));
            self.journal.events().emit(Event::QuestionAsked {
                request_id: request_id.clone(),
                agent_id,
                questions,
            });
        }
        self.refresh_status();
        Ok((request_id, rx))
    }

    /// `None` dismisses the questions.
    pub(crate) fn answer(&self, id: &RequestId, answers: Answers) -> Result<(), EngineError> {
        let (_, tx) = lock(&self.pending)
            .take_question(id)
            .ok_or_else(|| EngineError::NotFound(format!("question {id}")))?;
        self.finish_question(id, answers, Some(tx));
        Ok(())
    }

    pub(crate) fn withdraw_question(&self, id: &RequestId) {
        if lock(&self.pending).take_question(id).is_some() {
            self.finish_question(id, None, None);
        }
    }

    fn finish_question(
        &self,
        id: &RequestId,
        answers: Answers,
        reply: Option<oneshot::Sender<Answers>>,
    ) {
        let answered = answers.is_some();
        self.journal.append_or_report(&LogRecord::QuestionAnswered {
            request_id: id.clone(),
            answers: answers.clone(),
        });
        self.journal.events().emit(Event::QuestionResolved {
            request_id: id.clone(),
            answered,
        });
        if let Some(tx) = reply {
            if tx.send(answers).is_err() {
                tracing::debug!(request = %id, "question answered after its waiter stopped");
            }
        }
        self.refresh_status();
    }

    pub(crate) fn propose_plan(
        &self,
        agent_id: AgentId,
        plan: String,
    ) -> Result<(RequestId, oneshot::Receiver<PlanDecision>), EngineError> {
        let request_id = RequestId::new();
        self.journal.append(&LogRecord::PlanProposed {
            request_id: request_id.clone(),
            agent_id: agent_id.clone(),
            plan: plan.clone(),
        })?;
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = lock(&self.pending);
            let entry = PendingPlan {
                request_id: request_id.clone(),
                agent_id: agent_id.clone(),
                plan: plan.clone(),
            };
            pending.plans.push((entry, tx));
            self.journal.events().emit(Event::PlanProposed {
                request_id: request_id.clone(),
                agent_id,
                plan,
            });
        }
        self.refresh_status();
        Ok((request_id, rx))
    }

    /// `before_reply` applies an approval (the new mode) before the waiting
    /// agent resumes.
    pub(crate) fn resolve_plan(
        &self,
        id: &RequestId,
        decision: PlanDecision,
        before_reply: impl FnOnce(&PlanDecision),
    ) -> Result<(), EngineError> {
        let (plan, tx) = lock(&self.pending)
            .take_plan(id)
            .ok_or_else(|| EngineError::NotFound(format!("plan {id}")))?;
        let (approved, feedback, final_plan) = match &decision {
            PlanDecision::Approve { edited_plan, .. } => {
                let final_plan = edited_plan.clone().unwrap_or_else(|| plan.plan.clone());
                (true, None, Some(final_plan))
            }
            PlanDecision::Revise { feedback } => (false, Some(feedback.clone()), None),
        };
        self.journal.append_or_report(&LogRecord::PlanResolved {
            request_id: id.clone(),
            approved,
            feedback,
            final_plan,
        });
        self.journal.events().emit(Event::PlanResolved {
            request_id: id.clone(),
            approved,
        });
        before_reply(&decision);
        if tx.send(decision).is_err() {
            tracing::debug!(request = %id, "plan resolved after its waiter stopped");
        }
        self.refresh_status();
        Ok(())
    }

    pub(crate) fn withdraw_plan(&self, id: &RequestId) {
        if lock(&self.pending).take_plan(id).is_none() {
            return;
        }
        self.journal.append_or_report(&LogRecord::PlanResolved {
            request_id: id.clone(),
            approved: false,
            feedback: None,
            final_plan: None,
        });
        self.journal.events().emit(Event::PlanResolved {
            request_id: id.clone(),
            approved: false,
        });
        self.refresh_status();
    }
}
