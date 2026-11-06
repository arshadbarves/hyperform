//! How an [`McpClient`](super::McpClient) reaches its server: a spawned
//! stdio process speaking newline-delimited JSON-RPC, or a streamable HTTP
//! endpoint. Owns the transport-specific parts of the connection's life:
//! the negotiated version header, the HTTP listening stream, and teardown.

use std::sync::Arc;
use std::time::Duration;

use super::http::HttpOutbound;
use super::spec::{McpServerSpec, McpTransport};
use crate::error::IntegrationError;
use crate::jsonrpc::{Framing, Outbound, RpcClient, RpcOptions};
use crate::process::{ServerCommand, ServerProcess, StderrLog, spawn_server};

/// Time a stdio server gets to exit after its input closed.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

pub(crate) enum Link {
    Stdio(Box<tokio::sync::Mutex<ServerProcess>>),
    Http(Arc<HttpOutbound>),
}

/// Starts the server process or prepares the HTTP endpoint, and the
/// JSON-RPC client over it.
pub(crate) fn open(
    spec: &McpServerSpec,
    options: RpcOptions,
    stderr: &StderrLog,
) -> Result<(RpcClient, Link), IntegrationError> {
    match &spec.transport {
        McpTransport::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            let command = ServerCommand {
                program: command,
                args,
                cwd: cwd.as_deref(),
                env,
            };
            let spawned = spawn_server(&command, stderr)?;
            let rpc = RpcClient::over_io(spawned.stdout, spawned.stdin, Framing::Newline, options);
            let process = Box::new(tokio::sync::Mutex::new(spawned.process));
            Ok((rpc, Link::Stdio(process)))
        }
        McpTransport::Http { url, headers } => {
            let http = Arc::new(HttpOutbound::new(url, headers)?);
            let outbound: Arc<dyn Outbound> = Arc::clone(&http) as Arc<dyn Outbound>;
            Ok((
                RpcClient::over_outbound(outbound, options),
                Link::Http(http),
            ))
        }
    }
}

impl Link {
    /// Completes the handshake on the transport: HTTP requests from now on
    /// carry the negotiated version, and the optional listening stream (for
    /// messages the server sends on its own) is opened in the background.
    pub(crate) fn initialized(&self, rpc: &RpcClient, protocol_version: &str) {
        let Self::Http(http) = self else { return };
        http.set_protocol_version(protocol_version);
        let (http, rpc) = (Arc::clone(http), rpc.clone());
        tokio::spawn(async move {
            if let Some(stream) = http.open_listener().await {
                rpc.attach(stream);
            }
        });
    }

    /// Tears the transport down once the JSON-RPC connection is closed: a
    /// stdio server gets its grace period, then its tree is killed; an HTTP
    /// session is ended.
    pub(crate) async fn close(&self) {
        match self {
            Self::Stdio(process) => process.lock().await.shutdown(SHUTDOWN_GRACE).await,
            Self::Http(http) => http.terminate_session().await,
        }
    }
}
