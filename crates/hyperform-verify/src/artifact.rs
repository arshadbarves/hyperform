//! Check output kept as evidence: the full captured output written to a
//! uniquely named log file, and the bounded tail shown with the record.

use std::path::{Path, PathBuf};

use crate::VerifyError;

/// Longest check-id fragment used in an artifact file name.
const MAX_NAME_CHARS: usize = 64;

/// Writes `output` to `dir/check-<sanitized id>-<unique>.log`, creating
/// `dir` when missing, and returns the file's path.
pub(crate) async fn write_artifact(
    dir: &Path,
    check_id: &str,
    unique: &str,
    output: &str,
) -> Result<PathBuf, VerifyError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| VerifyError::io(dir, e))?;
    let path = dir.join(format!("check-{}-{unique}.log", file_stem(check_id)));
    tokio::fs::write(&path, output)
        .await
        .map_err(|e| VerifyError::io(&path, e))?;
    Ok(path)
}

/// `web/npm:test` -> `web-npm-test`: only ASCII letters, digits, `.`, `_`
/// and `-` survive, so an id can never leave the artifact directory.
fn file_stem(check_id: &str) -> String {
    let stem: String = check_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .take(MAX_NAME_CHARS)
        .collect();
    let stem = stem.trim_matches(['-', '.']);
    if stem.is_empty() {
        "check".to_string()
    } else {
        stem.to_string()
    }
}

/// The last `max_bytes` of `text`, cut forward to a character boundary.
pub(crate) fn tail(text: &str, max_bytes: usize) -> &str {
    let mut start = text.len().saturating_sub(max_bytes);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_are_file_safe() {
        assert_eq!(file_stem("web/npm:test"), "web-npm-test");
        assert_eq!(file_stem("../../etc/passwd"), "etc-passwd");
        assert_eq!(file_stem("custom:ünï"), "custom--n");
        assert_eq!(file_stem("///"), "check");
        assert_eq!(file_stem(&"x".repeat(100)).len(), MAX_NAME_CHARS);
    }

    #[test]
    fn tails_respect_character_boundaries() {
        assert_eq!(tail("hello", 10), "hello");
        assert_eq!(tail("hello", 3), "llo");
        assert_eq!(tail("aéé", 3), "é");
    }

    #[tokio::test]
    async fn writes_into_a_created_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("session/artifacts");
        let path = write_artifact(&nested, "cargo:test", "01abc", "out\n")
            .await
            .unwrap();
        assert_eq!(path, nested.join("check-cargo-test-01abc.log"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "out\n");
    }
}
