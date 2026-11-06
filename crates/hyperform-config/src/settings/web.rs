//! `[web]`: web search and fetch.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum SearchBackend {
    /// Web search is unavailable.
    #[default]
    #[serde(rename = "none")]
    Disabled,
    Brave,
    Tavily,
    Exa,
    /// A SearXNG instance at `search_url`.
    Searxng,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct WebSettings {
    pub search_backend: SearchBackend,
    /// Base URL of the SearXNG instance.
    pub search_url: Option<String>,
    /// Let the fast model answer `WebFetch`'s question about the page; off
    /// returns the page itself. Pages are always converted to markdown.
    pub fetch_extract: bool,
    /// Allow fetching loopback and private-network addresses.
    pub allow_private_network: bool,
}

impl Default for WebSettings {
    fn default() -> Self {
        Self {
            search_backend: SearchBackend::Disabled,
            search_url: None,
            fetch_extract: true,
            allow_private_network: false,
        }
    }
}
