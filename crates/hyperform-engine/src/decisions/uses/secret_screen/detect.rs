//! Deterministic secret detectors: well-known key formats, private key
//! blocks, passwords in URLs, and high-entropy values assigned to names
//! like `token` or `password`. Assignments they leave unflagged become
//! candidate lines for the decision model.

use std::sync::LazyLock;

use regex::Regex;

/// Longer text is screened up to here.
const SCAN_CHARS: usize = 200_000;
const MIN_VALUE: usize = 8;
const ENTROPY_LEN: usize = 16;
/// Bits per character; hex digests stay under 4.0, keys mix more.
const ENTROPY_BITS: f64 = 3.5;

const FORMATS: &[(&str, &str)] = &[
    (
        "private key",
        r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
    ),
    ("Anthropic API key", r"\bsk-ant-[A-Za-z0-9_-]{20,}"),
    (
        "OpenAI API key",
        r"\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,}",
    ),
    ("AWS access key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
    (
        "GitHub token",
        r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{40,})",
    ),
    ("Slack token", r"\bxox[abprs]-[A-Za-z0-9-]{10,}"),
    ("Stripe key", r"\b[rs]k_(?:live|test)_[A-Za-z0-9]{16,}"),
    ("Google API key", r"\bAIza[0-9A-Za-z_-]{35}"),
    (
        "JSON web token",
        r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
    ),
];

/// Lowercase name fragments that mark a value as a credential.
const SECRET_NAMES: &[&str] = &[
    "pass",
    "pwd",
    "secret",
    "token",
    "apikey",
    "api_key",
    "api-key",
    "accesskey",
    "access_key",
    "access-key",
    "privatekey",
    "private_key",
    "credential",
    "signing_key",
    "auth_key",
];

const PLACEHOLDERS: &[&str] = &[
    "password",
    "changeme",
    "example",
    "placeholder",
    "redacted",
    "undefined",
    "your_",
    "your-",
    "xxxx",
    "...",
    "****",
];

static PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    FORMATS
        .iter()
        .filter_map(|(kind, pattern)| Regex::new(pattern).ok().map(|re| (*kind, re)))
        .collect()
});

static URL_PASSWORD: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\b[a-zA-Z][a-zA-Z0-9+.-]*://[^\s:/@]+:([^\s:/@]{6,})@").ok());

static ASSIGNMENT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"^\s*(?:export\s+)?["']?([A-Za-z_][\w.-]*)["']?\s*[:=]\s*["']?([^\s"'`,;]+)"#).ok()
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Secret {
    pub value: String,
    pub kind: &'static str,
}

/// What screening one text found: secrets, and assignment lines (with
/// their value) for the model to judge.
#[derive(Debug, Default)]
pub(super) struct Scan {
    pub secrets: Vec<Secret>,
    pub candidates: Vec<(String, String)>,
}

pub(super) fn scan(text: &str) -> Scan {
    let text = prefix(text, SCAN_CHARS);
    let mut scan = Scan::default();
    for (kind, pattern) in PATTERNS.iter() {
        for found in pattern.find_iter(text) {
            add(&mut scan.secrets, found.as_str(), kind);
        }
    }
    if let Some(pattern) = URL_PASSWORD.as_ref() {
        for captures in pattern.captures_iter(text) {
            if let Some(value) = captures.get(1).map(|m| m.as_str())
                && !placeholder(value)
            {
                add(&mut scan.secrets, value, "password in a URL");
            }
        }
    }
    for line in text.lines() {
        let Some((name, value)) = assignment(line) else {
            continue;
        };
        if !secret_name(name) || placeholder(value) || covered(&scan.secrets, value) {
            continue;
        }
        if high_entropy(value) {
            add(&mut scan.secrets, value, "credential-like value");
        } else if !scan.candidates.iter().any(|(_, seen)| seen == value) {
            scan.candidates
                .push((line.trim().to_string(), value.to_string()));
        }
    }
    scan
}

fn add(secrets: &mut Vec<Secret>, value: &str, kind: &'static str) {
    if !covered(secrets, value) {
        secrets.push(Secret {
            value: value.to_string(),
            kind,
        });
    }
}

/// Already found, alone or inside a longer match (a key in a key block).
fn covered(secrets: &[Secret], value: &str) -> bool {
    secrets.iter().any(|secret| secret.value.contains(value))
}

fn assignment(line: &str) -> Option<(&str, &str)> {
    let captures = ASSIGNMENT.as_ref()?.captures(line)?;
    Some((captures.get(1)?.as_str(), captures.get(2)?.as_str()))
}

fn secret_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    SECRET_NAMES.iter().any(|fragment| name.contains(fragment))
}

/// Too short, a reference, a type, a member access or an obvious stand-in.
fn placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let mut chars = value.chars();
    let first = chars.next().unwrap_or(' ');
    value.chars().count() < MIN_VALUE
        || value.chars().all(|c| c == first)
        || ['$', '%', '<', '{', '&'].contains(&first)
        || value.contains(['(', ')', '<', '>'])
        || ["process.env", "os.environ", "getenv"]
            .iter()
            .any(|reference| lower.contains(reference))
        || PLACEHOLDERS.iter().any(|stand_in| lower.contains(stand_in))
        || member_access(value)
}

/// `config.password`, `self.token`: names, not values.
fn member_access(value: &str) -> bool {
    value.contains('.')
        && value.split('.').all(|part| {
            part.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

fn high_entropy(value: &str) -> bool {
    value.len() >= ENTROPY_LEN
        && value.chars().any(|c| c.is_ascii_digit())
        && value.chars().any(|c| c.is_ascii_alphabetic())
        && entropy(value) >= ENTROPY_BITS
}

/// Shannon entropy in bits per character.
pub(super) fn entropy(value: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    for c in value.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
    }
    let len = value.chars().count() as f64;
    counts
        .values()
        .map(|count| {
            let p = *count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

fn prefix(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}
