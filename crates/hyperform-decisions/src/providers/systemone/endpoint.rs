//! Endpoint parsing and the loopback rule: decisions stay on this machine
//! unless the user sets `decisions.allow_remote`.

use std::net::IpAddr;

use reqwest::Url;

use crate::error::DecisionError;

const PATH: &str = "v1/systemone";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Endpoint {
    pub url: Url,
    pub loopback: bool,
}

/// `base` is the server root (`http://127.0.0.1:8000`); a trailing
/// `/v1/systemone` is accepted too.
pub(super) fn parse(base: &str, allow_remote: bool) -> Result<Endpoint, DecisionError> {
    let invalid = |reason: &str| DecisionError::InvalidEndpoint {
        endpoint: base.to_string(),
        reason: reason.to_string(),
    };
    let trimmed = base.trim().trim_end_matches('/');
    let root = trimmed
        .strip_suffix(PATH)
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    let url = Url::parse(&format!("{root}/{PATH}")).map_err(|error| invalid(&error.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid("use an http:// or https:// address"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid(
            "put the key in decisions.api_key_env, not the address",
        ));
    }
    let loopback = is_loopback(&url);
    if !loopback && !allow_remote {
        return Err(DecisionError::RemoteNotAllowed(root.to_string()));
    }
    Ok(Endpoint { url, loopback })
}

pub(super) fn is_loopback(url: &Url) -> bool {
    match url.host_str() {
        Some(host) => {
            let bare = host.trim_start_matches('[').trim_end_matches(']');
            host.eq_ignore_ascii_case("localhost")
                || bare.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_addresses_are_accepted() {
        for base in [
            "http://127.0.0.1:8000",
            "http://localhost:8000/",
            "http://[::1]:9000/v1/systemone",
            "http://127.0.0.2:1",
        ] {
            let endpoint = parse(base, false).unwrap();
            assert!(endpoint.loopback, "{base}");
            assert!(endpoint.url.path().ends_with("/v1/systemone"), "{base}");
        }
    }

    #[test]
    fn remote_addresses_need_allow_remote() {
        let error = parse("https://decide.example.com", false).unwrap_err();
        assert!(matches!(error, DecisionError::RemoteNotAllowed(_)));
        assert!(matches!(
            parse("http://192.168.1.4:8000", false),
            Err(DecisionError::RemoteNotAllowed(_))
        ));
        let endpoint = parse("https://decide.example.com/api", true).unwrap();
        assert_eq!(
            endpoint.url.as_str(),
            "https://decide.example.com/api/v1/systemone"
        );
        assert!(!endpoint.loopback);
    }

    #[test]
    fn broken_addresses_are_rejected() {
        for base in [
            "",
            "ftp://127.0.0.1",
            "not a url",
            "http://user:pw@127.0.0.1",
        ] {
            assert!(
                matches!(
                    parse(base, true),
                    Err(DecisionError::InvalidEndpoint { .. })
                ),
                "{base}"
            );
        }
    }
}
