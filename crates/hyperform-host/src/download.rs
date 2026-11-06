//! Large pinned downloads (the decision model's files): each file streams
//! into `<name>.part` next to its destination, resumes from there with an
//! HTTP range request, must match its size and SHA-256, and only then is
//! renamed into place. A file already in place is checked, not fetched.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use crate::HostError;
use crate::blocking::run_blocking;
use crate::digest::hex;
use crate::web::USER_AGENT;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// A stalled transfer fails after this long without a byte; the part stays.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// One file to fetch, and what it must be once fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadSpec {
    pub url: String,
    pub dest: PathBuf,
    pub size: u64,
    /// Lowercase hex.
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct Downloader {
    http: reqwest::Client,
}

impl Downloader {
    pub fn new() -> Result<Self, HostError> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|error| HostError::Http(error.to_string()))?;
        Ok(Self { http })
    }

    /// Fetches `spec` unless it is already in place. `progress` gets the
    /// bytes of this file on disk so far, a resumed part included.
    /// Cancelling keeps the part for the next attempt.
    pub async fn fetch(
        &self,
        spec: &DownloadSpec,
        progress: &(dyn Fn(u64) + Send + Sync),
        cancel: &CancellationToken,
    ) -> Result<(), HostError> {
        if file_matches(&spec.dest, spec.size, &spec.sha256).await? {
            progress(spec.size);
            return Ok(());
        }
        let part = part_path(&spec.dest);
        if let Some(parent) = part.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| HostError::io(parent, error))?;
        }
        let mut offset = file_len(&part).await;
        if offset > spec.size {
            remove_if_present(&part).await?;
            offset = 0;
        }
        if offset < spec.size {
            self.stream(spec, &part, offset, progress, cancel).await?;
        }
        progress(spec.size);
        if sha256_file(&part).await? != spec.sha256.to_ascii_lowercase() {
            remove_if_present(&part).await?;
            return Err(HostError::Invalid(format!(
                "{} does not match its pinned SHA-256; it was deleted, try again",
                spec.url
            )));
        }
        tokio::fs::rename(&part, &spec.dest)
            .await
            .map_err(|error| HostError::io(&spec.dest, error))
    }

    async fn stream(
        &self,
        spec: &DownloadSpec,
        part: &Path,
        offset: u64,
        progress: &(dyn Fn(u64) + Send + Sync),
        cancel: &CancellationToken,
    ) -> Result<(), HostError> {
        let mut request = self.http.get(&spec.url);
        if offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
        }
        let mut response = tokio::select! {
            () = cancel.cancelled() => return Err(HostError::Cancelled),
            response = request.send() => response.map_err(http_error)?,
        };
        let status = response.status();
        let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT && offset > 0;
        if !status.is_success() {
            if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                // The part no longer lines up with the file; start over next time.
                remove_if_present(part).await?;
            }
            return Err(HostError::Http(format!("{} answered {status}", spec.url)));
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(part)
            .await
            .map_err(|error| HostError::io(part, error))?;
        let mut written = if resumed { offset } else { 0 };
        progress(written);
        loop {
            let chunk = tokio::select! {
                () = cancel.cancelled() => None,
                chunk = response.chunk() => Some(chunk.map_err(http_error)?),
            };
            let Some(chunk) = chunk else {
                file.flush()
                    .await
                    .map_err(|error| HostError::io(part, error))?;
                return Err(HostError::Cancelled);
            };
            let Some(bytes) = chunk else { break };
            written += bytes.len() as u64;
            if written > spec.size {
                drop(file);
                remove_if_present(part).await?;
                return Err(HostError::Invalid(format!(
                    "{} is larger than its pinned {} bytes",
                    spec.url, spec.size
                )));
            }
            file.write_all(&bytes)
                .await
                .map_err(|error| HostError::io(part, error))?;
            progress(written);
        }
        file.sync_all()
            .await
            .map_err(|error| HostError::io(part, error))?;
        if written < spec.size {
            return Err(HostError::Http(format!(
                "{} ended after {written} of {} bytes; try again to resume",
                spec.url, spec.size
            )));
        }
        Ok(())
    }
}

/// Bytes of `spec` on disk: its size once in place with that size, else
/// what its part holds. Sizes only; the checksum is checked on use.
pub async fn downloaded_bytes(spec: &DownloadSpec) -> u64 {
    match tokio::fs::metadata(&spec.dest).await {
        Ok(meta) if meta.is_file() && meta.len() == spec.size => spec.size,
        _ => file_len(&part_path(&spec.dest)).await.min(spec.size),
    }
}

/// Deletes a download folder and everything in it; a missing one is fine.
pub async fn remove_download_dir(dir: &Path) -> Result<(), HostError> {
    match tokio::fs::remove_dir_all(dir).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(HostError::io(dir, error)),
    }
}

/// The in-progress file next to `dest`.
pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// `path` exists with exactly `size` bytes and the given SHA-256.
pub async fn file_matches(path: &Path, size: u64, sha256: &str) -> Result<bool, HostError> {
    match tokio::fs::metadata(path).await {
        Ok(meta) if meta.is_file() && meta.len() == size => {
            Ok(sha256_file(path).await? == sha256.to_ascii_lowercase())
        }
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(HostError::io(path, error)),
    }
}

/// Lowercase hex SHA-256 of a file, read off the async executor.
pub async fn sha256_file(path: &Path) -> Result<String, HostError> {
    let path = path.to_path_buf();
    run_blocking(move || {
        let mut file = std::fs::File::open(&path).map_err(|error| HostError::io(&path, error))?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher).map_err(|error| HostError::io(&path, error))?;
        Ok(hex(&hasher.finalize()))
    })
    .await
}

async fn file_len(path: &Path) -> u64 {
    tokio::fs::metadata(path)
        .await
        .map(|meta| meta.len())
        .unwrap_or(0)
}

async fn remove_if_present(path: &Path) -> Result<(), HostError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(HostError::io(path, error)),
    }
}

/// Keeps the cause (DNS, TLS, connect) without repeating the URL twice.
fn http_error(error: reqwest::Error) -> HostError {
    if error.is_timeout() {
        return HostError::Timeout;
    }
    let mut message = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    HostError::Http(message)
}
