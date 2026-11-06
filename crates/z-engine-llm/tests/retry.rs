//! Transient HTTP failures are retried with `Retrying` notices; failures
//! that persist or cannot be retried surface as typed errors.

mod support;

use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;
use z_engine_llm::{LlmError, ModelEvent, StopReason, build_client};

use support::server::{MockServer, Reply};
use support::stream::{Item, config, drain, request};

const TEXT: &str = include_str!("fixtures/sse/text.sse");

fn busy(status: u16) -> Reply {
    Reply::status(status, &[("retry-after-ms", "1")], "slow down")
}

async fn run(replies: Vec<Reply>) -> (Vec<Item>, usize) {
    let server = MockServer::start(replies).await;
    let client = build_client(&config(&server.url)).unwrap();
    let items = drain(client.stream(request("m"), CancellationToken::new())).await;
    (items, server.requests().len())
}

fn retries(items: &[Item]) -> usize {
    items
        .iter()
        .filter(|item| matches!(item, Ok(ModelEvent::Retrying { .. })))
        .count()
}

#[tokio::test]
async fn rate_limit_honors_retry_after_then_succeeds() {
    let started = Instant::now();
    let (items, requests) = run(vec![
        Reply::status(
            429,
            &[("retry-after", "1")],
            r#"{"error":{"message":"slow down"}}"#,
        ),
        Reply::Sse(TEXT.into()),
    ])
    .await;
    assert!(started.elapsed() >= Duration::from_millis(900));
    assert_eq!(
        items[0],
        Ok(ModelEvent::Retrying {
            attempt: 1,
            delay_ms: 1_000,
            reason: "rate limited (HTTP 429)".into()
        })
    );
    assert_eq!(items[1], Ok(ModelEvent::TextDelta("Hello".into())));
    assert_eq!(
        items.last(),
        Some(&Ok(ModelEvent::Stop(StopReason::EndTurn)))
    );
    assert_eq!(requests, 2);
}

#[tokio::test]
async fn persistent_rate_limits_surface_as_rate_limited() {
    let (items, requests) = run((0..5).map(|_| busy(429)).collect()).await;
    assert_eq!(requests, 5);
    assert_eq!(retries(&items), 4);
    let attempts: Vec<u32> = items
        .iter()
        .filter_map(|item| match item {
            Ok(ModelEvent::Retrying { attempt, .. }) => Some(*attempt),
            _ => None,
        })
        .collect();
    assert_eq!(attempts, [1, 2, 3, 4]);
    assert_eq!(
        items.last(),
        Some(&Err(LlmError::RateLimited {
            attempts: 5,
            detail: "slow down".into()
        }))
    );
}

#[tokio::test]
async fn overload_then_server_error_then_success() {
    let (items, requests) = run(vec![
        busy(529),
        busy(503),
        busy(502),
        Reply::Sse(TEXT.into()),
    ])
    .await;
    assert_eq!(requests, 4);
    assert_eq!(retries(&items), 3);
    assert_eq!(
        items.last(),
        Some(&Ok(ModelEvent::Stop(StopReason::EndTurn)))
    );
}

#[tokio::test]
async fn persistent_overload_surfaces_as_overloaded() {
    let (items, _) = run((0..5).map(|_| busy(529)).collect()).await;
    assert!(matches!(
        items.last(),
        Some(Err(LlmError::Overloaded { attempts: 5, .. }))
    ));
}

#[tokio::test]
async fn client_errors_fail_at_once_with_the_body() {
    let (items, requests) = run(vec![Reply::status(
        400,
        &[],
        "prompt is too long: 300000 tokens > 200000 maximum",
    )])
    .await;
    assert_eq!(requests, 1);
    assert_eq!(items.len(), 1);
    let error = items[0].clone().unwrap_err();
    assert!(error.is_context_overflow(), "{error}");
    assert!(!error.is_retryable());
}
