//! [`RpcClient`]: requests with a deadline and a cancellation token,
//! notifications, server->client request handling, and one client over two
//! kinds of transport: byte streams (a child's stdio) and message
//! transports (MCP streamable HTTP, where every message is a POST).

use std::fmt;
use std::sync::{Arc, Weak};
use std::time::Duration;

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::dispatch::{Dispatcher, Outcome, Waiter};
use super::framing::Framing;
use super::io::{read_loop, spawn_writer};
use super::message::{RequestId, RpcMessage, RpcNotification, RpcRequest};
use crate::error::IntegrationError;

/// Receives server notifications on the reading task: keep it quick and
/// non-blocking.
pub type NotificationHandler = Arc<dyn Fn(RpcNotification) + Send + Sync>;

/// Messages received over one exchange of a message transport.
pub(crate) type Inbound = BoxStream<'static, Result<RpcMessage, IntegrationError>>;

/// A message transport.
#[async_trait]
pub(crate) trait Outbound: Send + Sync + fmt::Debug {
    /// Delivers one message. `Ok(Some(_))` carries messages received in reply
    /// over the same exchange (an HTTP response body). `NotSent` means the
    /// peer cannot have seen the message.
    async fn send(&self, message: Value) -> Result<Option<Inbound>, IntegrationError>;
    /// Stops delivering; later sends fail with `NotSent`.
    async fn close(&self);
}

/// How a request abandoned by timeout or cancellation is announced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CancelStyle {
    #[default]
    Silent,
    /// `notifications/cancelled { requestId, reason }` (never for `initialize`).
    Mcp,
    /// `$/cancelRequest { id }`.
    Lsp,
}

#[derive(Clone, Default)]
pub struct RpcOptions {
    pub cancel_style: CancelStyle,
    pub on_notification: Option<NotificationHandler>,
}

impl fmt::Debug for RpcOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RpcOptions")
            .field("cancel_style", &self.cancel_style)
            .field("on_notification", &self.on_notification.is_some())
            .finish()
    }
}

/// Aborts the transport tasks and closes the connection once the last
/// clone of the client is dropped.
#[derive(Debug)]
struct TaskGuard {
    tasks: Vec<JoinHandle<()>>,
    dispatcher: Weak<Dispatcher>,
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        if let Some(dispatcher) = self.dispatcher.upgrade() {
            dispatcher.close("the client was dropped");
        }
    }
}

/// A JSON-RPC 2.0 client. Cheap to clone; clones share one connection.
#[derive(Debug, Clone)]
pub struct RpcClient {
    dispatcher: Arc<Dispatcher>,
    cancel_style: CancelStyle,
    _guard: Arc<TaskGuard>,
}

impl RpcClient {
    /// A client over a split byte stream, e.g. a child's stdout and stdin.
    /// Spawns the reader and writer tasks; requires a Tokio runtime.
    pub fn over_io<R, W>(reader: R, writer: W, framing: Framing, options: RpcOptions) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let (outbound, writer_task) = spawn_writer(writer, framing);
        let dispatcher = Dispatcher::new(Arc::new(outbound), options.on_notification);
        let reader_task = tokio::spawn(read_loop(reader, framing, Arc::clone(&dispatcher)));
        Self::assemble(
            dispatcher,
            options.cancel_style,
            vec![reader_task, writer_task],
        )
    }

    /// A client over a message transport.
    pub(crate) fn over_outbound(outbound: Arc<dyn Outbound>, options: RpcOptions) -> Self {
        let dispatcher = Dispatcher::new(outbound, options.on_notification);
        Self::assemble(dispatcher, options.cancel_style, Vec::new())
    }

    fn assemble(
        dispatcher: Arc<Dispatcher>,
        cancel_style: CancelStyle,
        tasks: Vec<JoinHandle<()>>,
    ) -> Self {
        let guard = TaskGuard {
            tasks,
            dispatcher: Arc::downgrade(&dispatcher),
        };
        Self {
            dispatcher,
            cancel_style,
            _guard: Arc::new(guard),
        }
    }

    /// Feeds a stream of messages the server sends on its own.
    pub(crate) fn attach(&self, inbound: Inbound) {
        self.dispatcher.drain(inbound, None);
    }

    /// Sends a request and waits for its result. On timeout or cancellation
    /// the server is told to abandon the request (see [`CancelStyle`]); it
    /// may already have acted on it.
    pub async fn request(
        &self,
        method: &str,
        params: Option<Value>,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<Value, IntegrationError> {
        if cancel.is_cancelled() {
            return Err(IntegrationError::Cancelled);
        }
        let id = self.dispatcher.next_id();
        let waiter = self.dispatcher.register(id.clone())?;
        let message = RpcRequest {
            id: id.clone(),
            method: method.to_string(),
            params,
        }
        .to_value();
        let outcome = tokio::select! {
            biased;
            () = cancel.cancelled() => Err(IntegrationError::Cancelled),
            result = tokio::time::timeout(timeout, self.exchange(&id, message, waiter)) => {
                result.unwrap_or_else(|_| Err(IntegrationError::Timeout {
                    method: method.to_string(),
                    after: timeout,
                }))
            }
        };
        let reason = match &outcome {
            Err(IntegrationError::Cancelled) => "cancelled by the client",
            Err(IntegrationError::Timeout { .. }) => "timed out",
            _ => return outcome,
        };
        self.dispatcher.forget(&id);
        self.announce_cancel(&id, method, reason);
        outcome
    }

    async fn exchange(&self, id: &RequestId, message: Value, mut waiter: Waiter) -> Outcome {
        let send = self.dispatcher.outbound.send(message);
        tokio::pin!(send);
        let sent = tokio::select! {
            biased;
            sent = &mut send => sent,
            // Closed (or answered on another stream) before the send returned.
            outcome = &mut waiter => return flatten(outcome),
        };
        match sent {
            Ok(Some(inbound)) => self.dispatcher.drain(inbound, Some(id.clone())),
            Ok(None) => {}
            Err(error) => {
                self.dispatcher.forget(id);
                return Err(error);
            }
        }
        flatten(waiter.await)
    }

    /// Sends a notification.
    pub async fn notify(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), IntegrationError> {
        let message = RpcNotification {
            method: method.to_string(),
            params,
        }
        .to_value();
        send_notification(&self.dispatcher, message).await
    }

    fn announce_cancel(&self, id: &RequestId, method: &str, reason: &str) {
        let notice = match self.cancel_style {
            CancelStyle::Silent => return,
            CancelStyle::Mcp if method == "initialize" => return,
            CancelStyle::Mcp => RpcNotification {
                method: "notifications/cancelled".into(),
                params: Some(json!({"requestId": id, "reason": reason})),
            },
            CancelStyle::Lsp => RpcNotification {
                method: "$/cancelRequest".into(),
                params: Some(json!({"id": id})),
            },
        };
        let dispatcher = Arc::clone(&self.dispatcher);
        let id = id.clone();
        tokio::spawn(async move {
            if let Err(e) = send_notification(&dispatcher, notice.to_value()).await {
                tracing::debug!(%id, error = %e, "could not announce a cancelled request");
            }
        });
    }

    pub fn is_closed(&self) -> bool {
        self.dispatcher.close_reason().is_some()
    }

    /// Why the connection closed, once it has.
    pub fn close_reason(&self) -> Option<String> {
        self.dispatcher.close_reason()
    }

    /// Resolves once the connection is closed, for whatever reason.
    pub async fn closed(&self) {
        self.dispatcher.closed_token().cancelled().await;
    }

    pub(crate) fn closed_token(&self) -> CancellationToken {
        self.dispatcher.closed_token().clone()
    }

    /// Closes the connection: waiting requests fail with `Disconnected`,
    /// later ones with `NotSent`, and the transport stops (stdin closes).
    pub async fn close(&self, reason: &str) {
        self.dispatcher.close(reason);
        self.dispatcher.outbound.close().await;
    }
}

async fn send_notification(
    dispatcher: &Arc<Dispatcher>,
    message: Value,
) -> Result<(), IntegrationError> {
    if let Some(reason) = dispatcher.close_reason() {
        return Err(IntegrationError::NotSent(reason));
    }
    if let Some(inbound) = dispatcher.outbound.send(message).await? {
        dispatcher.drain(inbound, None);
    }
    Ok(())
}

fn flatten(outcome: Result<Outcome, tokio::sync::oneshot::error::RecvError>) -> Outcome {
    outcome.unwrap_or_else(|_| {
        Err(IntegrationError::Disconnected(
            "the connection dropped the request".into(),
        ))
    })
}
