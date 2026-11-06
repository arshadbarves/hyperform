//! `WebFetch(domain:..)` specifiers and URL host extraction.

use std::fmt;

/// A host and all of its subdomains. `*.example.com` is accepted as another
/// spelling of `example.com`; the spelling is kept for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DomainPattern {
    domain: String,
    wildcard: bool,
}

impl DomainPattern {
    pub fn parse(spec: &str) -> Result<Self, String> {
        let value = spec
            .strip_prefix("domain:")
            .ok_or("WebFetch rules take `domain:<host>`")?
            .trim();
        let (wildcard, host) = match value.strip_prefix("*.") {
            Some(host) => (true, host),
            None => (false, value),
        };
        let domain = host.trim_end_matches('.').to_ascii_lowercase();
        let valid = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
        if domain.is_empty() || !domain.chars().all(valid) {
            return Err(format!("invalid domain `{value}`"));
        }
        Ok(Self { domain, wildcard })
    }

    /// `host` must already be lower-cased (see [`url_host`]).
    pub fn matches_host(&self, host: &str) -> bool {
        host == self.domain
            || host
                .strip_suffix(self.domain.as_str())
                .is_some_and(|rest| rest.ends_with('.'))
    }
}

impl fmt::Display for DomainPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let star = if self.wildcard { "*." } else { "" };
        write!(f, "domain:{star}{}", self.domain)
    }
}

/// The lower-cased host of a URL, with userinfo, port, and trailing dot
/// removed and percent-escapes decoded; `None` for `file:` URLs and URLs
/// without a host. `\` ends the authority, as it does in browsers.
pub(crate) fn url_host(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = match url.split_once("://") {
        Some((scheme, _)) if scheme.eq_ignore_ascii_case("file") => return None,
        Some((scheme, rest)) if is_scheme(scheme) => rest.trim_start_matches(['/', '\\']),
        _ => url.strip_prefix("//").unwrap_or(url),
    };
    let end = rest.find(['/', '\\', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = match host_port.strip_prefix('[') {
        Some(ipv6) => ipv6.split(']').next().unwrap_or(ipv6),
        None => host_port.split(':').next().unwrap_or(host_port),
    };
    let host = percent_decode(host).to_ascii_lowercase();
    let host = host.trim_end_matches('.');
    (!host.is_empty()).then(|| host.to_string())
}

fn is_scheme(text: &str) -> bool {
    text.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .filter(|pair| pair.iter().all(u8::is_ascii_hexdigit))
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_cover_subdomains_only_at_label_boundaries() {
        let rule = DomainPattern::parse("domain:Example.com").unwrap();
        assert!(rule.matches_host("example.com"));
        assert!(rule.matches_host("api.example.com"));
        assert!(!rule.matches_host("badexample.com"));
        assert!(!rule.matches_host("example.com.evil.io"));
        let star = DomainPattern::parse("domain:*.example.com").unwrap();
        assert!(star.matches_host("docs.example.com"));
        assert_eq!(star.to_string(), "domain:*.example.com");
        assert_eq!(rule.to_string(), "domain:example.com");
    }

    #[test]
    fn invalid_domains_are_rejected() {
        for spec in [
            "example.com",
            "domain:",
            "domain:a/b",
            "domain:*",
            "domain:a b",
            "domain:[::1]",
        ] {
            assert!(DomainPattern::parse(spec).is_err(), "{spec}");
        }
    }

    #[test]
    fn hosts_are_extracted_like_browsers_do() {
        let host = |url: &str| url_host(url);
        assert_eq!(
            host("https://Example.COM./path?q#f"),
            Some("example.com".into())
        );
        assert_eq!(
            host("https://user:pass@example.com:8443/"),
            Some("example.com".into())
        );
        assert_eq!(host("https://example.com@evil.io/"), Some("evil.io".into()));
        assert_eq!(host("https://evil.io#@example.com"), Some("evil.io".into()));
        assert_eq!(
            host("https://evil.io\\@example.com"),
            Some("evil.io".into())
        );
        assert_eq!(host("http:///example.com/x"), Some("example.com".into()));
        assert_eq!(host("https://%65vil.io/"), Some("evil.io".into()));
        assert_eq!(host("http://[::1]:8080/"), Some("::1".into()));
        assert_eq!(host("example.com/docs"), Some("example.com".into()));
        assert_eq!(
            host("evil.io/?next=https://example.com"),
            Some("evil.io".into())
        );
        assert_eq!(host("file:///etc/passwd"), None);
        assert_eq!(host("https:///"), None);
    }
}
