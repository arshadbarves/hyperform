//! A scripted local HTTP server: every request, on any path, is recorded
//! and answered with the next queued [`Reply`].

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::response::Response;
use futures::StreamExt;
use serde_json::Value;
use tokio::sync::oneshot;

/// Small writes make the client decode events split across chunks.
const CHUNK_BYTES: usize = 17;

pub enum Reply {
    /// A complete `text/event-stream` body.
    Sse(String),
    /// A non-streaming response.
    Status {
        status: u16,
        headers: Vec<(&'static str, String)>,
        body: String,
    },
    /// Send `prefix`, then keep the body open. `closed` is dropped when the
    /// server lets go of the response, i.e. once the client disconnects.
    Stall {
        prefix: String,
        closed: oneshot::Sender<()>,
    },
    /// Never send response headers.
    Hang,
}

impl Reply {
    pub fn status(status: u16, headers: &[(&'static str, &str)], body: &str) -> Self {
        Self::Status {
            status,
            headers: headers
                .iter()
                .map(|(name, value)| (*name, (*value).to_string()))
                .collect(),
            body: body.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub headers: HeaderMap,
    pub body: Value,
}

#[derive(Default)]
struct Script {
    replies: Mutex<VecDeque<Reply>>,
    requests: Mutex<Vec<Recorded>>,
}

pub struct MockServer {
    pub url: String,
    script: Arc<Script>,
    task: tokio::task::JoinHandle<()>,
}

impl MockServer {
    pub async fn start(replies: Vec<Reply>) -> Self {
        let script = Arc::new(Script {
            replies: Mutex::new(replies.into()),
            requests: Mutex::default(),
        });
        let app = axum::Router::new()
            .fallback(handle)
            .with_state(Arc::clone(&script));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a local port");
        let url = format!("http://{}", listener.local_addr().expect("local address"));
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        Self { url, script, task }
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.script.requests.lock().expect("requests lock").clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn handle(State(script): State<Arc<Script>>, request: Request) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let headers = request.headers().clone();
    let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .expect("request body");
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    script
        .requests
        .lock()
        .expect("requests lock")
        .push(Recorded {
            method,
            path,
            headers,
            body,
        });
    let reply = script.replies.lock().expect("replies lock").pop_front();
    match reply {
        Some(Reply::Sse(text)) => {
            let chunks: Vec<Result<Vec<u8>, std::io::Error>> = text
                .into_bytes()
                .chunks(CHUNK_BYTES)
                .map(|chunk| Ok(chunk.to_vec()))
                .collect();
            event_stream(Body::from_stream(futures::stream::iter(chunks)))
        }
        Some(Reply::Status {
            status,
            headers,
            body,
        }) => {
            let mut response = Response::builder().status(status);
            for (name, value) in headers {
                response = response.header(name, value);
            }
            response.body(Body::from(body)).expect("response")
        }
        Some(Reply::Stall { prefix, closed }) => {
            let stream = futures::stream::once(async move { Ok::<_, std::io::Error>(prefix) })
                .chain(futures::stream::pending())
                .map(move |chunk| {
                    let _held_until_disconnect = &closed;
                    chunk
                });
            event_stream(Body::from_stream(stream))
        }
        Some(Reply::Hang) => futures::future::pending().await,
        None => Response::builder()
            .status(500)
            .body(Body::from("no scripted reply left"))
            .expect("response"),
    }
}

fn event_stream(body: Body) -> Response {
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(body)
        .expect("response")
}
