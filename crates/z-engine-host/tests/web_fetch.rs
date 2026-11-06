//! Page fetches against a local server: conversion, redirects, the cache,
//! body caps, and the private-network guard.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::get;
use tokio_util::sync::CancellationToken;
use z_engine_host::{FetchOptions, HostError, WebClient};

const PAGE: &str = "<html><head><title>T</title><script>evil()</script></head><body><h1>Title</h1><p>Hello <b>world</b></p></body></html>";

async fn serve() -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/page", get(|| async { Html(PAGE) }))
        .route(
            "/json",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    r#"{"a":[1,2]}"#,
                )
            }),
        )
        .route("/same", get(|| async { Redirect::temporary("/page") }))
        .route("/chain", get(|| async { Redirect::permanent("/same") }))
        .route(
            "/cross",
            get(|| async { Redirect::temporary("https://example.com/elsewhere") }),
        )
        .route("/loop", get(|| async { Redirect::temporary("/loop") }))
        .route("/big", get(|| async { "x".repeat(10_000) }))
        .route("/counted", get(counted))
        .with_state(Arc::clone(&hits));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), hits)
}

async fn counted(State(hits): State<Arc<AtomicUsize>>) -> impl IntoResponse {
    hits.fetch_add(1, Ordering::SeqCst);
    "counted"
}

fn local() -> FetchOptions {
    FetchOptions {
        allow_private_network: true,
        ..FetchOptions::default()
    }
}

async fn fetch(url: &str, opts: &FetchOptions) -> Result<z_engine_host::FetchedPage, HostError> {
    let client = WebClient::new().unwrap();
    client.fetch(url, opts, CancellationToken::new()).await
}

#[tokio::test]
async fn html_becomes_markdown_and_json_is_pretty_printed() {
    let (base, _) = serve().await;
    let page = fetch(&format!("{base}/page"), &local()).await.unwrap();
    assert_eq!(page.status, 200);
    assert!(page.content_type.starts_with("text/html"));
    assert!(page.content.contains("# Title"), "{}", page.content);
    assert!(page.content.contains("Hello **world**"), "{}", page.content);
    assert!(!page.content.contains("evil"));
    assert_eq!(page.final_url, format!("{base}/page"));
    assert!(!page.from_cache && !page.truncated && page.redirect.is_none());

    let json = fetch(&format!("{base}/json"), &local()).await.unwrap();
    assert_eq!(json.content, "{\n  \"a\": [\n    1,\n    2\n  ]\n}");
}

#[tokio::test]
async fn same_host_redirects_are_followed_and_loops_give_up() {
    let (base, _) = serve().await;
    let page = fetch(&format!("{base}/chain"), &local()).await.unwrap();
    assert_eq!(page.url, format!("{base}/chain"));
    assert_eq!(page.final_url, format!("{base}/page"));
    assert!(page.content.contains("# Title"));

    let looping = fetch(&format!("{base}/loop"), &local()).await;
    assert!(matches!(looping, Err(HostError::Http(_))), "{looping:?}");
}

#[tokio::test]
async fn cross_host_redirects_are_reported_not_followed() {
    let (base, _) = serve().await;
    let page = fetch(&format!("{base}/cross"), &local()).await.unwrap();
    assert_eq!(page.status, 307);
    assert_eq!(
        page.redirect.as_deref(),
        Some("https://example.com/elsewhere")
    );
    assert_eq!(page.final_url, format!("{base}/cross"));
    assert!(page.content.is_empty());
}

#[tokio::test]
async fn repeated_fetches_are_served_from_the_cache() {
    let (base, hits) = serve().await;
    let client = WebClient::new().unwrap();
    let url = format!("{base}/counted");
    let first = client
        .fetch(&url, &local(), CancellationToken::new())
        .await
        .unwrap();
    let second = client
        .clone()
        .fetch(&url, &local(), CancellationToken::new())
        .await
        .unwrap();
    assert!(!first.from_cache && second.from_cache);
    assert_eq!(second.content, "counted");
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // A page fetched with private access is not served to a guarded call.
    let guarded = client
        .fetch(&url, &FetchOptions::default(), CancellationToken::new())
        .await;
    assert!(matches!(guarded, Err(HostError::Blocked(_))), "{guarded:?}");
}

#[tokio::test]
async fn private_addresses_are_blocked_unless_allowed() {
    let (base, hits) = serve().await;
    let blocked = fetch(&format!("{base}/counted"), &FetchOptions::default()).await;
    assert!(matches!(blocked, Err(HostError::Blocked(_))), "{blocked:?}");
    let port = base.rsplit(':').next().unwrap();
    let by_name = fetch(
        &format!("http://localhost:{port}/counted"),
        &FetchOptions::default(),
    )
    .await;
    assert!(matches!(by_name, Err(HostError::Blocked(_))), "{by_name:?}");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "no request reached the server"
    );
}

#[tokio::test]
async fn bodies_are_capped_and_bad_input_is_rejected() {
    let (base, _) = serve().await;
    let capped = FetchOptions {
        max_bytes: 1_000,
        ..local()
    };
    let page = fetch(&format!("{base}/big"), &capped).await.unwrap();
    assert!(page.truncated);
    assert_eq!(page.content.len(), 1_000);

    assert!(matches!(
        fetch("ftp://example.com/x", &local()).await,
        Err(HostError::Invalid(_))
    ));
    assert!(matches!(
        fetch("not a url", &local()).await,
        Err(HostError::Invalid(_))
    ));
    let cancel = CancellationToken::new();
    cancel.cancel();
    let client = WebClient::new().unwrap();
    let cancelled = client
        .fetch(&format!("{base}/page"), &local(), cancel)
        .await;
    assert!(matches!(cancelled, Err(HostError::Cancelled)));
}
