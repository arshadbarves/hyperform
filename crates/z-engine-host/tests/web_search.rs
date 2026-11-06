//! Search backends: request shapes and result parsing against a local
//! server standing in for each API.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use z_engine_host::{HostError, SearchBackend, SearchHit, WebClient};

#[derive(Debug, Clone)]
struct Seen {
    method: Method,
    path: String,
    query: HashMap<String, String>,
    headers: HeaderMap,
    body: Vec<u8>,
}

#[derive(Clone)]
struct Api {
    seen: Arc<Mutex<Option<Seen>>>,
    status: StatusCode,
    response: Value,
}

async fn capture(
    State(api): State<Api>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let seen = Seen {
        method,
        path: uri.path().to_string(),
        query: parse_query(uri.query().unwrap_or("")),
        headers,
        body: body.to_vec(),
    };
    *api.seen.lock().unwrap() = Some(seen);
    (
        api.status,
        [(header::CONTENT_TYPE, "application/json")],
        api.response.to_string(),
    )
}

/// Minimal `application/x-www-form-urlencoded` decoding for assertions.
fn parse_query(query: &str) -> HashMap<String, String> {
    let decode = |s: &str| {
        let s = s.replace('+', " ");
        let mut out = Vec::new();
        let mut bytes = s.bytes();
        while let Some(b) = bytes.next() {
            if b == b'%' {
                let hex: String = bytes.by_ref().take(2).map(char::from).collect();
                out.push(u8::from_str_radix(&hex, 16).unwrap());
            } else {
                out.push(b);
            }
        }
        String::from_utf8(out).unwrap()
    };
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (decode(k), decode(v)))
        .collect()
}

async fn serve(status: StatusCode, response: Value) -> (String, Arc<Mutex<Option<Seen>>>) {
    let seen = Arc::new(Mutex::new(None));
    let api = Api {
        seen: Arc::clone(&seen),
        status,
        response,
    };
    let app = Router::new().fallback(capture).with_state(api);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), seen)
}

async fn search(
    base: &str,
    backend: &SearchBackend,
    allowed: &[&str],
    blocked: &[&str],
    limit: usize,
) -> Result<Vec<SearchHit>, HostError> {
    let client = WebClient::new().unwrap().with_search_base(base);
    let owned = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    client
        .search(
            backend,
            "rust async",
            &owned(allowed),
            &owned(blocked),
            limit,
            CancellationToken::new(),
        )
        .await
}

fn header<'a>(seen: &'a Seen, name: &str) -> &'a str {
    seen.headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

fn body(seen: &Seen) -> Value {
    serde_json::from_slice(&seen.body).unwrap()
}

#[tokio::test]
async fn brave_uses_a_get_with_the_subscription_token() {
    let response = json!({"web": {"results": [
        {"title": "<strong>Tokio</strong>", "url": "https://tokio.rs/", "description": "An <strong>async</strong>  runtime"},
        {"title": "Ad", "url": "https://ads.spam.example/x", "description": "buy"}
    ]}});
    let (base, seen) = serve(StatusCode::OK, response).await;
    let backend = SearchBackend::Brave {
        api_key: "brave-key".into(),
    };
    let hits = search(&base, &backend, &[], &["spam.example"], 3)
        .await
        .unwrap();
    assert_eq!(
        hits,
        [SearchHit {
            title: "Tokio".into(),
            url: "https://tokio.rs/".into(),
            snippet: "An async runtime".into()
        }]
    );
    let seen = seen.lock().unwrap().clone().unwrap();
    assert_eq!(
        (&seen.method, seen.path.as_str()),
        (&Method::GET, "/res/v1/web/search")
    );
    assert_eq!(seen.query["q"], "rust async");
    assert_eq!(seen.query["count"], "3");
    assert_eq!(header(&seen, "x-subscription-token"), "brave-key");
}

#[tokio::test]
async fn tavily_posts_json_with_bearer_auth_and_domain_lists() {
    let response = json!({"results": [
        {"title": "Docs", "url": "https://docs.rs/tokio", "content": "Tokio docs"},
        {"title": "Other", "url": "https://example.com/", "content": "not allowed"}
    ]});
    let (base, seen) = serve(StatusCode::OK, response).await;
    let backend = SearchBackend::Tavily {
        api_key: "tv-key".into(),
    };
    let hits = search(&base, &backend, &["docs.rs"], &[], 2).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].snippet, "Tokio docs");
    let seen = seen.lock().unwrap().clone().unwrap();
    assert_eq!(
        (&seen.method, seen.path.as_str()),
        (&Method::POST, "/search")
    );
    assert_eq!(header(&seen, "authorization"), "Bearer tv-key");
    assert_eq!(
        body(&seen),
        json!({"query": "rust async", "max_results": 2, "include_domains": ["docs.rs"]})
    );
}

#[tokio::test]
async fn exa_posts_json_with_an_api_key_header() {
    let response = json!({"results": [{"title": "Exa hit", "url": "https://blog.example.org/a", "text": "Body text"}]});
    let (base, seen) = serve(StatusCode::OK, response).await;
    let backend = SearchBackend::Exa {
        api_key: "exa-key".into(),
    };
    let hits = search(&base, &backend, &[], &["x.com"], 5).await.unwrap();
    assert_eq!(hits[0].title, "Exa hit");
    assert_eq!(hits[0].snippet, "Body text");
    let seen = seen.lock().unwrap().clone().unwrap();
    assert_eq!(
        (&seen.method, seen.path.as_str()),
        (&Method::POST, "/search")
    );
    assert_eq!(header(&seen, "x-api-key"), "exa-key");
    assert_eq!(
        body(&seen),
        json!({
            "query": "rust async",
            "numResults": 5,
            "excludeDomains": ["x.com"],
            "contents": {"text": {"maxCharacters": 500}}
        })
    );
}

#[tokio::test]
async fn searxng_queries_the_configured_instance() {
    let response = json!({"results": [
        {"title": "One", "url": "https://one.example/", "content": "first"},
        {"title": "Two", "url": "https://two.example/", "content": "second"}
    ]});
    let (base, seen) = serve(StatusCode::OK, response).await;
    let backend = SearchBackend::Searxng {
        base_url: format!("{base}/"),
    };
    let client = WebClient::new().unwrap();
    let hits = client
        .search(
            &backend,
            "rust async",
            &[],
            &[],
            1,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, "https://one.example/");
    let seen = seen.lock().unwrap().clone().unwrap();
    assert_eq!(
        (&seen.method, seen.path.as_str()),
        (&Method::GET, "/search")
    );
    assert_eq!(seen.query["q"], "rust async");
    assert_eq!(seen.query["format"], "json");
}

#[tokio::test]
async fn failures_are_typed() {
    let (base, _) = serve(StatusCode::UNAUTHORIZED, json!({"error": "bad key"})).await;
    let backend = SearchBackend::Brave {
        api_key: "wrong".into(),
    };
    match search(&base, &backend, &[], &[], 3).await {
        Err(HostError::Http(message)) => assert!(message.contains("401"), "{message}"),
        other => panic!("expected an HTTP error, got {other:?}"),
    }
    let none = search(&base, &SearchBackend::None, &[], &[], 3).await;
    assert!(matches!(none, Err(HostError::Invalid(_))));
    let backend = SearchBackend::Tavily {
        api_key: "k".into(),
    };
    let client = WebClient::new().unwrap().with_search_base(&base);
    let empty = client
        .search(&backend, "  ", &[], &[], 3, CancellationToken::new())
        .await;
    assert!(matches!(empty, Err(HostError::Invalid(_))));
}
