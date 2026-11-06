//! The shared HTTP client behind fetch and search: 30 s timeout, no
//! automatic redirects (fetch decides which to follow), a fixed user agent,
//! and a page cache shared by clones.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::cache::PageCache;
use super::guard::PublicOnlyResolver;
use crate::HostError;

pub const USER_AGENT: &str = "ZEngine/2 (+https://github.com/arshadbarves/z-engine)";
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct WebClient {
    /// Only connects to public addresses.
    pub(super) guarded: reqwest::Client,
    /// For explicitly allowed private targets and configured search APIs.
    pub(super) open: reqwest::Client,
    cache: Arc<Mutex<PageCache>>,
    /// Test override for the hosted search API origins.
    pub(super) search_base: Option<String>,
}

impl WebClient {
    pub fn new() -> Result<Self, HostError> {
        let guarded = builder()
            .dns_resolver(Arc::new(PublicOnlyResolver))
            .build()
            .map_err(http_error)?;
        let open = builder().build().map_err(http_error)?;
        Ok(Self {
            guarded,
            open,
            cache: Arc::default(),
            search_base: None,
        })
    }

    /// Sends Brave, Tavily and Exa requests to `base` (scheme and host)
    /// instead of their public origins; paths are unchanged.
    #[doc(hidden)]
    pub fn with_search_base(mut self, base: impl Into<String>) -> Self {
        self.search_base = Some(base.into().trim_end_matches('/').to_string());
        self
    }

    /// Entries are replaced whole, so a poisoned cache is still consistent.
    pub(super) fn cache(&self) -> MutexGuard<'_, PageCache> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(USER_AGENT)
}

/// Keeps the cause chain (DNS, TLS, connect) in the message.
pub(super) fn http_error(error: reqwest::Error) -> HostError {
    if error.is_timeout() {
        return HostError::Timeout;
    }
    let mut message = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    HostError::Http(message)
}
