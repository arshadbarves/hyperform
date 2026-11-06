//! `file://` URIs for language servers: percent-encoded absolute paths, and
//! their decoding back into paths. Windows drives use `file:///C:/...`.

use std::path::{Path, PathBuf};

/// The `file://` URI of an absolute path.
pub fn path_to_uri(path: &Path) -> String {
    let text = path.to_string_lossy();
    #[cfg(windows)]
    let text = text.replace('\\', "/");
    let mut uri = String::from("file://");
    if !text.starts_with('/') {
        uri.push('/');
    }
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                uri.push(char::from(byte));
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

/// The path of a `file://` URI (local host only); `None` for other schemes
/// or malformed escapes.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    Some(native(&percent_decode(rest)?))
}

/// A canonical path without the Windows `\\?\` verbatim prefix, which
/// language servers do not understand.
pub(crate) fn plain_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(windows)]
fn native(decoded: &str) -> PathBuf {
    let trimmed = match decoded.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => &decoded[1..],
        _ => decoded,
    };
    PathBuf::from(trimmed.replace('/', "\\"))
}

#[cfg(not(windows))]
fn native(decoded: &str) -> PathBuf {
    PathBuf::from(decoded)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_unusual_paths() {
        let path = Path::new("/tmp/my project/src/lïb#1.rs");
        let uri = path_to_uri(path);
        assert_eq!(uri, "file:///tmp/my%20project/src/l%C3%AFb%231.rs");
        assert_eq!(uri_to_path(&uri).as_deref(), Some(path));
    }

    #[test]
    fn accepts_other_encodings_of_the_same_path() {
        assert_eq!(
            uri_to_path("file://localhost/a/b%3Ac.rs").as_deref(),
            Some(Path::new("/a/b:c.rs"))
        );
        assert_eq!(
            uri_to_path("file:///a/b:c.rs").as_deref(),
            Some(Path::new("/a/b:c.rs"))
        );
    }

    #[test]
    fn rejects_foreign_or_broken_uris() {
        assert_eq!(uri_to_path("untitled:Untitled-1"), None);
        assert_eq!(uri_to_path("file://server/share/a.rs"), None);
        assert_eq!(uri_to_path("file:///a/%zz.rs"), None);
        assert_eq!(uri_to_path("file:///a/%C3"), None);
    }
}
