//! Bounded text reads and content-kind sniffing (extension, magic bytes,
//! and a NUL scan of the first 8 KiB).

use std::path::Path;

use tokio::io::AsyncReadExt;

use crate::HostError;

const SNIFF_BYTES: usize = 8 * 1024;

/// A file decoded as UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFile {
    pub content: String,
    /// Invalid UTF-8 was replaced with U+FFFD.
    pub lossy: bool,
    /// Only the first `max_bytes` were read.
    pub truncated: bool,
    /// Size of the file on disk in bytes.
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    Text,
    Image { media_type: String },
    Pdf,
    Notebook,
    Binary,
}

/// Reads at most `max_bytes` of `path` as text. A cut that lands inside a
/// multi-byte character is moved back to the character boundary.
pub async fn read_text(path: &Path, max_bytes: usize) -> Result<TextFile, HostError> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| HostError::io(path, e))?;
    let meta = file.metadata().await.map_err(|e| HostError::io(path, e))?;
    if meta.is_dir() {
        return Err(HostError::Invalid(format!(
            "{} is a directory",
            path.display()
        )));
    }
    let limit = u64::try_from(max_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::new();
    file.take(limit)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| HostError::io(path, e))?;
    let truncated = bytes.len() > max_bytes;
    bytes.truncate(max_bytes);
    let (content, lossy) = decode(bytes, truncated);
    Ok(TextFile {
        content,
        lossy,
        truncated,
        len: meta.len(),
    })
}

/// Classifies `path` by magic bytes, then by extension, then by content.
pub async fn sniff(path: &Path) -> Result<FileKind, HostError> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|e| HostError::io(path, e))?;
    if meta.is_dir() {
        return Err(HostError::Invalid(format!(
            "{} is a directory",
            path.display()
        )));
    }
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| HostError::io(path, e))?;
    let mut head = Vec::with_capacity(SNIFF_BYTES);
    file.take(SNIFF_BYTES as u64)
        .read_to_end(&mut head)
        .await
        .map_err(|e| HostError::io(path, e))?;
    Ok(classify(path, &head))
}

fn classify(path: &Path, head: &[u8]) -> FileKind {
    if let Some(media_type) = image_media_type(head) {
        return FileKind::Image {
            media_type: media_type.to_string(),
        };
    }
    if is_pdf(head) {
        return FileKind::Pdf;
    }
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    if extension.as_deref() == Some("ipynb") {
        return FileKind::Notebook;
    }
    if head.contains(&0) {
        FileKind::Binary
    } else {
        FileKind::Text
    }
}

/// The media type announced by an image's magic bytes (PNG, JPEG, GIF, WebP).
pub(crate) fn image_media_type(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

pub(crate) fn is_pdf(head: &[u8]) -> bool {
    head.starts_with(b"%PDF-")
}

fn decode(bytes: Vec<u8>, truncated: bool) -> (String, bool) {
    match String::from_utf8(bytes) {
        Ok(text) => (text, false),
        Err(e) => {
            let error = e.utf8_error();
            let bytes = e.into_bytes();
            if truncated && error.error_len().is_none() {
                let valid = &bytes[..error.valid_up_to()];
                return (String::from_utf8_lossy(valid).into_owned(), false);
            }
            (String::from_utf8_lossy(&bytes).into_owned(), true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_inside_a_character_is_not_lossy() {
        let bytes = "aé".as_bytes()[..2].to_vec();
        assert_eq!(decode(bytes, true), ("a".to_string(), false));
        assert!(decode(vec![b'a', 0xFF, b'b'], false).1);
    }

    #[test]
    fn classify_prefers_magic_over_extension() {
        let png = b"\x89PNG\r\n\x1a\n rest";
        assert_eq!(
            classify(Path::new("x.bin"), png),
            FileKind::Image {
                media_type: "image/png".into()
            }
        );
        assert_eq!(
            classify(Path::new("pointer.png"), b"version https://git-lfs"),
            FileKind::Text
        );
        assert_eq!(classify(Path::new("n.ipynb"), b"{}"), FileKind::Notebook);
        assert_eq!(classify(Path::new("doc"), b"%PDF-1.4"), FileKind::Pdf);
        assert_eq!(classify(Path::new("a.dat"), b"a\0b"), FileKind::Binary);
    }
}
