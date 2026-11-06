//! Seam: the main agent stopped and verification said done. The first
//! reminder from an `on` use sends the agent back to work; the stop
//! boundary caps how often per turn.

use std::path::PathBuf;
use std::sync::Arc;

use z_engine_context::wrap_reminder;

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::run::RunContext;

pub(crate) async fn review_stop(ctx: &RunContext, changed: &[PathBuf]) -> Option<String> {
    review_stop_with(USES, ctx, changed).await
}

pub(super) async fn review_stop_with(
    uses: &[&'static dyn DecisionUse],
    ctx: &RunContext,
    changed: &[PathBuf],
) -> Option<String> {
    let active = active(&ctx.core, uses, Seam::Stop);
    if active.is_empty() {
        return None;
    }
    let changed: Arc<[PathBuf]> = Arc::from(changed);
    let reminders = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let changed = Arc::clone(&changed);
        Box::pin(async move { decision_use.stop(&cx, &changed).await })
    })
    .await;
    let reminder = reminders.into_iter().flatten().next()?;
    Some(wrap_reminder(&reminder))
}
