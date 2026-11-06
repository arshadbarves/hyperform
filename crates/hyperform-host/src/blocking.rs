//! Moves blocking filesystem walks and CPU-heavy work off the async executor.

use crate::HostError;

pub(crate) async fn run_blocking<T, F>(work: F) -> Result<T, HostError>
where
    F: FnOnce() -> Result<T, HostError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| HostError::Process(format!("blocking task failed: {e}")))?
}
