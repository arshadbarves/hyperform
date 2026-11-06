//! Loopback helpers for local servers the engine launches (the decision
//! model's sidecar).

use std::net::{Ipv4Addr, TcpListener};

use crate::HostError;

/// A port on 127.0.0.1 that was free a moment ago. Another process may
/// still take it before the server binds; the caller treats that as the
/// server being unavailable.
pub fn free_loopback_port() -> Result<u16, HostError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| HostError::Process(format!("no free loopback port: {error}")))?;
    let port = listener
        .local_addr()
        .map_err(|error| HostError::Process(format!("no free loopback port: {error}")))?
        .port();
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_bindable_port() {
        let port = free_loopback_port().unwrap();
        assert_ne!(port, 0);
        assert!(TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok());
    }
}
