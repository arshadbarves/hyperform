//! `Notification` hooks: the session is waiting on the user. With
//! `decisions_inbox_priority` on, the waiting items are scored first and
//! the hooks fire only when one is high urgency (or was not scored).

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::event::{HookEvent, HookInput};
use super::runner::run_hooks;
use crate::decisions::seams::{AttentionItem, score_attention, worth_notifying};
use crate::session::SessionCore;

pub(crate) async fn notify(
    core: &Arc<SessionCore>,
    message: &str,
    items: Vec<AttentionItem>,
    cancel: &CancellationToken,
) {
    let urgencies = score_attention(core, items, cancel).await;
    if !worth_notifying(&urgencies) {
        tracing::debug!(message, "notification held back: nothing urgent");
        return;
    }
    let input = HookInput::new().with("message", json!(message));
    run_hooks(
        &core.hook_env(),
        &core.events,
        HookEvent::Notification,
        input,
        cancel,
    )
    .await;
}
