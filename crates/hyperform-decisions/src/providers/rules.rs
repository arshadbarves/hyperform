//! The rules provider: every answer abstains, so callers keep today's
//! deterministic behavior. Used when no model is configured.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::answer::{AbstainReason, Answer};
use crate::error::DecisionError;
use crate::provider::DecisionProvider;
use crate::question::DecisionRequest;

#[derive(Debug, Default, Clone, Copy)]
pub struct RulesProvider;

pub const RULES: &str = "rules";

#[async_trait]
impl DecisionProvider for RulesProvider {
    fn name(&self) -> &'static str {
        RULES
    }

    fn revision(&self) -> String {
        RULES.to_string()
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        _cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        Ok(request
            .questions
            .iter()
            .map(|question| Answer::abstained(&question.name, AbstainReason::Rules, RULES))
            .collect())
    }
}
