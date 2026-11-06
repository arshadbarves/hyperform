//! Pinned downloads: verified, resumable, never left half in place.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use z_engine_host::{
    DownloadSpec, Downloader, HostError, downloaded_bytes, part_path, remove_download_dir,
};

struct Server {
    body: Vec<u8>,
    hits: AtomicUsize,
    /// The first full response stops after this many bytes.
    cut_first_at: Option<usize>,
}

async fn serve(State(server): State<Arc<Server>>, headers: HeaderMap) -> Response {
    let hit = server.hits.fetch_add(1, Ordering::SeqCst);
    let body = &server.body;
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    if let Some(start) = range.and_then(|range| range.strip_prefix("bytes=")?.strip_suffix('-')) {
        let start: usize = start.parse().unwrap();
        if start >= body.len() {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        }
        return (StatusCode::PARTIAL_CONTENT, body[start..].to_vec()).into_response();
    }
    match server.cut_first_at {
        Some(cut) if hit == 0 => body[..cut].to_vec().into_response(),
        _ => body.clone().into_response(),
    }
}

async fn start(body: Vec<u8>, cut_first_at: Option<usize>) -> (String, Arc<Server>) {
    let server = Arc::new(Server {
        body,
        hits: AtomicUsize::new(0),
        cut_first_at,
    });
    let app = Router::new()
        .fallback(serve)
        .with_state(Arc::clone(&server));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}/model.bin"), server)
}

fn payload() -> Vec<u8> {
    (0..200_000_u32).map(|i| (i % 251) as u8).collect()
}

fn spec(url: &str, dir: &std::path::Path, body: &[u8]) -> DownloadSpec {
    DownloadSpec {
        url: url.into(),
        dest: dir.join("models").join("model.bin"),
        size: body.len() as u64,
        sha256: Sha256::digest(body)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    }
}

#[tokio::test]
async fn downloads_verify_and_land_in_place_once() {
    let dir = tempfile::tempdir().unwrap();
    let body = payload();
    let (url, server) = start(body.clone(), None).await;
    let spec = spec(&url, dir.path(), &body);
    let downloader = Downloader::new().unwrap();
    let seen = AtomicUsize::new(0);
    let progress = |bytes: u64| seen.store(bytes as usize, Ordering::SeqCst);
    let cancel = CancellationToken::new();
    downloader.fetch(&spec, &progress, &cancel).await.unwrap();
    assert_eq!(std::fs::read(&spec.dest).unwrap(), body);
    assert!(!part_path(&spec.dest).exists());
    assert_eq!(seen.load(Ordering::SeqCst), body.len());
    downloader.fetch(&spec, &progress, &cancel).await.unwrap();
    assert_eq!(
        server.hits.load(Ordering::SeqCst),
        1,
        "a verified file is not fetched again"
    );
}

#[tokio::test]
async fn a_cut_transfer_resumes_from_its_part() {
    let dir = tempfile::tempdir().unwrap();
    let body = payload();
    let (url, server) = start(body.clone(), Some(70_000)).await;
    let spec = spec(&url, dir.path(), &body);
    let downloader = Downloader::new().unwrap();
    let cancel = CancellationToken::new();
    let error = downloader.fetch(&spec, &|_| {}, &cancel).await.unwrap_err();
    assert!(error.to_string().contains("ended after 70000"), "{error}");
    assert_eq!(
        std::fs::metadata(part_path(&spec.dest)).unwrap().len(),
        70_000
    );
    assert_eq!(downloaded_bytes(&spec).await, 70_000);
    assert!(!spec.dest.exists());
    downloader.fetch(&spec, &|_| {}, &cancel).await.unwrap();
    assert_eq!(std::fs::read(&spec.dest).unwrap(), body);
    assert_eq!(downloaded_bytes(&spec).await, spec.size);
    assert_eq!(server.hits.load(Ordering::SeqCst), 2);
    remove_download_dir(&dir.path().join("models"))
        .await
        .unwrap();
    assert!(!spec.dest.exists());
    remove_download_dir(&dir.path().join("models"))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_wrong_checksum_deletes_the_part_and_places_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let body = payload();
    let (url, _server) = start(body.clone(), None).await;
    let mut spec = spec(&url, dir.path(), &body);
    spec.sha256 = "0".repeat(64);
    let downloader = Downloader::new().unwrap();
    let cancel = CancellationToken::new();
    let error = downloader.fetch(&spec, &|_| {}, &cancel).await.unwrap_err();
    assert!(matches!(error, HostError::Invalid(_)), "{error}");
    assert!(!spec.dest.exists());
    assert!(!part_path(&spec.dest).exists());
}

#[tokio::test]
async fn a_cancelled_download_stops_without_placing_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let body = payload();
    let (url, _server) = start(body.clone(), None).await;
    let spec = spec(&url, dir.path(), &body);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let downloader = Downloader::new().unwrap();
    let error = downloader.fetch(&spec, &|_| {}, &cancel).await.unwrap_err();
    assert!(matches!(error, HostError::Cancelled), "{error}");
    assert!(!spec.dest.exists());
}
