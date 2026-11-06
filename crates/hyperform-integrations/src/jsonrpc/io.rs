//! Byte-stream transport (a child's stdio, an in-memory duplex): a writer
//! task that owns the stream and always writes whole frames, so a caller
//! that stops waiting can never leave half a frame behind, and the reader
//! loop that feeds decoded messages to the dispatcher until end of stream.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::client::{Inbound, Outbound};
use super::dispatch::Dispatcher;
use super::framing::{FrameReader, Framing};
use super::message::RpcMessage;
use crate::error::IntegrationError;

/// Messages waiting for the writer; senders also wait for the write itself.
const QUEUE: usize = 64;

struct Outgoing {
    frame: Vec<u8>,
    written: oneshot::Sender<Result<(), IntegrationError>>,
}

#[derive(Debug)]
pub(crate) struct IoOutbound {
    queue: mpsc::Sender<Outgoing>,
    framing: Framing,
    stop: CancellationToken,
}

pub(crate) fn spawn_writer<W>(writer: W, framing: Framing) -> (IoOutbound, JoinHandle<()>)
where
    W: AsyncWrite + Send + Unpin + 'static,
{
    let (queue, receiver) = mpsc::channel(QUEUE);
    let stop = CancellationToken::new();
    let task = tokio::spawn(write_loop(writer, receiver, stop.clone()));
    (
        IoOutbound {
            queue,
            framing,
            stop,
        },
        task,
    )
}

async fn write_loop<W>(mut writer: W, mut queue: mpsc::Receiver<Outgoing>, stop: CancellationToken)
where
    W: AsyncWrite + Unpin,
{
    loop {
        let next = tokio::select! {
            biased;
            () = stop.cancelled() => break,
            next = queue.recv() => next,
        };
        let Some(outgoing) = next else { break };
        let result = async {
            writer.write_all(&outgoing.frame).await?;
            writer.flush().await
        }
        .await;
        let failed = result.is_err();
        let result = result
            .map_err(|e| IntegrationError::NotSent(format!("could not write to the server: {e}")));
        if outgoing.written.send(result).is_err() {
            tracing::trace!("the sender stopped waiting for its write");
        }
        if failed {
            break;
        }
    }
    // Closing our end tells a stdio server to exit.
    if let Err(e) = writer.shutdown().await {
        tracing::trace!(error = %e, "closing the server input");
    }
}

#[async_trait]
impl Outbound for IoOutbound {
    async fn send(&self, message: Value) -> Result<Option<Inbound>, IntegrationError> {
        let body = serde_json::to_vec(&message)
            .map_err(|e| IntegrationError::Protocol(format!("could not encode a message: {e}")))?;
        let (written, ack) = oneshot::channel();
        let outgoing = Outgoing {
            frame: self.framing.encode(&body),
            written,
        };
        if self.stop.is_cancelled() || self.queue.send(outgoing).await.is_err() {
            return Err(IntegrationError::NotSent("the connection is closed".into()));
        }
        match ack.await {
            Ok(result) => result.map(|()| None),
            Err(_) => Err(IntegrationError::NotSent(
                "the connection closed before the message was written".into(),
            )),
        }
    }

    async fn close(&self) {
        self.stop.cancel();
    }
}

/// Reads frames until end of stream or until the client closes, then marks
/// the connection closed so every waiting request fails.
pub(crate) async fn read_loop<R>(reader: R, framing: Framing, dispatcher: Arc<Dispatcher>)
where
    R: AsyncRead + Unpin,
{
    let mut frames = FrameReader::new(reader, framing);
    let reason = loop {
        let frame = tokio::select! {
            biased;
            () = dispatcher.closed_token().cancelled() => return,
            frame = frames.read_frame() => frame,
        };
        match frame {
            Ok(Some(bytes)) => {
                for message in RpcMessage::parse_frame(&bytes) {
                    dispatcher.dispatch(message);
                }
            }
            Ok(None) => break "the server closed its output".to_string(),
            Err(e) => break e.to_string(),
        }
    };
    tracing::debug!(%reason, "JSON-RPC connection closed");
    dispatcher.close(&reason);
    dispatcher.outbound.close().await;
}
