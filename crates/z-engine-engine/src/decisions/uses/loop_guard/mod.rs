//! Loop guard (`decisions_loop_guard`): notices when an agent repeats
//! itself without progress (the same call and result again and again, the
//! same failure after an edit, or a run the decision model judges stuck),
//! reminds it with text that fits the failure, and after the configured
//! reminders in a turn asks the user before its next call. Recovery is
//! only reminders, notices and that ask.

mod detect;
mod guard;
mod steps;
#[cfg(test)]
mod tests;
mod watch;

use async_trait::async_trait;
use z_engine_config::FeatureId;
use z_engine_protocol::{ToolResultPart, ToolStatus};

pub(crate) use watch::LoopWatch;

use crate::batch::ToolCall;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

#[derive(Debug)]
pub(crate) struct LoopGuard;

pub(crate) static LOOP_GUARD: LoopGuard = LoopGuard;

#[async_trait]
impl DecisionUse for LoopGuard {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsLoopGuard
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::ToolGate, Seam::AfterCall]
    }

    async fn tool_gate(&self, cx: &UseContext, call: &ToolCall) -> Option<String> {
        guard::gate(cx, call)
    }

    async fn after_call(
        &self,
        cx: &UseContext,
        call: &ToolCall,
        status: ToolStatus,
        output: &[ToolResultPart],
    ) -> Vec<String> {
        guard::after_call(cx, call, status, output).await
    }
}
