//! `WebFetch`: http is upgraded to https (except loopback), same-site
//! redirects are followed up to five times, a cross-host redirect is
//! reported instead of followed, private addresses are refused unless
//! allowed, and bodies are capped and converted to readable text.

use reqwest::Url;
use reqwest::header::{CONTENT_TYPE, LOCATION};
use tokio_util::sync::CancellationToken;

use super::client::{WebClient, http_error};
use super::convert::to_content;
use super::guard::{check_public, is_loopback_host};
use crate::HostError;

pub const DEFAULT_FETCH_MAX_BYTES: usize = 5 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchOptions {
    /// Body bytes read before the page is marked truncated.
    pub max_bytes: usize,
    /// Allow loopback, private and link-local targets.
    pub allow_private_network: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_FETCH_MAX_BYTES,
            allow_private_network: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedPage {
    /// The requested URL after the https upgrade.
    pub url: String,
    /// Where the content came from after same-site redirects.
    pub final_url: String,
    pub status: u16,
    pub content_type: String,
    /// Markdown for HTML, pretty JSON for JSON, raw text otherwise.
    pub content: String,
    /// A cross-host redirect target that was not followed.
    pub redirect: Option<String>,
    pub from_cache: bool,
    /// The body exceeded `max_bytes`.
    pub truncated: bool,
}

impl WebClient {
    pub async fn fetch(
        &self,
        url: &str,
        opts: &FetchOptions,
        cancel: CancellationToken,
    ) -> Result<FetchedPage, HostError> {
        let requested = prepare(url)?;
        let key = requested.to_string();
        if let Some(page) = self.cache().get(&key, opts) {
            return Ok(page);
        }
        let page = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(HostError::Cancelled),
            page = self.fetch_live(&requested, opts) => page?,
        };
        if (200..300).contains(&page.status) || page.redirect.is_some() {
            self.cache().put(key, &page, opts);
        }
        Ok(page)
    }

    async fn fetch_live(
        &self,
        requested: &Url,
        opts: &FetchOptions,
    ) -> Result<FetchedPage, HostError> {
        let client = if opts.allow_private_network {
            &self.open
        } else {
            &self.guarded
        };
        let mut current = requested.clone();
        for _ in 0..=MAX_REDIRECTS {
            if !opts.allow_private_network {
                check_public(&current).await?;
            }
            let response = client
                .get(current.clone())
                .send()
                .await
                .map_err(http_error)?;
            let status = response.status().as_u16();
            if let Some(next) = redirect_target(&current, &response)? {
                if same_site(&current, &next) {
                    current = upgrade(next);
                    continue;
                }
                return Ok(FetchedPage {
                    url: requested.to_string(),
                    final_url: current.to_string(),
                    status,
                    content_type: String::new(),
                    content: String::new(),
                    redirect: Some(next.to_string()),
                    from_cache: false,
                    truncated: false,
                });
            }
            return read_page(requested, current, response, opts.max_bytes).await;
        }
        Err(HostError::Http(format!(
            "gave up after {MAX_REDIRECTS} redirects"
        )))
    }
}

fn prepare(url: &str) -> Result<Url, HostError> {
    let parsed =
        Url::parse(url.trim()).map_err(|e| HostError::Invalid(format!("bad URL `{url}`: {e}")))?;
    match parsed.scheme() {
        "http" | "https" => Ok(upgrade(parsed)),
        other => Err(HostError::Invalid(format!(
            "unsupported URL scheme `{other}`"
        ))),
    }
}

pub(super) fn upgrade(mut url: Url) -> Url {
    if url.scheme() == "http" && !is_loopback_host(&url) && url.set_scheme("https").is_err() {
        tracing::debug!(%url, "could not upgrade to https");
    }
    url
}

fn redirect_target(current: &Url, response: &reqwest::Response) -> Result<Option<Url>, HostError> {
    if !response.status().is_redirection() {
        return Ok(None);
    }
    let Some(location) = response.headers().get(LOCATION) else {
        return Ok(None);
    };
    let location = location
        .to_str()
        .map_err(|_| HostError::Http("redirect location is not valid text".to_string()))?;
    current
        .join(location)
        .map(Some)
        .map_err(|e| HostError::Http(format!("bad redirect location `{location}`: {e}")))
}

/// Same host (ignoring a `www.` prefix) over http(s).
pub(super) fn same_site(from: &Url, to: &Url) -> bool {
    let bare = |url: &Url| {
        url.host_str().map(|host| {
            host.strip_prefix("www.")
                .unwrap_or(host)
                .to_ascii_lowercase()
        })
    };
    matches!(to.scheme(), "http" | "https") && bare(from).is_some() && bare(from) == bare(to)
}

async fn read_page(
    requested: &Url,
    current: Url,
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<FetchedPage, HostError> {
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let mut body = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = response.chunk().await.map_err(http_error)? {
        let room = max_bytes.saturating_sub(body.len());
        if chunk.len() > room {
            body.extend_from_slice(&chunk[..room]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
    }
    Ok(FetchedPage {
        url: requested.to_string(),
        final_url: current.to_string(),
        status,
        content: to_content(&content_type, &body),
        content_type,
        redirect: None,
        from_cache: false,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn upgrades_http_except_loopback() {
        assert_eq!(
            upgrade(url("http://example.com/a")).as_str(),
            "https://example.com/a"
        );
        assert_eq!(
            upgrade(url("http://localhost:8080/")).as_str(),
            "http://localhost:8080/"
        );
        assert_eq!(
            upgrade(url("http://127.0.0.1:9/x")).as_str(),
            "http://127.0.0.1:9/x"
        );
        assert!(prepare("ftp://example.com").is_err());
    }

    #[test]
    fn same_site_ignores_www_and_scheme() {
        assert!(same_site(
            &url("https://example.com/"),
            &url("https://www.example.com/x")
        ));
        assert!(same_site(
            &url("http://example.com/"),
            &url("https://example.com/")
        ));
        assert!(!same_site(
            &url("https://example.com/"),
            &url("https://evil.com/")
        ));
        assert!(!same_site(
            &url("https://example.com/"),
            &url("ftp://example.com/")
        ));
    }
}
