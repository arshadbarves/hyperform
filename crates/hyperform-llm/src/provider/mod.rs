//! Provider configuration, endpoint detection and client construction.

mod build;
mod config;
mod detect;
mod http;

pub use build::build_client;
pub use config::{ANTHROPIC_BASE_URL, ProviderConfig, ProviderKind, ReasoningStyle};
pub(crate) use detect::{authority, split_port};
pub use detect::{catalog_provider, detect_kind, provider_label};
pub(crate) use http::{apply_extra_headers, http_client, secret};
