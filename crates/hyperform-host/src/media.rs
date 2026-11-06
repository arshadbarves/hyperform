//! Images and PDFs for multimodal reads: size-checked base64 payloads and
//! per-page PDF text. `pdf-extract` can panic on malformed files, so it runs
//! on a blocking thread whose panic becomes `HostError::Invalid`.

use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::HostError;
use crate::fs::{image_media_type, is_pdf};

pub const DEFAULT_MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

/// `(media_type, base64)` for a PNG, JPEG, GIF or WebP file. The type comes
/// from the magic bytes, so a misnamed file is still labelled correctly.
pub async fn read_image(path: &Path, max_bytes: u64) -> Result<(String, String), HostError> {
    let bytes = read_bounded(path, max_bytes, "image").await?;
    let media_type = image_media_type(&bytes).ok_or_else(|| {
        HostError::Invalid(format!(
            "{} is not a PNG, JPEG, GIF or WebP image",
            path.display()
        ))
    })?;
    Ok((media_type.to_string(), STANDARD.encode(bytes)))
}

/// The whole PDF as base64 for providers that accept documents.
pub async fn read_pdf_base64(path: &Path, max_bytes: u64) -> Result<String, HostError> {
    let bytes = read_bounded(path, max_bytes, "PDF").await?;
    if !is_pdf(&bytes) {
        return Err(HostError::Invalid(format!(
            "{} is not a PDF",
            path.display()
        )));
    }
    Ok(STANDARD.encode(bytes))
}

/// Text of the first `max_pages` pages, one string per page.
pub async fn pdf_text(path: &Path, max_pages: usize) -> Result<Vec<String>, HostError> {
    let head = read_bounded(path, u64::MAX, "PDF").await?;
    if !is_pdf(&head) {
        return Err(HostError::Invalid(format!(
            "{} is not a PDF",
            path.display()
        )));
    }
    let display = path.display().to_string();
    tokio::task::spawn_blocking(move || extract_pages(&head, max_pages))
        .await
        .map_err(|e| HostError::Invalid(format!("could not read PDF {display}: {e}")))?
        .map_err(|e| HostError::Invalid(format!("could not read PDF {display}: {e}")))
}

fn extract_pages(bytes: &[u8], max_pages: usize) -> Result<Vec<String>, pdf_extract::OutputError> {
    let mut document = pdf_extract::Document::load_mem(bytes)?;
    if document.is_encrypted() {
        // Many PDFs are encrypted with an empty user password.
        document.decrypt("")?;
    }
    let mut pages = Vec::new();
    for number in document.get_pages().into_keys().take(max_pages) {
        let mut text = String::new();
        let mut output = pdf_extract::PlainTextOutput::new(&mut text);
        pdf_extract::output_doc_page(&document, &mut output, number)?;
        pages.push(text);
    }
    Ok(pages)
}

async fn read_bounded(path: &Path, max_bytes: u64, what: &str) -> Result<Vec<u8>, HostError> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|e| HostError::io(path, e))?;
    if !meta.is_file() {
        return Err(HostError::Invalid(format!(
            "{} is not a file",
            path.display()
        )));
    }
    if meta.len() > max_bytes {
        return Err(HostError::Invalid(format!(
            "{what} {} is {} bytes; the limit is {max_bytes}",
            path.display(),
            meta.len()
        )));
    }
    tokio::fs::read(path)
        .await
        .map_err(|e| HostError::io(path, e))
}
