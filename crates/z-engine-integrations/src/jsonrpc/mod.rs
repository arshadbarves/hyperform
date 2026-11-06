//! JSON-RPC 2.0 core shared by MCP and LSP: the message model, stream
//! framing, and a client that correlates responses, answers server
//! requests and forwards notifications.

mod client;
mod dispatch;
mod framing;
mod io;
mod message;
mod replies;

pub use client::{CancelStyle, NotificationHandler, RpcClient, RpcOptions};
pub(crate) use client::{Inbound, Outbound};
pub use framing::{FrameReader, Framing, MAX_FRAME_BYTES, write_frame};
pub use message::{
    INTERNAL_ERROR, INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR, RequestId,
    RpcErrorObject, RpcMessage, RpcNotification, RpcRequest, RpcResponse,
};
pub use replies::default_reply;
