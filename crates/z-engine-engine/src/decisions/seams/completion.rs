//! Seam: the main agent stopped with changes no check backs. The first
//! claim from an `on` use reaches the verifier, which may run checks or
//! show it on the turn's receipt; a claim never changes the badge.

use std::path::PathBuf;
use std::sync::Arc;

use z_engine_protocol::decisions::UncheckedClaim;

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{Seam, USES};
use crate::run::RunContext;

pub(crate) async fn review_completion(
    ctx: &RunContext,
    changed: &[PathBuf],
) -> Option<UncheckedClaim> {
    let active = active(&ctx.core, USES, Seam::Completion);
    if active.is_empty() {
        return None;
    }
    let changed: Arc<[PathBuf]> = Arc::from(changed);
    let claims = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let changed = Arc::clone(&changed);
        Box::pin(async move { decision_use.completion(&cx, &changed).await })
    })
    .await;
    claims.into_iter().flatten().next()
}
