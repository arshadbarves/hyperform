//! Atomic writes, bounded text reads, and content sniffing.

use std::ffi::OsString;

use z_engine_host::{FileKind, HostError, atomic_write, atomic_write_sync, read_text, sniff};

#[tokio::test]
async fn atomic_write_creates_parents_replaces_content_and_leaves_no_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a/b/c.txt");
    atomic_write(&path, b"one").await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
    atomic_write(&path, b"two").await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
    let names: Vec<OsString> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![OsString::from("c.txt")]);
}

#[cfg(unix)]
#[test]
fn atomic_write_sync_preserves_mode_and_writes_through_symlinks() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("run.sh");
    std::fs::write(&script, "old").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    atomic_write_sync(&script, b"#!/bin/sh\necho hi\n").unwrap();
    let mode = std::fs::metadata(&script).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);

    let link = dir.path().join("link.sh");
    std::os::unix::fs::symlink(&script, &link).unwrap();
    atomic_write_sync(&link, b"through the link").unwrap();
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::read_to_string(&script).unwrap(),
        "through the link"
    );
}

#[tokio::test]
async fn read_text_reports_truncation_lossiness_and_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.txt");
    std::fs::write(&path, "héllo world").unwrap();

    let full = read_text(&path, 1024).await.unwrap();
    assert_eq!(full.content, "héllo world");
    assert!(!full.truncated && !full.lossy);
    assert_eq!(full.len, 12);

    // The cut at byte 2 lands inside "é": back off to the boundary.
    let cut = read_text(&path, 2).await.unwrap();
    assert_eq!(cut.content, "h");
    assert!(cut.truncated && !cut.lossy);

    let bad = dir.path().join("bad.txt");
    std::fs::write(&bad, [b'o', b'k', 0xFF, b'!']).unwrap();
    let lossy = read_text(&bad, 1024).await.unwrap();
    assert!(lossy.lossy);
    assert_eq!(lossy.content, "ok\u{FFFD}!");
}

#[tokio::test]
async fn read_text_rejects_directories_and_reports_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        read_text(dir.path(), 10).await,
        Err(HostError::Invalid(_))
    ));
    let missing = read_text(&dir.path().join("nope.txt"), 10)
        .await
        .unwrap_err();
    assert!(missing.is_not_found(), "{missing}");
}

#[tokio::test]
async fn sniff_classifies_by_magic_extension_and_content() {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, bytes: &[u8]| {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    };
    let png = write("shot.dat", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
    let jpeg = write("photo.png", &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]);
    let pdf = write("paper.bin", b"%PDF-1.7\n");
    let notebook = write("analysis.ipynb", b"{\"cells\": []}");
    let text = write("notes.md", b"# hello\n");
    let binary = write("blob.bin", b"abc\0def");

    let image = |media_type: &str| FileKind::Image {
        media_type: media_type.to_string(),
    };
    assert_eq!(sniff(&png).await.unwrap(), image("image/png"));
    assert_eq!(sniff(&jpeg).await.unwrap(), image("image/jpeg"));
    assert_eq!(sniff(&pdf).await.unwrap(), FileKind::Pdf);
    assert_eq!(sniff(&notebook).await.unwrap(), FileKind::Notebook);
    assert_eq!(sniff(&text).await.unwrap(), FileKind::Text);
    assert_eq!(sniff(&binary).await.unwrap(), FileKind::Binary);
    assert!(sniff(dir.path()).await.is_err());
}
