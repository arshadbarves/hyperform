//! Risk review (`decisions_risk`): whether an allowed call should be shown
//! to the user first (the gate half), and whether a web or MCP result tries
//! to instruct the agent (the injection half). It can only add an approval
//! or a warning note; it never allows, denies or removes anything.

mod gate;
mod injection;
#[cfg(test)]
mod tests;

use async_trait::async_trait;
use z_engine_config::FeatureId;
use z_engine_protocol::{ToolResultPart, ToolStatus};

use crate::batch::ToolCall;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

#[derive(Debug)]
pub(crate) struct Risk;

pub(crate) static RISK: Risk = Risk;

#[async_trait]
impl DecisionUse for Risk {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsRisk
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::ToolGate, Seam::AfterCall]
    }

    async fn tool_gate(&self, cx: &UseContext, call: &ToolCall) -> Option<String> {
        gate::review(cx, call).await
    }

    async fn after_call(
        &self,
        cx: &UseContext,
        call: &ToolCall,
        _status: ToolStatus,
        output: &[ToolResultPart],
    ) -> Vec<String> {
        injection::screen(cx, call, output).await
    }
}
