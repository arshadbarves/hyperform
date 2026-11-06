//! One language server process: spawn, `initialize` (up to 60 s), full-text
//! document synchronization from disk with versions, the diagnostics it
//! publishes, and requests with a 10 s deadline.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::capabilities::{initialize_params, provides, server_capabilities, wants_save};
use super::convert;
use super::diagnostics::DiagnosticsStore;
use super::documents::{Documents, SyncOutcome};
use super::progress::ProgressTracker;
use super::resolve::Texts;
use super::spec::LspServerSpec;
use super::types::FileDiagnostics;
use super::uri::path_to_uri;
use crate::error::IntegrationError;
use crate::jsonrpc::{CancelStyle, Framing, NotificationHandler, RpcClient, RpcOptions};
use crate::process::{ServerCommand, ServerProcess, StderrLog, spawn_server};

pub const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(60);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const EXIT_GRACE: Duration = Duration::from_secs(2);
/// Larger files are not sent to a server.
const MAX_SYNC_BYTES: u64 = 8 * 1024 * 1024;

pub struct LspClient {
    name: String,
    root: PathBuf,
    spec: LspServerSpec,
    rpc: RpcClient,
    process: tokio::sync::Mutex<ServerProcess>,
    stderr: StderrLog,
    documents: Documents,
    diagnostics: Arc<DiagnosticsStore>,
    progress: Arc<ProgressTracker>,
    capabilities: Value,
    sync_lock: tokio::sync::Mutex<()>,
}

impl fmt::Debug for LspClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LspClient")
            .field("name", &self.name)
            .field("root", &self.root)
            .field("closed", &self.rpc.close_reason())
            .finish_non_exhaustive()
    }
}

impl LspClient {
    /// Starts `program` (resolved from `spec.command`) in `root` and
    /// completes the handshake.
    pub async fn start(
        spec: &LspServerSpec,
        root: &Path,
        program: &Path,
    ) -> Result<Self, IntegrationError> {
        Self::start_with_log(spec, root, program, StderrLog::default()).await
    }

    /// Like [`LspClient::start`], writing stderr into a log the caller keeps
    /// (so a failed start can still be diagnosed).
    pub(crate) async fn start_with_log(
        spec: &LspServerSpec,
        root: &Path,
        program: &Path,
        stderr: StderrLog,
    ) -> Result<Self, IntegrationError> {
        let program_text = program.to_string_lossy();
        let command = ServerCommand {
            program: &program_text,
            args: &spec.args,
            cwd: Some(root),
            env: &Default::default(),
        };
        let spawned = spawn_server(&command, &stderr)?;
        let diagnostics = Arc::new(DiagnosticsStore::default());
        let progress = Arc::new(ProgressTracker::default());
        let options = RpcOptions {
            cancel_style: CancelStyle::Lsp,
            on_notification: Some(notification_handler(
                &spec.name,
                Arc::clone(&diagnostics),
                Arc::clone(&progress),
            )),
        };
        let rpc = RpcClient::over_io(
            spawned.stdout,
            spawned.stdin,
            Framing::ContentLength,
            options,
        );
        let mut process = spawned.process;
        let capabilities = match handshake(&rpc, root, &spec.name).await {
            Ok(capabilities) => capabilities,
            Err(error) => {
                rpc.close("initialization failed").await;
                process.shutdown(EXIT_GRACE).await;
                return Err(error);
            }
        };
        tracing::info!(server = %spec.name, root = %root.display(), "language server ready");
        Ok(Self {
            name: spec.name.clone(),
            root: root.to_path_buf(),
            spec: spec.clone(),
            rpc,
            process: tokio::sync::Mutex::new(process),
            stderr,
            documents: Documents::default(),
            diagnostics,
            progress,
            capabilities,
            sync_lock: tokio::sync::Mutex::new(()),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The workspace root the server was initialized with.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The server's `initialize` capabilities.
    pub fn capabilities(&self) -> &Value {
        &self.capabilities
    }

    /// Whether the server offers `provider` (e.g. `definitionProvider`).
    pub fn supports(&self, provider: &str) -> bool {
        provides(&self.capabilities, provider)
    }

    pub fn is_closed(&self) -> bool {
        self.rpc.is_closed()
    }

    pub fn close_reason(&self) -> Option<String> {
        self.rpc.close_reason()
    }

    pub fn open_documents(&self) -> usize {
        self.documents.count()
    }

    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr.tail()
    }

    /// The text the server holds for `path`, once synced.
    pub fn document_text(&self, path: &Path) -> Option<Arc<str>> {
        self.documents.text(path)
    }

    pub(crate) fn documents(&self) -> &Documents {
        &self.documents
    }

    /// Makes the server's copy of `path` match the file on disk: `didOpen`
    /// the first time, `didChange` (full text) when it differs, and
    /// `didSave` after either when the server asks for it.
    pub async fn sync(&self, path: &Path) -> Result<SyncOutcome, IntegrationError> {
        let text = read_source(path).await?;
        let _serial = self.sync_lock.lock().await;
        let (outcome, version) = self.documents.update(path, &text);
        let uri = path_to_uri(path);
        match outcome {
            SyncOutcome::Unchanged => return Ok(outcome),
            SyncOutcome::Opened => {
                let extension = path
                    .extension()
                    .map(|e| e.to_string_lossy())
                    .unwrap_or_default();
                let document = json!({
                    "uri": uri,
                    "languageId": self.spec.language_id(&extension),
                    "version": version,
                    "text": text,
                });
                self.rpc
                    .notify(
                        "textDocument/didOpen",
                        Some(json!({"textDocument": document})),
                    )
                    .await?;
            }
            SyncOutcome::Changed => {
                let params = json!({
                    "textDocument": {"uri": uri, "version": version},
                    "contentChanges": [{"text": text}],
                });
                self.rpc
                    .notify("textDocument/didChange", Some(params))
                    .await?;
            }
        }
        if wants_save(&self.capabilities) {
            let params = json!({"textDocument": {"uri": uri}});
            self.rpc
                .notify("textDocument/didSave", Some(params))
                .await?;
        }
        Ok(outcome)
    }

    /// A request with the standard deadline.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, IntegrationError> {
        self.rpc
            .request(
                method,
                Some(params),
                REQUEST_TIMEOUT,
                &CancellationToken::new(),
            )
            .await
    }

    /// Syncs `path`, then waits up to `wait` for diagnostics published after
    /// the sync (not at all when the content was unchanged and diagnostics
    /// already arrived).
    pub async fn diagnostics(
        &self,
        path: &Path,
        wait: Duration,
    ) -> Result<FileDiagnostics, IntegrationError> {
        self.collect_diagnostics(path, wait, false).await
    }

    /// [`LspClient::diagnostics`] that never stalls a caller who only wants
    /// them if they come quickly: no wait while the server reports
    /// work in progress (indexing) or before it has published anything for
    /// `path` (the sync still lets it analyze the file for next time).
    pub async fn diagnostics_if_quick(
        &self,
        path: &Path,
        wait: Duration,
    ) -> Result<FileDiagnostics, IntegrationError> {
        self.collect_diagnostics(path, wait, true).await
    }

    /// Whether the server reports work in progress (e.g. indexing).
    pub fn is_busy(&self) -> bool {
        self.progress.busy()
    }

    async fn collect_diagnostics(
        &self,
        path: &Path,
        wait: Duration,
        only_if_quick: bool,
    ) -> Result<FileDiagnostics, IntegrationError> {
        let before = self.diagnostics.generation(path);
        let outcome = self.sync(path).await?;
        let fresh = if outcome == SyncOutcome::Unchanged && before > 0 {
            true
        } else if only_if_quick && (before == 0 || self.is_busy()) {
            false
        } else {
            self.diagnostics.wait_newer(path, before, wait).await
        };
        let raw = convert::diagnostics(&self.diagnostics.get(path));
        let mut texts = Texts::new(&self.documents);
        texts.load(path).await;
        let mut diagnostics: Vec<_> = raw.iter().map(|d| texts.diagnostic(path, d)).collect();
        diagnostics.sort_by_key(|d| (d.line, d.column, d.severity));
        Ok(FileDiagnostics {
            path: path.to_path_buf(),
            diagnostics,
            fresh,
        })
    }

    /// `shutdown` then `exit`, then the process tree is reaped (killed after
    /// a grace period).
    pub async fn shutdown(&self) {
        if !self.rpc.is_closed() {
            let never = CancellationToken::new();
            if let Err(e) = self
                .rpc
                .request("shutdown", None, SHUTDOWN_TIMEOUT, &never)
                .await
            {
                tracing::debug!(server = %self.name, error = %e, "shutdown request failed");
            }
            if let Err(e) = self.rpc.notify("exit", None).await {
                tracing::debug!(server = %self.name, error = %e, "exit notification failed");
            }
        }
        self.rpc.close("the client shut down").await;
        self.process.lock().await.shutdown(EXIT_GRACE).await;
    }
}

async fn handshake(rpc: &RpcClient, root: &Path, name: &str) -> Result<Value, IntegrationError> {
    let never = CancellationToken::new();
    let result = rpc
        .request(
            "initialize",
            Some(initialize_params(root)),
            INITIALIZE_TIMEOUT,
            &never,
        )
        .await?;
    let capabilities = server_capabilities(&result, name)?;
    rpc.notify("initialized", Some(json!({}))).await?;
    Ok(capabilities)
}

fn notification_handler(
    server: &str,
    diagnostics: Arc<DiagnosticsStore>,
    progress: Arc<ProgressTracker>,
) -> NotificationHandler {
    let server = server.to_string();
    Arc::new(move |notification| match notification.method.as_str() {
        "textDocument/publishDiagnostics" => diagnostics.publish(notification.params.as_ref()),
        "$/progress" => progress.update(notification.params.as_ref()),
        method => tracing::trace!(%server, method, "LSP notification ignored"),
    })
}

/// A UTF-8 source file of reasonable size.
async fn read_source(path: &Path) -> Result<String, IntegrationError> {
    let shown = path.display();
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => IntegrationError::NotFound(shown.to_string()),
            _ => IntegrationError::io(format!("reading {shown}"), e),
        })?;
    if !metadata.is_file() {
        return Err(IntegrationError::NotFound(format!("{shown} is not a file")));
    }
    if metadata.len() > MAX_SYNC_BYTES {
        return Err(IntegrationError::Unsupported(format!(
            "{shown} is larger than {MAX_SYNC_BYTES} bytes"
        )));
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| IntegrationError::io(format!("reading {shown}"), e))?;
    String::from_utf8(bytes)
        .map_err(|_| IntegrationError::Unsupported(format!("{shown} is not UTF-8 text")))
}
