//! The user-facing broker: structured questions, plan review, todos.

use async_trait::async_trait;
use z_engine_protocol::{PlanDecision, Question, QuestionAnswer, TodoItem};

use crate::context::ToolCtx;

#[async_trait]
pub trait InteractionPort: Send + Sync {
    /// False where nobody can answer (subagents).
    fn can_ask(&self, ctx: &ToolCtx) -> bool;

    /// A note for the model instead of asking, when the chat already
    /// answered `questions`; `None` asks the user as usual.
    async fn already_answered(&self, _ctx: &ToolCtx, _questions: &[Question]) -> Option<String> {
        None
    }

    /// `Ok(None)` when the user dismissed the questions.
    async fn ask(
        &self,
        ctx: &ToolCtx,
        questions: Vec<Question>,
    ) -> Result<Option<Vec<QuestionAnswer>>, String>;

    async fn propose_plan(&self, ctx: &ToolCtx, plan: String) -> Result<PlanDecision, String>;

    /// Replaces the agent's todo list.
    fn update_todos(&self, ctx: &ToolCtx, todos: Vec<TodoItem>);
}
