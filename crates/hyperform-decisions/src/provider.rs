//! The provider seam: anything that answers typed questions about a state.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::answer::Answer;
use crate::error::DecisionError;
use crate::question::DecisionRequest;

#[async_trait]
pub trait DecisionProvider: Send + Sync + std::fmt::Debug {
    /// Short name shown in traces (`rules`, `systemone`, `hybrid`).
    fn name(&self) -> &'static str;

    /// Changes whenever answers could change (endpoint, checkpoint, model);
    /// part of every cache key.
    fn revision(&self) -> String;

    /// One answer per question of `request`, in order. A provider may
    /// abstain per question; an error means no answer at all. Returns
    /// promptly with `Cancelled` once `cancel` fires.
    async fn decide(
        &self,
        request: &DecisionRequest,
        cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError>;
}
