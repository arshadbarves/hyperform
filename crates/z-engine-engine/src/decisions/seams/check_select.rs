//! Seam: verification is about to run its automatic checks for the turn's
//! changes. `on` uses may name checks to skip; nothing is skipped when a
//! changed file is a test, and `strict` mode keeps its Test, Build or
//! Typecheck checks.

use std::path::PathBuf;
use std::sync::Arc;

use z_engine_verify::{CheckSpec, skip_checks, touches_tests};

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{Seam, USES};
use crate::run::RunContext;

pub(crate) async fn select_needed<'a>(
    ctx: &RunContext,
    selected: Vec<&'a CheckSpec>,
    changed: &[PathBuf],
    strict: bool,
) -> Vec<&'a CheckSpec> {
    if selected.is_empty() || touches_tests(changed) {
        return selected;
    }
    let active = active(&ctx.core, USES, Seam::CheckSelect);
    if active.is_empty() {
        return selected;
    }
    let checks: Arc<[CheckSpec]> = selected.iter().map(|check| (*check).clone()).collect();
    let changed: Arc<[PathBuf]> = Arc::from(changed);
    let skips = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let (checks, changed) = (Arc::clone(&checks), Arc::clone(&changed));
        Box::pin(async move { decision_use.check_select(&cx, &checks, &changed).await })
    })
    .await;
    let skip: Vec<String> = skips.into_iter().next().unwrap_or_default();
    skip_checks(selected, &skip, strict)
}
