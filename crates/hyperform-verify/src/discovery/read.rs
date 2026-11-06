//! Bounded manifest reads: regular files only (never through a symlink),
//! at most the configured size, UTF-8 text without a byte-order mark.

use std::io::Read as _;
use std::path::Path;

#[derive(Debug)]
pub(crate) enum ReadProblem {
    NotAFile,
    TooLarge { limit: u64 },
    NotText,
    Io(std::io::Error),
}

impl std::fmt::Display for ReadProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAFile => f.write_str("skipped: not a regular file"),
            Self::TooLarge { limit } => write!(f, "skipped: larger than {limit} bytes"),
            Self::NotText => f.write_str("skipped: not UTF-8 text"),
            Self::Io(error) => write!(f, "cannot be read: {error}"),
        }
    }
}

pub(crate) fn read_bounded(path: &Path, limit: u64) -> Result<String, ReadProblem> {
    let meta = std::fs::symlink_metadata(path).map_err(ReadProblem::Io)?;
    if !meta.file_type().is_file() {
        return Err(ReadProblem::NotAFile);
    }
    if meta.len() > limit {
        return Err(ReadProblem::TooLarge { limit });
    }
    let file = std::fs::File::open(path).map_err(ReadProblem::Io)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(ReadProblem::Io)?;
    // The file may have grown after it was measured.
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(ReadProblem::TooLarge { limit });
    }
    let text = String::from_utf8(bytes).map_err(|_| ReadProblem::NotText)?;
    Ok(match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_within_the_limit_and_rejects_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("m.json");
        std::fs::write(&file, "\u{feff}{}").unwrap();
        assert_eq!(read_bounded(&file, 16).unwrap(), "{}");
        assert!(matches!(
            read_bounded(&file, 2),
            Err(ReadProblem::TooLarge { limit: 2 })
        ));
        std::fs::write(&file, [0xff, 0xfe]).unwrap();
        assert!(matches!(read_bounded(&file, 16), Err(ReadProblem::NotText)));
        assert!(matches!(
            read_bounded(dir.path(), 16),
            Err(ReadProblem::NotAFile)
        ));
        assert!(matches!(
            read_bounded(&dir.path().join("missing"), 16),
            Err(ReadProblem::Io(_))
        ));
    }
}
