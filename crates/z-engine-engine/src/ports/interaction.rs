//! `InteractionPort`: questions and plan reviews through the broker (main
//! agent only; a deep work plan is audited while the user reviews it), and
//! todo lists persisted and announced per agent (and kept as the steps of
//! the work task the agent owns).

use std::sync::Arc;

use async_trait::async_trait;
use z_engine_protocol::work::WorkCause;
use z_engine_protocol::{Event, PlanDecision, Question, QuestionAnswer, TodoItem};
use z_engine_store::LogRecord;
use z_engine_tools::{InteractionPort, ToolCtx, names};

use crate::broker::Broker;
use crate::decisions::seams::{AttentionItem, AttentionKind, review_questions};
use crate::hooks::notify;
use crate::run::WebTaint;
use crate::session::SessionCore;
use crate::work::{record_steps, review_deep_plan};

/// Characters of a proposed plan an urgency score reads.
const PLAN_CHARS: usize = 600;

#[derive(Debug)]
pub(crate) struct Interaction {
    core: Arc<SessionCore>,
    web: Arc<WebTaint>,
}

impl Interaction {
    pub(crate) fn new(core: Arc<SessionCore>, web: Arc<WebTaint>) -> Self {
        Self { core, web }
    }
}

#[async_trait]
impl InteractionPort for Interaction {
    fn can_ask(&self, ctx: &ToolCtx) -> bool {
        ctx.agent_id.is_main()
    }

    async fn already_answered(&self, ctx: &ToolCtx, questions: &[Question]) -> Option<String> {
        review_questions(&self.core, &ctx.cancel, questions).await
    }

    async fn ask(
        &self,
        ctx: &ToolCtx,
        questions: Vec<Question>,
    ) -> Result<Option<Vec<QuestionAnswer>>, String> {
        if !self.can_ask(ctx) {
            return Err("only the main agent can ask the user".to_string());
        }
        let broker = &self.core.broker;
        let asked = questions
            .iter()
            .map(|q| q.question.as_str())
            .collect::<Vec<_>>();
        let text = asked.join("\n");
        let (request_id, reply) = broker
            .ask(ctx.agent_id.clone(), questions)
            .map_err(|error| error.to_string())?;
        let item = AttentionItem::request(AttentionKind::Question, request_id.as_str(), text);
        let message = "Z Engine has a question for you";
        notify(&self.core, message, vec![item], &ctx.cancel).await;
        match Broker::wait(reply, &ctx.cancel).await {
            Some(answers) => Ok(answers),
            None => {
                broker.withdraw_question(&request_id);
                Err("cancelled by user".to_string())
            }
        }
    }

    async fn propose_plan(&self, ctx: &ToolCtx, plan: String) -> Result<PlanDecision, String> {
        if !self.can_ask(ctx) {
            return Err("only the main agent can propose a plan".to_string());
        }
        review_deep_plan(&self.core);
        let broker = &self.core.broker;
        let text = plan.chars().take(PLAN_CHARS).collect();
        let (request_id, reply) = broker
            .propose_plan(ctx.agent_id.clone(), plan)
            .map_err(|error| error.to_string())?;
        let item = AttentionItem::request(AttentionKind::Plan, request_id.as_str(), text);
        let message = "Z Engine has a plan for you to review";
        notify(&self.core, message, vec![item], &ctx.cancel).await;
        match Broker::wait(reply, &ctx.cancel).await {
            Some(decision) => Ok(decision),
            None => {
                broker.withdraw_plan(&request_id);
                Err("cancelled by user".to_string())
            }
        }
    }

    fn update_todos(&self, ctx: &ToolCtx, todos: Vec<TodoItem>) {
        let agent_id = ctx.agent_id.clone();
        let persisted = self.core.journal.append_or_report(&LogRecord::Todos {
            agent_id: agent_id.clone(),
            todos: todos.clone(),
        });
        if !persisted {
            tracing::warn!(agent = %agent_id, count = todos.len(), "todo list kept in memory only");
        }
        self.core
            .with_state(|state| state.todos.insert(agent_id.clone(), todos.clone()));
        let cause = WorkCause::Tool {
            tool: names::TODO_WRITE.to_string(),
            call_id: Some(ctx.call_id.clone()),
        };
        record_steps(&self.core, &agent_id, &todos, cause, self.web.is_set());
        self.core
            .events
            .emit(Event::TodosUpdated { agent_id, todos });
    }
}
