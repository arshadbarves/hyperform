//! One connection to one MCP server: the handshake, capability-gated
//! catalog requests (following `nextCursor`), tool calls, resources and
//! prompts, and list-change tracking.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::notifications::{ChangeFlags, ListChangedCallback, notification_handler};
use super::parse::{self, Page};
use super::spec::McpServerSpec;
use super::transport::{Link, open};
use super::types::{
    CallToolResult, McpListKind, McpPromptInfo, McpResourceInfo, McpToolInfo, PromptMessage,
    ResourceContents, ServerCapabilities, ServerInfo,
};
use crate::error::IntegrationError;
use crate::jsonrpc::{CancelStyle, RpcClient, RpcOptions};
use crate::process::StderrLog;

/// The protocol version this client requests.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_VERSIONS: &[&str] = &[PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];
const MAX_PAGES: usize = 100;

pub struct McpClient {
    name: String,
    rpc: RpcClient,
    link: Link,
    timeout: Duration,
    capabilities: ServerCapabilities,
    server_info: ServerInfo,
    protocol_version: String,
    instructions: Option<String>,
    changes: Arc<ChangeFlags>,
    stderr: StderrLog,
}

impl fmt::Debug for McpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpClient")
            .field("name", &self.name)
            .field("protocol_version", &self.protocol_version)
            .field("capabilities", &self.capabilities)
            .field("closed", &self.rpc.close_reason())
            .finish_non_exhaustive()
    }
}

impl McpClient {
    /// Starts or opens the transport and completes the MCP handshake.
    pub async fn connect(
        spec: &McpServerSpec,
        on_list_changed: Option<ListChangedCallback>,
    ) -> Result<Self, IntegrationError> {
        Self::connect_with_log(spec, on_list_changed, StderrLog::default()).await
    }

    pub(crate) async fn connect_with_log(
        spec: &McpServerSpec,
        on_list_changed: Option<ListChangedCallback>,
        stderr: StderrLog,
    ) -> Result<Self, IntegrationError> {
        let changes = Arc::new(ChangeFlags::default());
        let options = RpcOptions {
            cancel_style: CancelStyle::Mcp,
            on_notification: Some(notification_handler(
                &spec.name,
                Arc::clone(&changes),
                on_list_changed,
            )),
        };
        let (rpc, link) = open(spec, options, &stderr)?;
        let mut client = Self {
            name: spec.name.clone(),
            rpc,
            link,
            timeout: spec.timeout,
            capabilities: ServerCapabilities::default(),
            server_info: ServerInfo::default(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            instructions: None,
            changes,
            stderr,
        };
        match client.handshake().await {
            Ok(()) => Ok(client),
            Err(e) => {
                client.shutdown().await;
                Err(e)
            }
        }
    }

    async fn handshake(&mut self) -> Result<(), IntegrationError> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "z-engine", "version": env!("CARGO_PKG_VERSION")},
        });
        let result = self
            .request("initialize", Some(params), &CancellationToken::new())
            .await?;
        let init = parse::initialize_result(&result)?;
        if !SUPPORTED_VERSIONS.contains(&init.protocol_version.as_str()) {
            return Err(IntegrationError::Unsupported(format!(
                "MCP server `{}` speaks protocol {} (supported: {})",
                self.name,
                init.protocol_version,
                SUPPORTED_VERSIONS.join(", ")
            )));
        }
        self.protocol_version = init.protocol_version;
        self.capabilities = init.capabilities;
        self.server_info = init.server_info;
        self.instructions = init.instructions;
        self.rpc.notify("notifications/initialized", None).await?;
        self.link.initialized(&self.rpc, &self.protocol_version);
        Ok(())
    }

    async fn request(
        &self,
        method: &str,
        params: Option<Value>,
        cancel: &CancellationToken,
    ) -> Result<Value, IntegrationError> {
        self.rpc.request(method, params, self.timeout, cancel).await
    }

    async fn paginate<T>(
        &self,
        method: &str,
        parse_page: fn(&Value) -> Result<Page<T>, IntegrationError>,
    ) -> Result<Vec<T>, IntegrationError> {
        let never = CancellationToken::new();
        let mut items = Vec::new();
        let mut cursor: Option<String> = None;
        let mut seen = HashSet::new();
        for _ in 0..MAX_PAGES {
            let params = cursor.as_ref().map(|c| json!({"cursor": c}));
            let page = parse_page(&self.request(method, params, &never).await?)?;
            items.extend(page.items);
            match page.next_cursor {
                None => return Ok(items),
                Some(next) if !seen.insert(next.clone()) => {
                    return Err(IntegrationError::Protocol(format!(
                        "`{method}` repeated the cursor `{next}`"
                    )));
                }
                Some(next) => cursor = Some(next),
            }
        }
        Err(IntegrationError::Protocol(format!(
            "`{method}` returned more than {MAX_PAGES} pages"
        )))
    }

    /// Lists the server's tools, all pages; empty when it declares none.
    pub async fn list_tools(&self) -> Result<Vec<McpToolInfo>, IntegrationError> {
        self.listing(McpListKind::Tools, "tools/list", parse::tools_page)
            .await
    }

    pub async fn list_resources(&self) -> Result<Vec<McpResourceInfo>, IntegrationError> {
        self.listing(
            McpListKind::Resources,
            "resources/list",
            parse::resources_page,
        )
        .await
    }

    pub async fn list_prompts(&self) -> Result<Vec<McpPromptInfo>, IntegrationError> {
        self.listing(McpListKind::Prompts, "prompts/list", parse::prompts_page)
            .await
    }

    async fn listing<T>(
        &self,
        kind: McpListKind,
        method: &str,
        parse_page: fn(&Value) -> Result<Page<T>, IntegrationError>,
    ) -> Result<Vec<T>, IntegrationError> {
        if !self.declares(kind) {
            return Ok(Vec::new());
        }
        let flag = self.changes.flag(kind);
        flag.store(false, Ordering::SeqCst);
        let result = self.paginate(method, parse_page).await;
        if result.is_err() {
            // The cached list is still stale; let the next refresh retry.
            flag.store(true, Ordering::SeqCst);
        }
        result
    }

    /// Calls a tool. `is_error` results are successes of the call; transport
    /// and protocol failures are errors.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        cancel: &CancellationToken,
    ) -> Result<CallToolResult, IntegrationError> {
        self.require(McpListKind::Tools)?;
        let arguments = match arguments {
            Value::Null => json!({}),
            Value::Object(_) => arguments,
            _ => {
                return Err(IntegrationError::Protocol(
                    "tool arguments must be a JSON object".into(),
                ));
            }
        };
        let params = json!({"name": name, "arguments": arguments});
        let result = self.request("tools/call", Some(params), cancel).await?;
        parse::call_result(&result)
    }

    pub async fn read_resource(
        &self,
        uri: &str,
    ) -> Result<Vec<ResourceContents>, IntegrationError> {
        self.require(McpListKind::Resources)?;
        let params = json!({"uri": uri});
        let result = self
            .request("resources/read", Some(params), &CancellationToken::new())
            .await?;
        parse::resource_contents(&result)
    }

    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: &BTreeMap<String, String>,
    ) -> Result<Vec<PromptMessage>, IntegrationError> {
        self.require(McpListKind::Prompts)?;
        let params = json!({"name": name, "arguments": arguments});
        let result = self
            .request("prompts/get", Some(params), &CancellationToken::new())
            .await?;
        parse::prompt_messages(&result)
    }

    fn declares(&self, kind: McpListKind) -> bool {
        match kind {
            McpListKind::Tools => self.capabilities.tools,
            McpListKind::Resources => self.capabilities.resources,
            McpListKind::Prompts => self.capabilities.prompts,
        }
    }

    fn require(&self, kind: McpListKind) -> Result<(), IntegrationError> {
        if self.declares(kind) {
            return Ok(());
        }
        Err(IntegrationError::Unsupported(format!(
            "MCP server `{}` does not provide {}",
            self.name,
            match kind {
                McpListKind::Tools => "tools",
                McpListKind::Resources => "resources",
                McpListKind::Prompts => "prompts",
            }
        )))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    pub fn server_info(&self) -> &ServerInfo {
        &self.server_info
    }

    pub fn protocol_version(&self) -> &str {
        &self.protocol_version
    }

    /// Usage guidance the server sent with `initialize` (untrusted).
    pub fn instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// Whether the server announced a change since the last listing.
    pub fn is_changed(&self, kind: McpListKind) -> bool {
        self.changes.flag(kind).load(Ordering::SeqCst)
    }

    /// Reads and clears the change flag of `kind`.
    pub fn take_changed(&self, kind: McpListKind) -> bool {
        self.changes.flag(kind).swap(false, Ordering::SeqCst)
    }

    /// The last lines the server wrote to stderr (stdio servers only).
    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr.tail()
    }

    pub fn is_closed(&self) -> bool {
        self.rpc.is_closed()
    }

    pub fn close_reason(&self) -> Option<String> {
        self.rpc.close_reason()
    }

    pub(crate) fn closed_token(&self) -> CancellationToken {
        self.rpc.closed_token()
    }

    /// Closes the connection: a stdio server gets EOF, then its tree is
    /// killed after a grace period; an HTTP session is ended.
    pub async fn shutdown(&self) {
        self.rpc.close("the client shut down").await;
        self.link.close().await;
    }
}
