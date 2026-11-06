//! Workspace fingerprints stay stable until content changes.

mod support;

use support::{git as run_git, repo_with, set_mtime, write_files};
use z_engine_host::{git_available, workspace_fingerprint};

#[tokio::test]
async fn plain_directories_track_size_and_mtime_of_unignored_files() {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[("a.txt", "one"), (".gitignore", "*.tmp\n"), ("x.tmp", "t")],
    );
    let first = workspace_fingerprint(dir.path()).await.unwrap();
    assert_eq!(first.len(), 16);
    assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(workspace_fingerprint(dir.path()).await.unwrap(), first);

    write_files(dir.path(), &[("x.tmp", "ignored change")]);
    assert_eq!(workspace_fingerprint(dir.path()).await.unwrap(), first);

    write_files(dir.path(), &[("a.txt", "two")]);
    set_mtime(&dir.path().join("a.txt"), 5);
    let edited = workspace_fingerprint(dir.path()).await.unwrap();
    assert_ne!(edited, first);

    write_files(dir.path(), &[("b.txt", "new")]);
    assert_ne!(workspace_fingerprint(dir.path()).await.unwrap(), edited);
}

#[tokio::test]
async fn repositories_track_head_status_and_dirty_files() {
    if !git_available() {
        return;
    }
    let repo = repo_with(&[("a.txt", "one\n"), ("b.txt", "bee\n")]);
    let clean = workspace_fingerprint(repo.path()).await.unwrap();
    assert_eq!(workspace_fingerprint(repo.path()).await.unwrap(), clean);

    write_files(repo.path(), &[("a.txt", "two\n")]);
    set_mtime(&repo.path().join("a.txt"), 1);
    let dirty = workspace_fingerprint(repo.path()).await.unwrap();
    assert_ne!(dirty, clean);

    // Same size, new content: the mtime moves the fingerprint.
    write_files(repo.path(), &[("a.txt", "six\n")]);
    set_mtime(&repo.path().join("a.txt"), 2);
    let dirtier = workspace_fingerprint(repo.path()).await.unwrap();
    assert_ne!(dirtier, dirty);

    write_files(repo.path(), &[("a.txt", "one\n")]);
    assert_eq!(workspace_fingerprint(repo.path()).await.unwrap(), clean);

    write_files(repo.path(), &[("a.txt", "committed\n")]);
    run_git(repo.path(), &["commit", "-qam", "second"]);
    let committed = workspace_fingerprint(repo.path()).await.unwrap();
    assert_ne!(committed, clean);
    assert_eq!(workspace_fingerprint(repo.path()).await.unwrap(), committed);
}

#[tokio::test]
async fn missing_directories_are_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let missing = workspace_fingerprint(&dir.path().join("missing")).await;
    assert!(missing.unwrap_err().is_not_found());
}
