//! The JSON-RPC client over an in-memory duplex: routing by id, server
//! requests, notifications, end of stream, timeouts, cancellation, and
//! malformed replies.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{DuplexStream, ReadHalf, WriteHalf};
use tokio_util::sync::CancellationToken;
use z_engine_integrations::jsonrpc::{FrameReader, RpcNotification, write_frame};
use z_engine_integrations::{CancelStyle, Framing, IntegrationError, RpcClient, RpcOptions};

const LONG: Duration = Duration::from_secs(5);

/// The server end of the connection.
struct Peer {
    reader: FrameReader<ReadHalf<DuplexStream>>,
    writer: WriteHalf<DuplexStream>,
    framing: Framing,
}

impl Peer {
    async fn recv(&mut self) -> Value {
        let frame = self.reader.read_frame().await.unwrap().expect("a frame");
        serde_json::from_slice(&frame).unwrap()
    }

    async fn send(&mut self, message: Value) {
        self.send_raw(message.to_string().as_bytes()).await;
    }

    async fn send_raw(&mut self, body: &[u8]) {
        write_frame(&mut self.writer, self.framing, body)
            .await
            .unwrap();
    }
}

fn connect(framing: Framing, options: RpcOptions) -> (RpcClient, Peer) {
    let (client_end, server_end) = tokio::io::duplex(64 * 1024);
    let (client_read, client_write) = tokio::io::split(client_end);
    let (server_read, server_write) = tokio::io::split(server_end);
    let client = RpcClient::over_io(client_read, client_write, framing, options);
    let peer = Peer {
        reader: FrameReader::new(server_read, framing),
        writer: server_write,
        framing,
    };
    (client, peer)
}

fn spawn_request(
    client: &RpcClient,
    method: &str,
    timeout: Duration,
) -> tokio::task::JoinHandle<Result<Value, IntegrationError>> {
    let (client, method) = (client.clone(), method.to_string());
    tokio::spawn(async move {
        client
            .request(
                &method,
                Some(json!({"m": method})),
                timeout,
                &CancellationToken::new(),
            )
            .await
    })
}

#[tokio::test]
async fn responses_route_by_id_in_any_order() {
    let (client, mut peer) = connect(Framing::ContentLength, RpcOptions::default());
    let first = spawn_request(&client, "first", LONG);
    let a = peer.recv().await;
    let second = spawn_request(&client, "second", LONG);
    let b = peer.recv().await;
    assert_eq!(
        (a["jsonrpc"].as_str(), a["params"]["m"].as_str()),
        (Some("2.0"), Some("first"))
    );
    peer.send(
        json!({"jsonrpc": "2.0", "id": b["id"], "error": {"code": -32000, "message": "busy"}}),
    )
    .await;
    peer.send(json!({"jsonrpc": "2.0", "id": a["id"], "result": {"ok": 1}}))
        .await;
    assert_eq!(first.await.unwrap().unwrap(), json!({"ok": 1}));
    let error = second.await.unwrap().unwrap_err();
    assert!(
        matches!(error, IntegrationError::Rpc { code: -32000, ref message } if message == "busy")
    );
}

#[tokio::test]
async fn server_requests_get_default_replies_and_notifications_reach_the_handler() {
    let seen: Arc<Mutex<Vec<RpcNotification>>> = Arc::default();
    let sink = Arc::clone(&seen);
    let options = RpcOptions {
        cancel_style: CancelStyle::Silent,
        on_notification: Some(Arc::new(move |n| sink.lock().unwrap().push(n))),
    };
    let (_client, mut peer) = connect(Framing::Newline, options);
    peer.send(
        json!({"jsonrpc": "2.0", "method": "window/logMessage", "params": {"message": "hi"}}),
    )
    .await;
    peer.send(json!({"jsonrpc": "2.0", "id": "p1", "method": "ping"}))
        .await;
    assert_eq!(
        peer.recv().await,
        json!({"jsonrpc": "2.0", "id": "p1", "result": {}})
    );
    let items = json!({"items": [{"section": "a"}, {"section": "b"}]});
    peer.send(
        json!({"jsonrpc": "2.0", "id": 2, "method": "workspace/configuration", "params": items}),
    )
    .await;
    assert_eq!(peer.recv().await["result"], json!([null, null]));
    peer.send(
        json!({"jsonrpc": "2.0", "id": 3, "method": "client/registerCapability", "params": {}}),
    )
    .await;
    assert_eq!(peer.recv().await["result"], Value::Null);
    peer.send(json!({"jsonrpc": "2.0", "id": 4, "method": "sampling/createMessage"}))
        .await;
    let refused = peer.recv().await;
    assert_eq!(
        (refused["id"].clone(), refused["error"]["code"].clone()),
        (json!(4), json!(-32601))
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, "window/logMessage");
}

#[tokio::test]
async fn end_of_stream_fails_pending_requests_and_later_ones_are_not_sent() {
    let (client, mut peer) = connect(Framing::ContentLength, RpcOptions::default());
    let pending = spawn_request(&client, "work", LONG);
    peer.recv().await;
    drop(peer);
    let error = pending.await.unwrap().unwrap_err();
    assert!(
        matches!(error, IntegrationError::Disconnected(_)),
        "{error:?}"
    );
    assert!(client.is_closed());
    client.closed().await;
    let later = client
        .request("again", None, LONG, &CancellationToken::new())
        .await;
    assert!(
        matches!(later, Err(IntegrationError::NotSent(_))),
        "{later:?}"
    );
    assert!(matches!(
        client.notify("n", None).await,
        Err(IntegrationError::NotSent(_))
    ));
}

#[tokio::test]
async fn timeouts_send_an_lsp_cancel_request() {
    let options = RpcOptions {
        cancel_style: CancelStyle::Lsp,
        on_notification: None,
    };
    let (client, mut peer) = connect(Framing::ContentLength, options);
    let error = client
        .request(
            "slow",
            None,
            Duration::from_millis(50),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, IntegrationError::Timeout { ref method, .. } if method == "slow"));
    let request = peer.recv().await;
    let cancel = peer.recv().await;
    assert_eq!(cancel["method"], "$/cancelRequest");
    assert_eq!(cancel["params"]["id"], request["id"]);
    // A late answer to the abandoned request is ignored; the client still works.
    peer.send(json!({"jsonrpc": "2.0", "id": request["id"], "result": 1}))
        .await;
    let next = spawn_request(&client, "next", LONG);
    let asked = peer.recv().await;
    peer.send(json!({"jsonrpc": "2.0", "id": asked["id"], "result": "fine"}))
        .await;
    assert_eq!(next.await.unwrap().unwrap(), json!("fine"));
}

#[tokio::test]
async fn cancellation_sends_an_mcp_notice_and_returns_promptly() {
    let options = RpcOptions {
        cancel_style: CancelStyle::Mcp,
        on_notification: None,
    };
    let (client, mut peer) = connect(Framing::Newline, options);
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        trigger.cancel();
    });
    let started = std::time::Instant::now();
    let error = client
        .request("tools/call", None, LONG, &cancel)
        .await
        .unwrap_err();
    assert!(matches!(error, IntegrationError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(2));
    let request = peer.recv().await;
    let notice = peer.recv().await;
    assert_eq!(notice["method"], "notifications/cancelled");
    assert_eq!(notice["params"]["requestId"], request["id"]);
    let already = client.request("x", None, LONG, &cancel).await;
    assert!(matches!(already, Err(IntegrationError::Cancelled)));
}

#[tokio::test]
async fn malformed_replies_fail_their_request_and_garbage_lines_are_skipped() {
    let (client, mut peer) = connect(Framing::Newline, RpcOptions::default());
    let broken = spawn_request(&client, "broken", LONG);
    let asked = peer.recv().await;
    peer.send(json!({"jsonrpc": "2.0", "id": asked["id"]}))
        .await;
    let error = broken.await.unwrap().unwrap_err();
    assert!(matches!(error, IntegrationError::Protocol(_)), "{error:?}");

    let fine = spawn_request(&client, "fine", LONG);
    let asked = peer.recv().await;
    peer.send_raw(b"npm WARN this is not JSON").await;
    peer.send(json!({"jsonrpc": "2.0", "id": asked["id"], "result": [1, 2]}))
        .await;
    assert_eq!(fine.await.unwrap().unwrap(), json!([1, 2]));
}

#[tokio::test]
async fn closing_fails_waiters_with_the_reason() {
    let (client, mut peer) = connect(Framing::ContentLength, RpcOptions::default());
    let pending = spawn_request(&client, "work", LONG);
    peer.recv().await;
    client.close("shutting down").await;
    let error = pending.await.unwrap().unwrap_err();
    assert_eq!(error.to_string(), "server disconnected: shutting down");
    assert_eq!(client.close_reason().as_deref(), Some("shutting down"));
    // The writer closed our end: the peer sees end of stream.
    assert!(peer.reader.read_frame().await.unwrap().is_none());
}
