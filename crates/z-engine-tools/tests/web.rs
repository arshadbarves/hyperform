//! `WebFetch` and `WebSearch` against a local server: extraction by the
//! side model, raw fallback, redirects, HTTP errors, and search results.

mod support;

use std::sync::Arc;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::get;
use serde_json::json;
use support::fakes::FakeSideModel;
use support::{ctx, err_text, ok_text, project};
use z_engine_host::SearchBackend;
use z_engine_policy::Action;
use z_engine_tools::builtin::{WebFetchTool, WebSearchTool};
use z_engine_tools::{Ports, Tool, ToolCtx, ToolError, WebOptions};

async fn serve() -> String {
    let app = Router::new()
        .route("/page", get(|| async { Html("<html><body><h1>Install</h1><p>Run <code>make</code>.</p></body></html>") }))
        .route("/moved", get(|| async { Redirect::temporary("https://example.com/elsewhere") }))
        .route("/missing", get(|| async { (StatusCode::NOT_FOUND, "no such page") }))
        .route(
            "/search",
            get(|| async {
                let body = json!({"results": [
                    {"title": "Tokio docs", "url": "https://docs.rs/tokio", "content": "An async <b>runtime</b>."},
                    {"title": "Spam", "url": "https://spam.example/x", "content": "buy"}
                ]});
                ([(header::CONTENT_TYPE, "application/json")], body.to_string()).into_response()
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn local(dir: &tempfile::TempDir, side: Option<Arc<FakeSideModel>>) -> ToolCtx {
    let mut ctx = ctx(dir.path());
    ctx.web_options.allow_private_network = true;
    ctx.ports = Arc::new(Ports {
        side_model: side.map(|side| side as _),
        ..Ports::default()
    });
    ctx
}

#[tokio::test]
async fn fetch_answers_the_prompt_with_the_side_model() {
    let base = serve().await;
    let dir = project(&[]);
    let side = Arc::new(FakeSideModel::default());
    let ctx = local(&dir, Some(Arc::clone(&side)));
    let url = format!("{base}/page");
    let output = WebFetchTool
        .call(json!({"url": url, "prompt": "How do I install it?"}), &ctx)
        .await
        .unwrap();
    assert_eq!(
        output.text_content(),
        format!("extracted answer to: How do I install it?\n\n(Source: {url})")
    );
    assert!(
        output.summary.starts_with("Fetched 127.0.0.1:"),
        "{}",
        output.summary
    );
    let calls = side.calls.lock().unwrap();
    assert!(
        calls[0].0.starts_with(&format!("URL: {url}\n\n")),
        "{}",
        calls[0].0
    );
    assert!(calls[0].0.contains("# Install"), "{}", calls[0].0);
}

#[tokio::test]
async fn fetch_returns_the_page_without_extraction() {
    let base = serve().await;
    let dir = project(&[]);
    let mut ctx = local(&dir, Some(Arc::new(FakeSideModel::default())));
    ctx.web_options.fetch_extract = false;
    let text = ok_text(
        &WebFetchTool,
        &ctx,
        json!({"url": format!("{base}/page"), "prompt": "x"}),
    )
    .await;
    assert!(
        text.starts_with(&format!("Content of {base}/page:\n\n")),
        "{text}"
    );
    assert!(
        text.contains("# Install") && text.contains("`make`"),
        "{text}"
    );

    let failing = local(
        &dir,
        Some(Arc::new(FakeSideModel {
            fail: true,
            ..FakeSideModel::default()
        })),
    );
    let text = ok_text(
        &WebFetchTool,
        &failing,
        json!({"url": format!("{base}/page"), "prompt": "x"}),
    )
    .await;
    assert!(
        text.starts_with("(The page could not be summarized: model overloaded."),
        "{text}"
    );
}

#[tokio::test]
async fn cross_host_redirects_and_http_errors_are_reported() {
    let base = serve().await;
    let dir = project(&[]);
    let ctx = local(&dir, None);
    let output = WebFetchTool
        .call(json!({"url": format!("{base}/moved"), "prompt": "x"}), &ctx)
        .await
        .unwrap();
    assert!(
        output
            .text_content()
            .contains("redirected to a different host: https://example.com/elsewhere")
    );
    assert!(
        output
            .text_content()
            .contains("call WebFetch again with url \"https://example.com/elsewhere\"")
    );
    assert_eq!(output.summary, "Redirected to example.com");
    let output = WebFetchTool
        .call(
            json!({"url": format!("{base}/missing"), "prompt": "x"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(output.is_error);
    assert!(
        output.text_content().starts_with("HTTP 404 from"),
        "{}",
        output.text_content()
    );
}

#[tokio::test]
async fn fetch_refuses_private_targets_unless_allowed() {
    let base = serve().await;
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let err = err_text(
        &WebFetchTool,
        &ctx,
        json!({"url": format!("{base}/page"), "prompt": "x"}),
    )
    .await;
    assert!(err.contains("was not fetched"), "{err}");
    let err = err_text(
        &WebFetchTool,
        &ctx,
        json!({"url": "ftp://example.com", "prompt": "x"}),
    )
    .await;
    assert!(err.contains("unsupported URL scheme"), "{err}");
    let input = json!({"url": "https://docs.rs/x", "prompt": "p"});
    assert_eq!(
        WebFetchTool.action(&input, &ctx),
        Action::Fetch {
            url: "https://docs.rs/x".into()
        }
    );
    assert!(WebFetchTool.is_read_only(&input));
}

#[tokio::test]
async fn search_lists_filtered_results() {
    let base = serve().await;
    let dir = project(&[]);
    let mut ctx = ctx(dir.path());
    ctx.web_options = WebOptions {
        search: SearchBackend::Searxng { base_url: base },
        ..WebOptions::default()
    };
    let output = WebSearchTool
        .call(
            json!({"query": "tokio runtime", "blocked_domains": ["spam.example"]}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        output.text_content(),
        "Web search results for \"tokio runtime\":\n\n1. Tokio docs\n   https://docs.rs/tokio\n   An async runtime.\n"
    );
    assert_eq!(output.summary, "1 result");
    assert_eq!(WebSearchTool.action(&json!({}), &ctx), Action::Search);
}

#[tokio::test]
async fn search_without_a_backend_is_unavailable() {
    let dir = project(&[]);
    let ctx = ctx(dir.path());
    let err = WebSearchTool
        .call(json!({"query": "rust"}), &ctx)
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Unavailable(_)), "{err:?}");
    let err = err_text(&WebSearchTool, &ctx, json!({"query": "r"})).await;
    assert!(err.contains("at least 2 characters"), "{err}");
}
