//! Cancelling a request, or dropping its stream, ends it promptly and lets
//! go of the connection: while streaming, awaiting headers, or backing off.

mod support;

use std::time::{Duration, Instant};

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use z_engine_llm::{LlmError, ModelEvent, build_client};

use support::server::{MockServer, Reply};
use support::stream::{config, next, request};

const STARTED: &str = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"started\"}}]}\n\n";
const PROMPT: Duration = Duration::from_secs(1);

async fn stalled_server() -> (MockServer, oneshot::Receiver<()>) {
    let (closed, released) = oneshot::channel();
    let server = MockServer::start(vec![Reply::Stall {
        prefix: STARTED.into(),
        closed,
    }])
    .await;
    (server, released)
}

async fn assert_released(released: oneshot::Receiver<()>) {
    let outcome = tokio::time::timeout(Duration::from_secs(5), released).await;
    assert!(
        outcome.is_ok(),
        "the server kept streaming to a gone client"
    );
}

#[tokio::test]
async fn cancel_mid_stream_ends_promptly_and_releases_the_connection() {
    let (server, released) = stalled_server().await;
    let client = build_client(&config(&server.url)).unwrap();
    let cancel = CancellationToken::new();
    let mut stream = client.stream(request("m"), cancel.clone());
    assert_eq!(
        next(&mut stream).await,
        Some(Ok(ModelEvent::TextDelta("started".into())))
    );
    let cancelled_at = Instant::now();
    cancel.cancel();
    assert_eq!(next(&mut stream).await, Some(Err(LlmError::Cancelled)));
    assert_eq!(next(&mut stream).await, None);
    assert!(cancelled_at.elapsed() < PROMPT);
    assert_released(released).await;
}

#[tokio::test]
async fn dropping_the_stream_releases_the_connection() {
    let (server, released) = stalled_server().await;
    let client = build_client(&config(&server.url)).unwrap();
    let mut stream = client.stream(request("m"), CancellationToken::new());
    assert_eq!(
        next(&mut stream).await,
        Some(Ok(ModelEvent::TextDelta("started".into())))
    );
    drop(stream);
    assert_released(released).await;
}

#[tokio::test]
async fn cancel_while_awaiting_headers_is_prompt() {
    let server = MockServer::start(vec![Reply::Hang]).await;
    let client = build_client(&config(&server.url)).unwrap();
    let cancel = CancellationToken::new();
    let mut stream = client.stream(request("m"), cancel.clone());
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.requests().is_empty() {
        assert!(
            Instant::now() < deadline,
            "request never reached the server"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let cancelled_at = Instant::now();
    cancel.cancel();
    assert_eq!(next(&mut stream).await, Some(Err(LlmError::Cancelled)));
    assert!(cancelled_at.elapsed() < PROMPT);
}

#[tokio::test]
async fn cancel_during_retry_backoff_is_prompt() {
    let server =
        MockServer::start(vec![Reply::status(429, &[("retry-after", "30")], "busy")]).await;
    let client = build_client(&config(&server.url)).unwrap();
    let cancel = CancellationToken::new();
    let mut stream = client.stream(request("m"), cancel.clone());
    match next(&mut stream).await {
        Some(Ok(ModelEvent::Retrying { delay_ms, .. })) => assert_eq!(delay_ms, 30_000),
        other => panic!("expected a retry notice, got {other:?}"),
    }
    let cancelled_at = Instant::now();
    cancel.cancel();
    assert_eq!(next(&mut stream).await, Some(Err(LlmError::Cancelled)));
    assert!(cancelled_at.elapsed() < PROMPT);
    assert_eq!(server.requests().len(), 1);
}
