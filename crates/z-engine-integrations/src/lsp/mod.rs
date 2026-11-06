//! Language Server Protocol client: presets and routing by file extension,
//! lazily started servers, UTF-16 position conversion, navigation and
//! diagnostics in model coordinates, rename previews, and text rendering.

mod capabilities;
mod client;
mod convert;
mod diagnostics;
mod documents;
pub mod format;
mod manager;
mod ops;
mod position;
mod progress;
mod resolve;
mod routing;
mod spec;
mod types;
mod uri;

pub use client::{INITIALIZE_TIMEOUT, LspClient, REQUEST_TIMEOUT};
pub use documents::SyncOutcome;
pub use manager::{LspManager, LspServerState, LspServerStatus};
pub use ops::DIAGNOSTICS_WAIT;
pub use position::{LspPosition, char_index, from_lsp, line_text, to_lsp, utf16_offset};
pub use spec::{LspServerSpec, default_language_id, merge_specs, presets};
pub use types::{
    CallDirection, CallEdge, CallItem, Diagnostic, FileDiagnostics, FileEdits, Hover, Location,
    Severity, SymbolKind, SymbolNode, TextEditPlan, WorkspaceEditPlan, WorkspaceSymbol,
};
pub use uri::{path_to_uri, uri_to_path};
