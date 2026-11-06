//! State shared by an [`RpcClient`](super::RpcClient) and whatever reads its
//! transport: requests waiting for a response, routing of incoming
//! messages, and the one-way transition to closed that fails every waiter.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use serde_json::Value;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::client::{Inbound, NotificationHandler, Outbound};
use super::message::{RequestId, RpcMessage, RpcRequest, RpcResponse};
use super::replies::default_reply;
use crate::error::IntegrationError;
use crate::sync::lock;

pub(crate) type Outcome = Result<Value, IntegrationError>;
pub(crate) type Waiter = oneshot::Receiver<Outcome>;
type Pending = HashMap<RequestId, oneshot::Sender<Outcome>>;

#[derive(Default)]
struct State {
    pending: Pending,
    close_reason: Option<String>,
}

pub(crate) struct Dispatcher {
    pub(crate) outbound: Arc<dyn Outbound>,
    state: Mutex<State>,
    next_id: AtomicI64,
    on_notification: Option<NotificationHandler>,
    closed: CancellationToken,
}

impl std::fmt::Debug for Dispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = lock(&self.state);
        f.debug_struct("Dispatcher")
            .field("pending", &state.pending.len())
            .field("close_reason", &state.close_reason)
            .finish_non_exhaustive()
    }
}

impl Dispatcher {
    pub(crate) fn new(
        outbound: Arc<dyn Outbound>,
        on_notification: Option<NotificationHandler>,
    ) -> Arc<Self> {
        Arc::new(Self {
            outbound,
            state: Mutex::new(State::default()),
            next_id: AtomicI64::new(1),
            on_notification,
            closed: CancellationToken::new(),
        })
    }

    pub(crate) fn next_id(&self) -> RequestId {
        RequestId::Number(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Registers a waiter for `id`; fails when the connection is closed.
    pub(crate) fn register(&self, id: RequestId) -> Result<Waiter, IntegrationError> {
        let mut state = lock(&self.state);
        if let Some(reason) = &state.close_reason {
            return Err(IntegrationError::NotSent(reason.clone()));
        }
        let (sender, waiter) = oneshot::channel();
        state.pending.insert(id, sender);
        Ok(waiter)
    }

    /// Stops waiting for `id` (cancelled, timed out, or never sent).
    pub(crate) fn forget(&self, id: &RequestId) {
        lock(&self.state).pending.remove(id);
    }

    /// Fails the waiter of `id`, if it is still waiting.
    pub(crate) fn fail(&self, id: &RequestId, error: IntegrationError) {
        let waiter = lock(&self.state).pending.remove(id);
        if let Some(waiter) = waiter {
            deliver(waiter, Err(error));
        }
    }

    /// Marks the connection closed (first reason wins) and fails every
    /// waiting request with `Disconnected`.
    pub(crate) fn close(&self, reason: &str) {
        let waiters = {
            let mut state = lock(&self.state);
            if state.close_reason.is_some() {
                return;
            }
            state.close_reason = Some(reason.to_string());
            std::mem::take(&mut state.pending)
        };
        for waiter in waiters.into_values() {
            deliver(
                waiter,
                Err(IntegrationError::Disconnected(reason.to_string())),
            );
        }
        self.closed.cancel();
    }

    pub(crate) fn close_reason(&self) -> Option<String> {
        lock(&self.state).close_reason.clone()
    }

    pub(crate) fn closed_token(&self) -> &CancellationToken {
        &self.closed
    }

    /// Routes one incoming message. Never blocks: replies to server requests
    /// are written by a short-lived task so a full pipe cannot stall reading.
    pub(crate) fn dispatch(&self, message: RpcMessage) {
        match message {
            RpcMessage::Response(response) => self.resolve(response),
            RpcMessage::Notification(notification) => match &self.on_notification {
                Some(handler) => handler(notification),
                None => tracing::trace!(method = %notification.method, "notification ignored"),
            },
            RpcMessage::Request(request) => self.reply(request),
            RpcMessage::Invalid {
                id: Some(id),
                reason,
            } => self.fail(&id, IntegrationError::Protocol(reason)),
            RpcMessage::Invalid { id: None, reason } => {
                tracing::warn!(%reason, "ignored an invalid JSON-RPC message");
            }
        }
    }

    fn resolve(&self, response: RpcResponse) {
        let Some(id) = response.id else {
            tracing::warn!(result = ?response.result, "ignored a response without an id");
            return;
        };
        let waiter = lock(&self.state).pending.remove(&id);
        match waiter {
            Some(waiter) => deliver(
                waiter,
                response.result.map_err(|e| IntegrationError::Rpc {
                    code: e.code,
                    message: e.message,
                }),
            ),
            None => tracing::debug!(%id, "response to an unknown or abandoned request"),
        }
    }

    fn reply(&self, request: RpcRequest) {
        let result = default_reply(&request.method, request.params.as_ref());
        let method = request.method;
        let response = RpcResponse {
            id: Some(request.id),
            result,
        }
        .to_value();
        let outbound = Arc::clone(&self.outbound);
        tokio::spawn(async move {
            if let Err(e) = outbound.send(response).await {
                tracing::debug!(%method, error = %e, "could not answer a server request");
            }
        });
    }

    /// Feeds messages received over one exchange (an HTTP response body or
    /// listening stream) to the dispatcher. When `request` is set and the
    /// stream ends before its response, that request fails: with the
    /// stream's error, the last protocol violation seen, or `Disconnected`.
    pub(crate) fn drain(self: &Arc<Self>, mut inbound: Inbound, request: Option<RequestId>) {
        let dispatcher = Arc::clone(self);
        tokio::spawn(async move {
            let mut invalid = None;
            let failure = loop {
                let next = tokio::select! {
                    biased;
                    () = dispatcher.closed.cancelled() => return,
                    next = inbound.next() => next,
                };
                match next {
                    Some(Ok(RpcMessage::Invalid { id: None, reason })) => {
                        tracing::warn!(%reason, "ignored an invalid JSON-RPC message");
                        invalid = Some(reason);
                    }
                    Some(Ok(message)) => dispatcher.dispatch(message),
                    Some(Err(error)) => break Some(error),
                    None => break None,
                }
            };
            match request {
                Some(id) => {
                    let error = failure.unwrap_or_else(|| match invalid {
                        Some(reason) => IntegrationError::Protocol(reason),
                        None => IntegrationError::Disconnected(
                            "the response stream ended before the response".into(),
                        ),
                    });
                    dispatcher.fail(&id, error);
                }
                None => {
                    if let Some(error) = failure {
                        tracing::debug!(%error, "a background message stream failed");
                    }
                }
            }
        });
    }
}

fn deliver(waiter: oneshot::Sender<Outcome>, outcome: Outcome) {
    if waiter.send(outcome).is_err() {
        tracing::trace!("the requester stopped waiting");
    }
}
