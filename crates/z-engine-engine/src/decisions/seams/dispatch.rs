//! Running the uses of one seam. With every feature off nothing is built
//! or spawned. Shadow uses run in background tasks (a few at a time, extra
//! ones skipped) and their advice is dropped, so they never add latency.
//! `on` uses run concurrently against one deadline; one that times out or
//! panics gives no advice and leaves a trace record saying so.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use z_engine_config::FeatureMode;
use z_engine_decisions::{AbstainReason, Answer};

use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::session::SessionCore;

pub(super) type UseFuture<O> = Pin<Box<dyn Future<Output = O> + Send>>;

/// Uses of `seam` whose feature runs, with their mode.
pub(super) fn active(
    core: &SessionCore,
    uses: &[&'static dyn DecisionUse],
    seam: Seam,
) -> Vec<(&'static dyn DecisionUse, FeatureMode)> {
    let service = core.decisions.service();
    uses.iter()
        .filter(|u| u.seams().contains(&seam))
        .map(|u| (*u, service.mode(u.feature())))
        .filter(|(_, mode)| mode.runs())
        .collect()
}

/// The advice of every `on` use that answered in time, in `active` order.
pub(super) async fn dispatch<O: Send + 'static>(
    core: &Arc<SessionCore>,
    active: Vec<(&'static dyn DecisionUse, FeatureMode)>,
    cancel: &CancellationToken,
    call: impl Fn(&'static dyn DecisionUse, UseContext) -> UseFuture<O>,
) -> Vec<O> {
    let service = core.decisions.service();
    let deadline = Instant::now() + budget(service.timeout());
    let mut acting = Vec::new();
    for (decision_use, mode) in active {
        let shadow = mode != FeatureMode::On;
        let cx = UseContext {
            core: Arc::clone(core),
            service: Arc::clone(&service),
            feature: decision_use.feature(),
            shadow,
            cancel: if shadow {
                core.cancel.child_token()
            } else {
                cancel.child_token()
            },
            agent: None,
        };
        if shadow {
            let Some(slot) = core.decisions.shadow_slot() else {
                tracing::debug!(feature = %cx.feature, "shadow decision skipped: all slots busy");
                continue;
            };
            let work = call(decision_use, cx);
            tokio::spawn(async move {
                work.await;
                drop(slot);
            });
        } else {
            let handle = tokio::spawn(call(decision_use, cx.clone()));
            acting.push((cx, handle));
        }
    }
    let mut advice = Vec::with_capacity(acting.len());
    for (cx, mut handle) in acting {
        match tokio::time::timeout_at(deadline, &mut handle).await {
            Ok(Ok(output)) => advice.push(output),
            Ok(Err(error)) => {
                tracing::warn!(feature = %cx.feature, %error, "decision use failed");
                record_failure(&cx, AbstainReason::Invalid);
            }
            Err(_) => {
                handle.abort();
                cx.cancel.cancel();
                record_failure(&cx, AbstainReason::Timeout);
            }
        }
    }
    advice
}

/// Room for a use to ask twice, plus scheduling slack.
fn budget(timeout: Duration) -> Duration {
    timeout.saturating_mul(2) + Duration::from_millis(50)
}

fn record_failure(cx: &UseContext, reason: AbstainReason) {
    let answer = Answer::abstained("(use)", reason, cx.service.provider_name());
    cx.record(cx.record_of(&answer, ""));
}
