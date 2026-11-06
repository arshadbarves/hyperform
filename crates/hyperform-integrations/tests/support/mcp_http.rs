//! An axum stand-in for a streamable HTTP MCP server: session ids, JSON and
//! SSE replies (sent in small chunks), 202 for notifications and responses,
//! 404 for unknown sessions, 405 for the listening GET. Every request is
//! recorded.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct Recorded {
    pub http_method: String,
    pub session: Option<String>,
    pub version: Option<String>,
    pub accept: Option<String>,
    pub body: Value,
}

#[derive(Default)]
struct Shared {
    sessions: Mutex<u32>,
    current: Mutex<Option<String>>,
    requests: Mutex<Vec<Recorded>>,
}

pub struct FakeHttpMcp {
    pub url: String,
    shared: Arc<Shared>,
    task: tokio::task::JoinHandle<()>,
}

impl FakeHttpMcp {
    pub async fn start() -> Self {
        let shared = Arc::new(Shared::default());
        let app = axum::Router::new()
            .fallback(handle)
            .with_state(Arc::clone(&shared));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { url, shared, task }
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.shared.requests.lock().unwrap().clone()
    }

    /// JSON-RPC methods of the recorded POSTs ("" for responses).
    pub fn rpc_methods(&self) -> Vec<String> {
        self.requests()
            .iter()
            .filter(|r| r.http_method == "POST")
            .map(|r| r.body["method"].as_str().unwrap_or_default().to_string())
            .collect()
    }
}

impl Drop for FakeHttpMcp {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

fn status(code: StatusCode) -> Response {
    Response::builder()
        .status(code)
        .body(Body::empty())
        .unwrap()
}

fn json_reply(id: Value, result: Value) -> Response {
    let body = json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string();
    Response::builder()
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

/// An SSE response carrying `messages`, written 7 bytes at a time.
fn sse(prefix: &str, messages: &[Value]) -> Response {
    let mut text = prefix.to_string();
    for (n, message) in messages.iter().enumerate() {
        text.push_str(&format!("event: message\nid: {n}\ndata: {message}\n\n"));
    }
    let chunks: Vec<Result<Vec<u8>, std::io::Error>> = text
        .into_bytes()
        .chunks(7)
        .map(|chunk| Ok(chunk.to_vec()))
        .collect();
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(futures::stream::iter(chunks)))
        .unwrap()
}

fn result(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn text(text: &str, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

async fn handle(State(shared): State<Arc<Shared>>, request: Request) -> Response {
    let method = request.method().clone();
    let headers = request.headers().clone();
    let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    shared.requests.lock().unwrap().push(Recorded {
        http_method: method.to_string(),
        session: header(&headers, "mcp-session-id"),
        version: header(&headers, "mcp-protocol-version"),
        accept: header(&headers, "accept"),
        body: body.clone(),
    });
    match method {
        Method::POST => post(&shared, &headers, body).await,
        Method::DELETE => {
            *shared.current.lock().unwrap() = None;
            status(StatusCode::OK)
        }
        _ => status(StatusCode::METHOD_NOT_ALLOWED),
    }
}

async fn post(shared: &Shared, headers: &HeaderMap, message: Value) -> Response {
    let accept = header(headers, "accept").unwrap_or_default();
    if !(accept.contains("application/json") && accept.contains("text/event-stream")) {
        return status(StatusCode::NOT_ACCEPTABLE);
    }
    if message["method"] == "initialize" {
        let session = {
            let mut count = shared.sessions.lock().unwrap();
            *count += 1;
            format!("session-{count}")
        };
        *shared.current.lock().unwrap() = Some(session.clone());
        let init = json!({"protocolVersion": "2025-06-18", "capabilities": {"tools": {"listChanged": true}},
                          "serverInfo": {"name": "fake-http", "version": "0.1.0"}});
        let mut response = json_reply(message["id"].clone(), init);
        response
            .headers_mut()
            .insert("mcp-session-id", session.parse().unwrap());
        return response;
    }
    let current = shared.current.lock().unwrap().clone();
    match (header(headers, "mcp-session-id"), current) {
        (Some(sent), Some(current)) if sent == current => {}
        (Some(_), _) => return status(StatusCode::NOT_FOUND),
        (None, _) => return status(StatusCode::BAD_REQUEST),
    }
    if header(headers, "mcp-protocol-version").as_deref() != Some("2025-06-18") {
        return status(StatusCode::BAD_REQUEST);
    }
    let (Some(method), Some(id)) = (message["method"].as_str(), message.get("id").cloned()) else {
        return status(StatusCode::ACCEPTED);
    };
    match method {
        "ping" => json_reply(id, json!({})),
        "tools/list" => tools_list(&id, message["params"]["cursor"].as_str()),
        "tools/call" => call(shared, id, &message["params"]).await,
        _ => {
            let error = json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "method not found"}});
            Response::builder()
                .header("content-type", "application/json")
                .body(Body::from(error.to_string()))
                .unwrap()
        }
    }
}

fn tool(name: &str) -> Value {
    json!({"name": name, "inputSchema": {"type": "object"}})
}

fn tools_list(id: &Value, cursor: Option<&str>) -> Response {
    match cursor {
        None => {
            let log = json!({"jsonrpc": "2.0", "method": "notifications/message",
                             "params": {"level": "info", "data": "listing"}});
            let ping = json!({"jsonrpc": "2.0", "id": "srv-ping", "method": "ping"});
            let page = result(
                id,
                json!({"tools": [tool("echo"), tool("sse_echo")], "nextCursor": "2"}),
            );
            sse(": first page\n\n", &[log, ping, page])
        }
        Some(_) => {
            let names = ["fail", "malformed", "drop", "slow", "notify", "expire"];
            let page = result(id, json!({"tools": names.map(tool)}));
            sse("", &[page])
        }
    }
}

async fn call(shared: &Shared, id: Value, params: &Value) -> Response {
    let echoed = format!(
        "echo: {}",
        params["arguments"]["text"].as_str().unwrap_or("")
    );
    match params["name"].as_str().unwrap_or_default() {
        "echo" => json_reply(id, text(&echoed, false)),
        "sse_echo" => {
            let log = json!({"jsonrpc": "2.0", "method": "notifications/message", "params": {"level": "debug", "data": "x"}});
            sse("", &[log, result(&id, text(&echoed, false))])
        }
        "fail" => json_reply(id, text("remote failure", true)),
        "malformed" => Response::builder()
            .header("content-type", "application/json")
            .body(Body::from("{this is not json"))
            .unwrap(),
        "drop" => sse(": the response never comes\n\n", &[]),
        "slow" => {
            tokio::time::sleep(Duration::from_secs(5)).await;
            json_reply(id, text("slow done", false))
        }
        "notify" => {
            let changed = json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"});
            sse("", &[changed, result(&id, text("notified", false))])
        }
        "expire" => {
            *shared.current.lock().unwrap() = None;
            json_reply(id, text("session expired", false))
        }
        "http_500" => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("internal boom"))
            .unwrap(),
        other => json_reply(id, text(&format!("unknown tool {other}"), true)),
    }
}
