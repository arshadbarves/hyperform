//! Seam: a tool call finished (not cancelled) and its `PostToolUse` hooks
//! ran. Notes from `on` uses are prepended to the result as reminders.

use std::sync::Arc;

use z_engine_context::wrap_reminder;
use z_engine_protocol::{ToolResultPart, ToolStatus};

use super::dispatch::{active, dispatch};
use crate::batch::ToolCall;
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::run::RunContext;

pub(crate) async fn annotate_result(
    ctx: &RunContext,
    call: &ToolCall,
    status: ToolStatus,
    content: &mut Vec<ToolResultPart>,
) {
    annotate_result_with(USES, ctx, call, status, content).await;
}

pub(super) async fn annotate_result_with(
    uses: &[&'static dyn DecisionUse],
    ctx: &RunContext,
    call: &ToolCall,
    status: ToolStatus,
    content: &mut Vec<ToolResultPart>,
) {
    let active = active(&ctx.core, uses, Seam::AfterCall);
    if active.is_empty() {
        return;
    }
    let state = Arc::new((call.clone(), content.clone()));
    let agent = ctx.spec.agent_id.clone();
    let notes = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let state = Arc::clone(&state);
        let cx = cx.for_agent(&agent);
        Box::pin(async move {
            decision_use
                .after_call(&cx, &state.0, status, &state.1)
                .await
        })
    })
    .await;
    let notes = notes
        .into_iter()
        .flatten()
        .map(|note| ToolResultPart::Text {
            text: wrap_reminder(&note),
        });
    content.splice(0..0, notes);
}
