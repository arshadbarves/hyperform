//! Building clients against the mock server and draining their streams.

use std::time::Duration;

use z_engine_llm::{LlmError, ModelEvent, ModelRequest, ModelStream, ProviderConfig};
use z_engine_protocol::Message;

pub type Item = Result<ModelEvent, LlmError>;

const DRAIN_LIMIT: Duration = Duration::from_secs(10);

/// An OpenAI-compatible config with a (fake) key.
pub fn config(base_url: &str) -> ProviderConfig {
    ProviderConfig {
        base_url: base_url.to_string(),
        api_key: Some("test-key".into()),
        ..ProviderConfig::default()
    }
}

pub fn request(model: &str) -> ModelRequest {
    ModelRequest::new(model, vec![Message::user_text("hello")])
}

/// Every item until the stream ends.
pub async fn drain(mut stream: ModelStream) -> Vec<Item> {
    tokio::time::timeout(DRAIN_LIMIT, async move {
        let mut items = Vec::new();
        while let Some(item) = stream.recv().await {
            items.push(item);
        }
        items
    })
    .await
    .expect("stream did not end")
}

/// The next item, failing the test if none arrives in time.
pub async fn next(stream: &mut ModelStream) -> Option<Item> {
    tokio::time::timeout(Duration::from_secs(5), stream.recv())
        .await
        .expect("no stream item in time")
}
