//! `UseContext`: what a use sees while it runs at a seam: the session, the
//! service it asks, its feature, whether it runs in shadow, and its cancel
//! token.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use z_engine_config::FeatureId;
use z_engine_decisions::{Answer, DecisionRecord, DecisionRequest};
use z_engine_protocol::AgentId;

use super::service::DecisionService;
use crate::session::SessionCore;

#[derive(Debug, Clone)]
pub(crate) struct UseContext {
    pub core: Arc<SessionCore>,
    pub service: Arc<DecisionService>,
    pub feature: FeatureId,
    /// Shadow: record what would have happened; the advice is dropped.
    pub shadow: bool,
    pub cancel: CancellationToken,
    /// The agent whose call the seam is about (tool gate, after call).
    pub agent: Option<AgentId>,
}

impl UseContext {
    pub(crate) fn for_agent(self, agent: &AgentId) -> Self {
        Self {
            agent: Some(agent.clone()),
            ..self
        }
    }

    /// Asks the session's provider; failures and low confidence abstain.
    pub(crate) async fn ask(&self, request: &DecisionRequest) -> Vec<Answer> {
        let service = &self.service;
        service
            .ask(self.feature, self.shadow, request, &self.cancel)
            .await
    }

    /// `ask` without the dataset line, for states that may hold secrets.
    pub(crate) async fn ask_unrecorded(&self, request: &DecisionRequest) -> Vec<Answer> {
        self.service.ask_unrecorded(request, &self.cancel).await
    }

    /// A trace record of `answer` for this use, outcome `unchanged`.
    pub(crate) fn record_of(&self, answer: &Answer, fingerprint: &str) -> DecisionRecord {
        DecisionRecord::of(self.feature.as_str(), answer, fingerprint, self.shadow)
    }

    pub(crate) fn record(&self, record: DecisionRecord) {
        self.core.decisions.trace().record(record);
    }
}
