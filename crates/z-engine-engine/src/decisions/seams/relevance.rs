//! Seam: a tool is about to cut a long result (command output, search
//! results). The first answer from an `on` use says which parts matter;
//! no answer in time keeps the tool's usual cut.

use std::sync::Arc;

use z_engine_tools::RankRequest;

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{Seam, USES};
use crate::run::RunContext;

pub(crate) async fn rank_items(
    ctx: &RunContext,
    request: RankRequest,
) -> Option<Vec<Option<bool>>> {
    let active = active(&ctx.core, USES, Seam::Relevance);
    if active.is_empty() {
        return None;
    }
    let count = request.items.len();
    let request = Arc::new(request);
    let ranked = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let request = Arc::clone(&request);
        Box::pin(async move { decision_use.relevance(&cx, &request).await })
    })
    .await;
    ranked
        .into_iter()
        .flatten()
        .find(|answers| answers.len() == count)
}
