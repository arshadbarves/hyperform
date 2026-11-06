//! Keeps fetches on the public internet: private, loopback, link-local and
//! other non-routable addresses are refused unless explicitly allowed. The
//! check runs before each request and again inside DNS resolution, so a
//! name that re-resolves to a private address cannot slip through.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use reqwest::Url;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use crate::HostError;

/// The host as an IP literal, if it is one.
pub(super) fn host_ip(url: &Url) -> Option<IpAddr> {
    let host = url.host_str()?;
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .ok()
}

/// `localhost`, `*.localhost`, or a loopback literal.
pub(super) fn is_loopback_host(url: &Url) -> bool {
    match (host_ip(url), url.host_str()) {
        (Some(ip), _) => ip.is_loopback(),
        (None, Some(host)) => host == "localhost" || host.ends_with(".localhost"),
        (None, None) => false,
    }
}

/// Resolves the URL's host and fails with `HostError::Blocked` when any
/// address it maps to is not public.
pub(super) async fn check_public(url: &Url) -> Result<(), HostError> {
    let host = url
        .host_str()
        .ok_or_else(|| HostError::Invalid(format!("{url} has no host")))?;
    if let Some(ip) = host_ip(url) {
        return ensure_public(ip, host);
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| HostError::Http(format!("could not resolve {host}: {e}")))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        ensure_public(addr.ip(), host)?;
    }
    if any {
        Ok(())
    } else {
        Err(HostError::Http(format!(
            "{host} did not resolve to any address"
        )))
    }
}

fn ensure_public(ip: IpAddr, host: &str) -> Result<(), HostError> {
    if is_public(ip) {
        Ok(())
    } else {
        Err(HostError::Blocked(format!(
            "{host} resolves to the private or local address {ip}"
        )))
    }
}

pub(super) fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_public_v4(v4),
            None => is_public_v6(v6),
        },
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // shared address space (CGNAT)
        || (a == 198 && (18..20).contains(&b)) // benchmarking
        || a >= 240)
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let nat64 = segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0];
    if nat64 {
        let [.., c, d] = segments;
        let embedded = Ipv4Addr::new((c >> 8) as u8, c as u8, (d >> 8) as u8, d as u8);
        return is_public_v4(embedded);
    }
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00 // unique local
        || (segments[0] & 0xffc0) == 0xfe80 // link local
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)) // documentation
}

/// DNS for the guarded client: only public addresses are handed to the
/// connector.
#[derive(Debug)]
pub(super) struct PublicOnlyResolver;

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|addr| is_public(addr.ip()))
                .collect();
            if addrs.is_empty() {
                return Err(format!("{host} resolves only to private or local addresses").into());
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_addresses() {
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.0.1",
            "169.254.1.1",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!is_public(private.parse().unwrap()), "{private}");
        }
        for public in ["1.1.1.1", "93.184.216.34", "2606:4700:4700::1111"] {
            assert!(is_public(public.parse().unwrap()), "{public}");
        }
    }

    #[test]
    fn loopback_hosts() {
        assert!(is_loopback_host(
            &Url::parse("http://localhost:3000/").unwrap()
        ));
        assert!(is_loopback_host(&Url::parse("http://[::1]:80/").unwrap()));
        assert!(is_loopback_host(&Url::parse("http://127.0.0.1/").unwrap()));
        assert!(!is_loopback_host(
            &Url::parse("http://example.com/").unwrap()
        ));
    }
}
