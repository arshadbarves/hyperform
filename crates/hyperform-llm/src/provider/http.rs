//! The HTTP client and header helpers every adapter is built with.

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::error::LlmError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn http_client() -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .use_rustls_tls()
        .build()
        .map_err(|error| LlmError::Config(format!("http client: {error}")))
}

/// A credential header value, hidden from `Debug` output.
pub(crate) fn secret(value: &str) -> Result<HeaderValue, LlmError> {
    let mut header = HeaderValue::from_str(value).map_err(|_| {
        LlmError::Config("api key contains characters not allowed in an HTTP header".into())
    })?;
    header.set_sensitive(true);
    Ok(header)
}

/// Extra headers replace defaults of the same name; values may carry
/// credentials, so all are marked sensitive.
pub(crate) fn apply_extra_headers(
    headers: &mut HeaderMap,
    extra: &[(String, String)],
) -> Result<(), LlmError> {
    let mut parsed = Vec::with_capacity(extra.len());
    for (name, value) in extra {
        let header_name = HeaderName::from_bytes(name.trim().as_bytes())
            .map_err(|_| LlmError::Config(format!("invalid header name {name:?}")))?;
        let mut header_value = HeaderValue::from_str(value.trim())
            .map_err(|_| LlmError::Config(format!("invalid value for header {header_name}")))?;
        header_value.set_sensitive(true);
        parsed.push((header_name, header_value));
    }
    for (name, _) in &parsed {
        headers.remove(name);
    }
    for (name, value) in parsed {
        headers.append(name, value);
    }
    Ok(())
}
