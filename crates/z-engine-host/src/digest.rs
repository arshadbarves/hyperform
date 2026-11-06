//! Hex rendering of SHA-256 digests used for stable short identifiers.

use sha2::{Digest, Sha256};

/// Lowercase hex of `bytes`.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The first 16 hex characters (64 bits) of a finished hasher.
pub(crate) fn short_hex(hasher: Sha256) -> String {
    let mut full = hex(&hasher.finalize());
    full.truncate(16);
    full
}

/// The first 16 hex characters of `sha256(bytes)`.
pub(crate) fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    short_hex(hasher)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_digest_is_16_hex_chars() {
        assert_eq!(short_sha256(b"abc"), "ba7816bf8f01cfea");
        assert_eq!(hex(&[0, 255, 16]), "00ff10");
    }
}
