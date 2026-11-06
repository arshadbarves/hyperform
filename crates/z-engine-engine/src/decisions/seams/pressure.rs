//! Seam: context pressure is about to clear old tool results. `on` uses
//! may keep planned clears or add more; keeping wins over clearing.

use std::sync::Arc;

use z_engine_context::ClearTarget;
use z_engine_protocol::{CallId, Message};

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::run::RunContext;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ClearAdvice {
    /// Planned clears to skip: these results are still needed.
    pub keep: Vec<CallId>,
    /// Results to clear now as well.
    pub clear: Vec<ClearTarget>,
}

/// The clears to apply, oldest first.
pub(crate) async fn review_clears(
    ctx: &RunContext,
    working: &[Message],
    planned: Vec<ClearTarget>,
) -> Vec<ClearTarget> {
    review_clears_with(USES, ctx, working, planned).await
}

pub(super) async fn review_clears_with(
    uses: &[&'static dyn DecisionUse],
    ctx: &RunContext,
    working: &[Message],
    planned: Vec<ClearTarget>,
) -> Vec<ClearTarget> {
    let active = active(&ctx.core, uses, Seam::Pressure);
    if active.is_empty() {
        return planned;
    }
    let state = Arc::new((working.to_vec(), planned.clone()));
    let advice = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let state = Arc::clone(&state);
        Box::pin(async move { decision_use.pressure(&cx, &state.0, &state.1).await })
    })
    .await;
    apply(planned, advice)
}

fn apply(mut targets: Vec<ClearTarget>, advice: Vec<ClearAdvice>) -> Vec<ClearTarget> {
    let mut keep = Vec::new();
    for ClearAdvice { keep: kept, clear } in advice {
        keep.extend(kept);
        for target in clear {
            if !targets
                .iter()
                .any(|planned| planned.call_id == target.call_id)
            {
                targets.push(target);
            }
        }
    }
    targets.retain(|target| !keep.contains(&target.call_id));
    targets.sort_by_key(|target| (target.message, target.block));
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(message: usize, id: &str) -> ClearTarget {
        ClearTarget {
            message,
            block: 0,
            call_id: CallId::from(id.to_string()),
            chars: 2_000,
        }
    }

    #[test]
    fn keeping_wins_over_clearing_and_order_is_oldest_first() {
        let planned = vec![target(1, "a"), target(3, "b")];
        let advice = vec![
            ClearAdvice {
                keep: vec![CallId::from("b".to_string())],
                clear: vec![target(2, "c"), target(1, "a")],
            },
            ClearAdvice {
                keep: Vec::new(),
                clear: vec![target(3, "b")],
            },
        ];
        let ids: Vec<String> = apply(planned.clone(), advice)
            .iter()
            .map(|target| target.call_id.to_string())
            .collect();
        assert_eq!(ids, ["a", "c"]);
        assert_eq!(apply(planned.clone(), Vec::new()), planned);
    }
}
