//! `RelevancePort`: which parts of a long tool result matter for the task,
//! through the decision layer's relevance seam. Provided only while
//! `decisions_output_trim` or `decisions_search_rank` runs.

use async_trait::async_trait;
use z_engine_config::FeatureId;
use z_engine_tools::{RankRequest, RankTarget, RelevancePort, ToolCtx};

use crate::decisions::seams::rank_items;
use crate::run::RunContext;

#[derive(Debug)]
pub(crate) struct Relevance {
    run: RunContext,
}

fn feature_of(target: RankTarget) -> FeatureId {
    match target {
        RankTarget::OutputWindows => FeatureId::DecisionsOutputTrim,
        RankTarget::SearchHits => FeatureId::DecisionsSearchRank,
    }
}

impl Relevance {
    /// `None` while neither ranking feature runs.
    pub(crate) fn for_run(run: &RunContext) -> Option<Self> {
        let relevance = Self { run: run.clone() };
        let any = [RankTarget::OutputWindows, RankTarget::SearchHits];
        any.into_iter()
            .any(|target| relevance.ranks(target))
            .then_some(relevance)
    }
}

#[async_trait]
impl RelevancePort for Relevance {
    fn ranks(&self, target: RankTarget) -> bool {
        let service = self.run.core.decisions.service();
        service.mode(feature_of(target)).runs()
    }

    async fn rank(&self, _ctx: &ToolCtx, request: RankRequest) -> Option<Vec<Option<bool>>> {
        rank_items(&self.run, request).await
    }
}
