//! `WebSearch` over a configured backend (Brave, Tavily, Exa, or a SearXNG
//! instance), with domain allow/block lists applied to every result.

use std::fmt;

use regex::Regex;
use reqwest::header::ACCEPT;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::client::{WebClient, http_error};
use crate::HostError;

const MAX_RESULTS: usize = 20;
const SNIPPET_CHARS: usize = 500;

#[derive(Clone, Default, PartialEq, Eq)]
pub enum SearchBackend {
    #[default]
    None,
    Brave {
        api_key: String,
    },
    Tavily {
        api_key: String,
    },
    Exa {
        api_key: String,
    },
    Searxng {
        base_url: String,
    },
}

/// Never prints API keys.
impl fmt::Debug for SearchBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Brave { .. } => f.write_str("Brave { api_key: \"***\" }"),
            Self::Tavily { .. } => f.write_str("Tavily { api_key: \"***\" }"),
            Self::Exa { .. } => f.write_str("Exa { api_key: \"***\" }"),
            Self::Searxng { base_url } => f
                .debug_struct("Searxng")
                .field("base_url", base_url)
                .finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

impl WebClient {
    /// Up to `limit` (at most 20) hits. `allowed` keeps only those domains
    /// (and subdomains) when non-empty; `blocked` drops them.
    pub async fn search(
        &self,
        backend: &SearchBackend,
        query: &str,
        allowed: &[String],
        blocked: &[String],
        limit: usize,
        cancel: CancellationToken,
    ) -> Result<Vec<SearchHit>, HostError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(HostError::Invalid("search query is empty".to_string()));
        }
        let limit = limit.clamp(1, MAX_RESULTS);
        let (request, results, snippet) = self.request(backend, query, allowed, blocked, limit)?;
        let body = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(HostError::Cancelled),
            body = send(request) => body?,
        };
        let hits = parse_hits(pointer(&body, results), snippet)
            .filter(|hit| permitted(&hit.url, allowed, blocked))
            .take(limit)
            .collect();
        Ok(hits)
    }

    /// The request plus where its results live and which field holds the
    /// snippet.
    fn request(
        &self,
        backend: &SearchBackend,
        query: &str,
        allowed: &[String],
        blocked: &[String],
        limit: usize,
    ) -> Result<(reqwest::RequestBuilder, &'static str, &'static str), HostError> {
        let count = limit.to_string();
        Ok(match backend {
            SearchBackend::None => {
                return Err(HostError::Invalid(
                    "no web search backend configured".to_string(),
                ));
            }
            SearchBackend::Brave { api_key } => (
                self.open
                    .get(self.endpoint("https://api.search.brave.com", "/res/v1/web/search"))
                    .query(&[("q", query), ("count", count.as_str())])
                    .header("X-Subscription-Token", api_key)
                    .header(ACCEPT, "application/json"),
                "/web/results",
                "description",
            ),
            SearchBackend::Tavily { api_key } => {
                let mut body = json!({ "query": query, "max_results": limit });
                with_domains(&mut body, "include_domains", allowed);
                with_domains(&mut body, "exclude_domains", blocked);
                (
                    self.open
                        .post(self.endpoint("https://api.tavily.com", "/search"))
                        .bearer_auth(api_key)
                        .json(&body),
                    "/results",
                    "content",
                )
            }
            SearchBackend::Exa { api_key } => {
                let mut body = json!({
                    "query": query,
                    "numResults": limit,
                    "contents": { "text": { "maxCharacters": SNIPPET_CHARS } },
                });
                with_domains(&mut body, "includeDomains", allowed);
                with_domains(&mut body, "excludeDomains", blocked);
                (
                    self.open
                        .post(self.endpoint("https://api.exa.ai", "/search"))
                        .header("x-api-key", api_key)
                        .json(&body),
                    "/results",
                    "text",
                )
            }
            SearchBackend::Searxng { base_url } => (
                self.open
                    .get(format!("{}/search", base_url.trim_end_matches('/')))
                    .query(&[("q", query), ("format", "json")])
                    .header(ACCEPT, "application/json"),
                "/results",
                "content",
            ),
        })
    }

    fn endpoint(&self, origin: &str, path: &str) -> String {
        format!("{}{path}", self.search_base.as_deref().unwrap_or(origin))
    }
}

async fn send(request: reqwest::RequestBuilder) -> Result<Value, HostError> {
    let response = request.send().await.map_err(http_error)?;
    let status = response.status();
    let text = response.text().await.map_err(http_error)?;
    if !status.is_success() {
        let detail: String = text.chars().take(300).collect();
        return Err(HostError::Http(format!(
            "search returned HTTP {status}: {detail}"
        )));
    }
    serde_json::from_str(&text)
        .map_err(|e| HostError::Http(format!("search returned invalid JSON: {e}")))
}

fn with_domains(body: &mut Value, key: &str, domains: &[String]) {
    if !domains.is_empty() {
        body[key] = json!(domains);
    }
}

fn pointer<'a>(body: &'a Value, path: &str) -> &'a [Value] {
    body.pointer(path)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn parse_hits<'a>(results: &'a [Value], snippet: &'a str) -> impl Iterator<Item = SearchHit> + 'a {
    results.iter().filter_map(move |result| {
        let url = result.get("url")?.as_str()?.to_string();
        let text = |key: &str| result.get(key).and_then(Value::as_str).unwrap_or("");
        Some(SearchHit {
            title: clean(text("title")),
            snippet: clean(text(snippet)).chars().take(SNIPPET_CHARS).collect(),
            url,
        })
    })
}

/// Strips highlight tags (Brave wraps matches in `<strong>`) and folds
/// whitespace.
fn clean(text: &str) -> String {
    static TAGS: std::sync::OnceLock<Option<Regex>> = std::sync::OnceLock::new();
    let stripped = match TAGS.get_or_init(|| Regex::new(r"<[^>]{1,40}>").ok()) {
        Some(tags) => tags.replace_all(text, ""),
        None => text.into(),
    };
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn permitted(url: &str, allowed: &[String], blocked: &[String]) -> bool {
    let Some(host) = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
    else {
        return false;
    };
    let matches = |domain: &String| {
        let domain = domain.trim().trim_start_matches("*.").to_ascii_lowercase();
        !domain.is_empty() && (host == domain || host.ends_with(&format!(".{domain}")))
    };
    (allowed.is_empty() || allowed.iter().any(matches)) && !blocked.iter().any(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_filters_cover_subdomains() {
        let allowed = vec!["docs.rs".to_string()];
        assert!(permitted("https://docs.rs/tokio", &allowed, &[]));
        assert!(permitted("https://www.docs.rs/x", &allowed, &[]));
        assert!(!permitted("https://notdocs.rs/x", &allowed, &[]));
        let blocked = vec!["spam.com".to_string()];
        assert!(!permitted("https://a.spam.com/", &[], &blocked));
        assert!(permitted("https://example.com/", &[], &blocked));
    }

    #[test]
    fn snippets_lose_tags_and_extra_space() {
        assert_eq!(
            clean("<strong>Rust</strong>  async\n book"),
            "Rust async book"
        );
    }

    #[test]
    fn debug_redacts_keys() {
        let backend = SearchBackend::Brave {
            api_key: "secret".into(),
        };
        assert!(!format!("{backend:?}").contains("secret"));
    }
}
