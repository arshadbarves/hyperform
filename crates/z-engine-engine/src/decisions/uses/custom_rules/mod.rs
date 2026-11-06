//! Custom decision rules (`decisions_custom_rules`): the user's own
//! `[[decisions.rules]]` questions, asked at `PreToolUse` or
//! `UserPromptSubmit`. An answer listed in a rule's `when` asks the user
//! (an approval before an allowed call; for a message, the agent is told to
//! ask first), posts a notice, or reminds the agent. Nothing here can allow
//! a call, and a project's rules arrive only once the project is trusted.

mod judge;
mod prompt;
#[cfg(test)]
mod tests;
mod tool;

use async_trait::async_trait;
use z_engine_config::FeatureId;
use z_engine_protocol::{ToolResultPart, ToolStatus};

use crate::batch::ToolCall;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

#[derive(Debug)]
pub(crate) struct CustomRules;

pub(crate) static CUSTOM_RULES: CustomRules = CustomRules;

#[async_trait]
impl DecisionUse for CustomRules {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsCustomRules
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::TurnStart, Seam::ToolGate, Seam::AfterCall]
    }

    async fn turn_start(&self, cx: &UseContext, text: &str) -> Vec<String> {
        prompt::at_prompt(cx, text).await
    }

    async fn tool_gate(&self, cx: &UseContext, call: &ToolCall) -> Option<String> {
        tool::before_call(cx, call).await
    }

    async fn after_call(
        &self,
        cx: &UseContext,
        call: &ToolCall,
        _status: ToolStatus,
        _output: &[ToolResultPart],
    ) -> Vec<String> {
        tool::remind(cx, call).await
    }
}
