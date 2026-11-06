//! The Seatbelt (SBPL) profile for `sandbox-exec -p`: allow everything,
//! then deny file writes outside the writable paths, and optionally deny
//! the network except localhost (DNS goes through a system socket, so
//! name lookups fail too).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Always writable: shared temp directories and the device files commands
/// use for output and terminals.
const FIXED_SUBPATHS: &[&str] = &["/private/tmp", "/private/var/folders", "/dev/fd"];
const FIXED_LITERALS: &[&str] = &[
    "/dev/null",
    "/dev/zero",
    "/dev/stdout",
    "/dev/stderr",
    "/dev/dtracehelper",
];
const TTY_REGEX: &str = r"^/dev/tty";

/// The profile text; paths should already be canonical because Seatbelt
/// matches resolved paths (`/tmp` is `/private/tmp`). The last matching
/// rule wins, so `read_only` carve-outs follow the writable allowances.
pub(crate) fn profile(writable: &[PathBuf], read_only: &[PathBuf], allow_network: bool) -> String {
    let mut text = String::from("(version 1)\n(allow default)\n(deny file-write*)\n");
    text.push_str("(allow file-write*\n");
    let tmpdir = std::env::var_os("TMPDIR")
        .and_then(|dir| std::fs::canonicalize(dir).ok())
        .into_iter();
    let subpaths = FIXED_SUBPATHS
        .iter()
        .map(PathBuf::from)
        .chain(tmpdir)
        .chain(writable.iter().cloned());
    for path in subpaths {
        let _ = writeln!(text, "  (subpath {})", quote(&path));
    }
    for literal in FIXED_LITERALS {
        let _ = writeln!(text, "  (literal {})", quote(Path::new(literal)));
    }
    let _ = writeln!(text, "  (regex #\"{TTY_REGEX}\"))");
    if !read_only.is_empty() {
        text.push_str("(deny file-write*\n");
        for path in read_only {
            let _ = writeln!(text, "  (subpath {})", quote(path));
        }
        text.push_str(")\n");
    }
    if !allow_network {
        // `(allow network* (local ip "localhost:*"))` would also match
        // outbound sockets and reopen the whole network.
        text.push_str("(deny network*)\n");
        text.push_str("(allow network-bind (local ip \"localhost:*\"))\n");
        text.push_str("(allow network-inbound (local ip \"localhost:*\"))\n");
        text.push_str("(allow network-outbound (remote ip \"localhost:*\"))\n");
    }
    text
}

/// An SBPL string literal.
fn quote(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for ch in raw.chars() {
        if matches!(ch, '"' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_allows_writable_paths_and_denies_the_network() {
        let text = profile(
            &[PathBuf::from("/work/my \"proj\"")],
            &[PathBuf::from("/work/p/.git/hooks")],
            false,
        );
        let carve_out = text.find("(deny file-write*\n  (subpath \"/work/p/.git/hooks\")");
        assert!(carve_out > text.find("(allow file-write*"), "{text}");
        assert!(text.starts_with("(version 1)\n(allow default)\n(deny file-write*)\n"));
        assert!(text.contains(r#"(subpath "/work/my \"proj\"")"#));
        assert!(text.contains(r#"(subpath "/private/tmp")"#));
        assert!(text.contains(r#"(literal "/dev/null")"#));
        assert!(text.contains(r#"(regex #"^/dev/tty"))"#));
        assert!(text.contains("(deny network*)"));
        assert!(text.contains(r#"(allow network-outbound (remote ip "localhost:*"))"#));
        assert!(!text.contains("(allow network*"));
        assert_eq!(text.matches('(').count(), text.matches(')').count());
    }

    #[test]
    fn network_rules_are_omitted_when_allowed() {
        let text = profile(&[], &[], true);
        assert!(!text.contains("network"));
        assert_eq!(text.matches("(deny file-write*").count(), 1);
    }

    #[test]
    fn quoting_escapes_backslashes_and_quotes() {
        assert_eq!(quote(Path::new(r#"/a\b"c"#)), r#""/a\\b\"c""#);
    }
}
