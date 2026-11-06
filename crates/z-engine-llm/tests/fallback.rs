//! FallbackClient over the real adapter: a retryable failure before any
//! content moves the request to the next model; later failures are final.

mod support;

use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use z_engine_llm::{FallbackClient, LlmError, ModelClient, ModelEvent, StopReason, build_client};

use support::server::{MockServer, Reply};
use support::stream::{config, drain, request};

const TEXT: &str = include_str!("fixtures/sse/text.sse");
const ERROR: &str = include_str!("fixtures/sse/error_midstream.sse");

fn upstream_error() -> LlmError {
    LlmError::Http {
        status: 502,
        body: "Upstream provider returned an error".into(),
    }
}

fn fallback_client(server: &MockServer) -> FallbackClient {
    let inner = build_client(&config(&server.url)).unwrap();
    FallbackClient::new(inner, vec!["backup-model".into()])
}

fn models(server: &MockServer) -> Vec<Value> {
    server
        .requests()
        .iter()
        .map(|recorded| recorded.body["model"].clone())
        .collect()
}

#[tokio::test]
async fn retryable_failure_before_content_falls_back() {
    let server = MockServer::start(vec![Reply::Sse(ERROR.into()), Reply::Sse(TEXT.into())]).await;
    let client = fallback_client(&server);
    assert_eq!(client.provider(), "custom");
    let items = drain(client.stream(request("primary-model"), CancellationToken::new())).await;
    match &items[0] {
        Ok(ModelEvent::Retrying {
            attempt: 1,
            delay_ms: 0,
            reason,
        }) => assert_eq!(
            reason,
            &format!("falling back to backup-model: {}", upstream_error())
        ),
        other => panic!("expected a fallback notice, got {other:?}"),
    }
    assert_eq!(items[1], Ok(ModelEvent::TextDelta("Hello".into())));
    assert_eq!(
        items.last(),
        Some(&Ok(ModelEvent::Stop(StopReason::EndTurn)))
    );
    assert_eq!(
        models(&server),
        [json!("primary-model"), json!("backup-model")]
    );
}

#[tokio::test]
async fn failure_after_content_is_not_replayed() {
    let partial = format!(
        "data: {}\n\n{ERROR}",
        json!({"choices": [{"index": 0, "delta": {"content": "Hel"}}]})
    );
    let server = MockServer::start(vec![Reply::Sse(partial), Reply::Sse(TEXT.into())]).await;
    let client = fallback_client(&server);
    let items = drain(client.stream(request("primary-model"), CancellationToken::new())).await;
    assert_eq!(
        items,
        [
            Ok(ModelEvent::TextDelta("Hel".into())),
            Err(upstream_error())
        ]
    );
    assert_eq!(models(&server), [json!("primary-model")]);
}
