//! Who HyperForm is to GitHub Copilot, and which hosts are Copilot's.
//!
//! Copilot only serves tokens issued to an app it trusts, so sign-in uses
//! OpenCode's OAuth app and requests present OpenCode's user agent.

use crate::provider::{authority, split_port};

/// OpenCode's GitHub OAuth app.
pub(crate) const CLIENT_ID: &str = "Ov23li8tweQw6odWQebz";
pub(crate) const API_VERSION: &str = "2026-06-01";
pub(crate) const LABEL: &str = "github-copilot";

const GITHUB_WEB: &str = "https://github.com";
/// GitHub Enterprise serves Copilot at `copilot-api.<domain>`.
const ENTERPRISE_PREFIX: &str = "copilot-api.";

/// A GitHub Copilot API base: `*.githubcopilot.com`, or an Enterprise
/// `copilot-api.<domain>` host.
pub fn is_copilot_url(base_url: &str) -> bool {
    let authority = authority(base_url);
    is_copilot_host(split_port(&authority).0)
}

pub(crate) fn is_copilot_host(host: &str) -> bool {
    is_github_host(host) || enterprise_domain(host).is_some()
}

/// The GitHub web origin that signs users in for a Copilot API base:
/// `https://github.com`, or `https://<domain>` for `copilot-api.<domain>`.
/// `None` when `base_url` is not a Copilot API base.
pub fn copilot_login_url(base_url: &str) -> Option<String> {
    let authority = authority(base_url);
    let host = split_port(&authority).0;
    if is_github_host(host) {
        return Some(GITHUB_WEB.to_string());
    }
    enterprise_domain(host)?;
    authority
        .strip_prefix(ENTERPRISE_PREFIX)
        .map(|domain| format!("https://{domain}"))
}

fn is_github_host(host: &str) -> bool {
    host == "githubcopilot.com" || host.ends_with(".githubcopilot.com")
}

fn enterprise_domain(host: &str) -> Option<&str> {
    host.strip_prefix(ENTERPRISE_PREFIX)
        .filter(|domain| !domain.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_copilot_hosts() {
        for url in [
            "https://api.githubcopilot.com",
            "https://API.GITHUBCOPILOT.COM/",
            "https://api.business.githubcopilot.com/v1",
            "api.githubcopilot.com",
            "https://copilot-api.acme.ghe.com",
            "https://copilot-api.github.example:8443/",
        ] {
            assert!(is_copilot_url(url), "{url}");
        }
        for url in [
            "https://github.com",
            "https://notgithubcopilot.com",
            "https://githubcopilot.com.evil.example",
            "https://copilot-api./v1",
            "https://proxy.example/api.githubcopilot.com",
            "http://127.0.0.1:8080",
            "",
        ] {
            assert!(!is_copilot_url(url), "{url}");
        }
    }

    #[test]
    fn login_urls_point_at_the_github_origin() {
        let cases = [
            ("https://api.githubcopilot.com", Some("https://github.com")),
            (
                "https://api.enterprise.githubcopilot.com/",
                Some("https://github.com"),
            ),
            (
                "https://copilot-api.acme.ghe.com",
                Some("https://acme.ghe.com"),
            ),
            (
                "https://copilot-api.git.corp.example:8443/v1",
                Some("https://git.corp.example:8443"),
            ),
            ("https://openrouter.ai/api/v1", None),
            ("https://copilot-api.", None),
            ("https://copilot-api.:8443", None),
        ];
        for (base, login) in cases {
            assert_eq!(copilot_login_url(base).as_deref(), login, "{base}");
        }
    }
}
