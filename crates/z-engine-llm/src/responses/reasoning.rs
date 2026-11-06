//! Encrypted Responses reasoning, carried in `ContentBlock::Thinking`
//! signatures.
//!
//! The prefix tells it apart from an Anthropic thinking signature, so after
//! a model switch neither API is sent the other's opaque reasoning.

const PREFIX: &str = "responses:";

pub(crate) fn signature(encrypted: &str) -> String {
    format!("{PREFIX}{encrypted}")
}

pub(crate) fn is_responses_signature(signature: &str) -> bool {
    signature.starts_with(PREFIX)
}

/// The encrypted reasoning a Responses signature carries.
pub(super) fn encrypted(signature: &str) -> Option<&str> {
    signature
        .strip_prefix(PREFIX)
        .filter(|encrypted| !encrypted.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_round_trip_and_reject_other_apis() {
        let signed = signature("gAAAA-blob");
        assert!(is_responses_signature(&signed));
        assert_eq!(encrypted(&signed), Some("gAAAA-blob"));
        assert!(!is_responses_signature("EqQBCkYIBRgCKkA"));
        assert_eq!(encrypted("EqQBCkYIBRgCKkA"), None);
        assert_eq!(encrypted(&signature("")), None);
    }
}
