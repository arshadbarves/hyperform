//! Seam: the user's message arrived and the run is about to start. Notes
//! from `on` uses become reminders in the turn's opening message.

use std::sync::Arc;

use z_engine_context::wrap_reminder;

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::run::RunContext;

pub(crate) async fn at_turn_start(ctx: &RunContext, text: &str) {
    at_turn_start_with(USES, ctx, text).await;
}

pub(super) async fn at_turn_start_with(
    uses: &[&'static dyn DecisionUse],
    ctx: &RunContext,
    text: &str,
) {
    let active = active(&ctx.core, uses, Seam::TurnStart);
    if active.is_empty() {
        return;
    }
    let text: Arc<str> = Arc::from(text);
    let notes = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let text = Arc::clone(&text);
        Box::pin(async move { decision_use.turn_start(&cx, &text).await })
    })
    .await;
    for note in notes.into_iter().flatten() {
        ctx.core
            .reminders
            .push(&ctx.spec.agent_id, wrap_reminder(&note));
    }
}
