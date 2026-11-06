//! Review suggestion (`decisions_review_suggest`): when the main agent
//! stops, whether its changes touch risky areas. Path rules decide first
//! (`rules`), the model only for ambiguous files (`suggest`). A hit offers a
//! card that runs the review command; nothing runs by itself and the agent
//! is never sent back.

mod rules;
mod suggest;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use async_trait::async_trait;
use z_engine_config::FeatureId;

use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

#[derive(Debug)]
pub(crate) struct ReviewSuggest;

pub(crate) static REVIEW_SUGGEST: ReviewSuggest = ReviewSuggest;

#[async_trait]
impl DecisionUse for ReviewSuggest {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsReviewSuggest
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::Stop]
    }

    async fn stop(&self, cx: &UseContext, changed: &[PathBuf]) -> Option<String> {
        suggest::suggest(cx, changed).await;
        None
    }
}
