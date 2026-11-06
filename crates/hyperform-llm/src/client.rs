//! The model transport seam. The engine depends only on this trait; tests
//! substitute a scripted implementation without any HTTP.

use tokio_util::sync::CancellationToken;

use crate::accumulate::{AssistantTurn, ResponseAccumulator};
use crate::error::LlmError;
use crate::types::{ModelRequest, ModelStream};

pub trait ModelClient: Send + Sync + std::fmt::Debug {
    /// Start a streaming response. The returned stream ends after
    /// `ModelEvent::Stop` or one terminal error. Cancelling `cancel` or
    /// dropping the receiver aborts the request, including retry backoff.
    fn stream(&self, request: ModelRequest, cancel: CancellationToken) -> ModelStream;

    /// Short provider label for diagnostics ("openrouter", "anthropic", ...).
    fn provider(&self) -> &str {
        "unknown"
    }
}

/// Drain a stream into a complete assistant turn (for side requests such as
/// titles and summaries that do not need live deltas).
pub async fn collect(mut stream: ModelStream) -> Result<AssistantTurn, LlmError> {
    let mut acc = ResponseAccumulator::default();
    while let Some(item) = stream.recv().await {
        acc.absorb(&item?);
    }
    Ok(acc.finish())
}
