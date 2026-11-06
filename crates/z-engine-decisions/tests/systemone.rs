//! The SystemOne provider against a local fake laya-serve: the request it
//! sends, bearer auth, batching, and each way the server can fail.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::post;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use z_engine_decisions::{
    DecisionError, DecisionProvider, DecisionRequest, Question, SystemOneConfig, SystemOneProvider,
    probe,
};

#[derive(Clone, Copy)]
enum Reply {
    /// Answers `a` (yes) to every question, with confidence 0.9.
    Yes,
    Garbage,
    Unauthorized,
    Slow,
}

#[derive(Default)]
struct Seen {
    auth: Vec<Option<String>>,
    bodies: Vec<Value>,
}

type Shared = (Reply, Arc<Mutex<Seen>>);

async fn systemone(
    State((reply, seen)): State<Shared>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let request: Value = serde_json::from_slice(&body).unwrap();
    let auth = headers
        .get(header::AUTHORIZATION)
        .map(|v| v.to_str().unwrap().to_string());
    let names: Vec<String> = request["questions"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    {
        let mut seen = seen.lock().unwrap();
        seen.auth.push(auth);
        seen.bodies.push(request);
    }
    let answers: serde_json::Map<String, Value> = names
        .into_iter()
        .map(|name| {
            (
                name,
                json!({ "choice": "a", "answer_confidence": 0.9, "act_probability": 0.1 }),
            )
        })
        .collect();
    let ok = json!({ "answers": answers, "routing": { "system": 1 } }).to_string();
    match reply {
        Reply::Yes => (StatusCode::OK, ok),
        Reply::Garbage => (StatusCode::OK, "{\"answers\": 3}".to_string()),
        Reply::Unauthorized => (StatusCode::UNAUTHORIZED, "missing key".to_string()),
        Reply::Slow => {
            tokio::time::sleep(Duration::from_secs(5)).await;
            (StatusCode::OK, ok)
        }
    }
}

async fn serve(reply: Reply) -> (String, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let app = Router::new()
        .route("/v1/systemone", post(systemone))
        .with_state((reply, Arc::clone(&seen)));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), seen)
}

fn config(endpoint: &str) -> SystemOneConfig {
    SystemOneConfig {
        endpoint: endpoint.to_string(),
        api_key: Some("secret".into()),
        checkpoint: "multilingual".into(),
        max_len: 1024,
        timeout: Duration::from_millis(500),
        allow_remote: false,
        max_batch: 2,
    }
}

fn question(name: &str) -> Question {
    Question::yes_no(name, "Relevant?\n- yes: Needed.\n- no: Not needed.\n").unwrap()
}

fn request(count: usize) -> DecisionRequest {
    (0..count).fold(
        DecisionRequest::new(json!({ "text": "hi" })),
        |request, i| request.ask(question(&format!("q{i}"))),
    )
}

#[tokio::test]
async fn batches_questions_with_bearer_auth_and_decodes_answers() {
    let (url, seen) = serve(Reply::Yes).await;
    let provider = SystemOneProvider::new(config(&url)).unwrap();
    let answers = provider
        .decide(&request(3), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(answers.len(), 3);
    assert!(answers.iter().all(|answer| answer.yes() == Some(true)));
    assert_eq!(answers[2].question, "q2");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.bodies.len(), 2, "max_batch 2 splits three questions");
    assert!(
        seen.auth
            .iter()
            .all(|auth| auth.as_deref() == Some("Bearer secret"))
    );
    let first = &seen.bodies[0];
    assert_eq!(first["state"], json!({ "text": "hi" }));
    assert_eq!(first["questions"]["q0"]["type"], "choice");
    assert_eq!(first["questions"]["q0"]["criteria"]["b"], "Not needed.");
    assert_eq!(first["model"], "multilingual");
    assert_eq!(first["max_len"], 1024);
}

#[tokio::test]
async fn server_failures_are_typed_errors() {
    let cancel = CancellationToken::new();
    let (garbage, _) = serve(Reply::Garbage).await;
    let error = SystemOneProvider::new(config(&garbage))
        .unwrap()
        .decide(&request(1), &cancel)
        .await;
    assert!(
        matches!(error, Err(DecisionError::Malformed(_))),
        "{error:?}"
    );
    let (locked, _) = serve(Reply::Unauthorized).await;
    let error = SystemOneProvider::new(config(&locked))
        .unwrap()
        .decide(&request(1), &cancel)
        .await;
    assert!(
        matches!(error, Err(DecisionError::Status { status: 401, .. })),
        "{error:?}"
    );
    let error = SystemOneProvider::new(config("http://127.0.0.1:9"))
        .unwrap()
        .decide(&request(1), &cancel)
        .await;
    assert!(
        matches!(error, Err(DecisionError::Unavailable(_))),
        "{error:?}"
    );
}

#[tokio::test]
async fn slow_servers_time_out_and_cancellation_wins() {
    let (url, _) = serve(Reply::Slow).await;
    let provider = SystemOneProvider::new(config(&url)).unwrap();
    let started = std::time::Instant::now();
    let error = provider
        .decide(&request(1), &CancellationToken::new())
        .await;
    assert!(
        matches!(error, Err(DecisionError::Timeout(500))),
        "{error:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    let cancel = CancellationToken::new();
    cancel.cancel();
    let error = provider.decide(&request(1), &cancel).await;
    assert!(matches!(error, Err(DecisionError::Cancelled)), "{error:?}");
}

#[tokio::test]
async fn remote_endpoints_are_refused_without_allow_remote() {
    let error = SystemOneProvider::new(config("https://laya.example.com")).unwrap_err();
    assert!(matches!(error, DecisionError::RemoteNotAllowed(_)));
    let remote = SystemOneConfig {
        allow_remote: true,
        ..config("https://laya.example.com")
    };
    assert!(SystemOneProvider::new(remote).is_ok());
}

#[tokio::test]
async fn the_connection_probe_reports_answer_and_errors() {
    let (url, seen) = serve(Reply::Yes).await;
    let provider = SystemOneProvider::new(config(&url)).unwrap();
    let report = probe(&provider, &CancellationToken::new()).await;
    assert!(report.ok, "{report:?}");
    assert_eq!(report.answer.as_deref(), Some("yes"));
    let body = seen.lock().unwrap().bodies[0].clone();
    assert!(body["questions"]["connection_probe"]["instructions"].is_string());
    let down = SystemOneProvider::new(config("http://127.0.0.1:9")).unwrap();
    let report = probe(&down, &CancellationToken::new()).await;
    assert!(!report.ok && report.error.is_some());
}
