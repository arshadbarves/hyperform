//! Read-before-edit tracking and external-modification detection.

mod support;

use support::{canonical, set_mtime};
use z_engine_host::{FileTracker, Freshness};

#[test]
fn external_modification_is_detected_and_own_writes_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f.txt");
    std::fs::write(&path, "v1").unwrap();
    set_mtime(&path, 0);

    let tracker = FileTracker::new();
    assert_eq!(tracker.check_fresh(&path), Err(Freshness::NotRead));
    tracker.record_read(&path).unwrap();
    assert!(tracker.was_read(&path));
    assert_eq!(tracker.check_fresh(&path), Ok(()));
    assert!(tracker.changed_since_read().is_empty());

    std::fs::write(&path, "v2, edited elsewhere").unwrap();
    set_mtime(&path, 10);
    assert_eq!(tracker.check_fresh(&path), Err(Freshness::Modified));
    assert_eq!(tracker.changed_since_read(), vec![canonical(&path)]);

    tracker.record_write(&path).unwrap();
    assert_eq!(tracker.check_fresh(&path), Ok(()));
    assert!(tracker.changed_since_read().is_empty());
}

#[test]
fn touching_without_changing_content_stays_fresh() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("same.txt");
    std::fs::write(&path, "same").unwrap();
    set_mtime(&path, 0);
    let tracker = FileTracker::new();
    tracker.record_read(&path).unwrap();

    std::fs::write(&path, "same").unwrap();
    set_mtime(&path, 100);
    assert_eq!(tracker.check_fresh(&path), Ok(()));
    assert!(tracker.changed_since_read().is_empty());
}

#[test]
fn same_size_edits_with_new_mtime_are_modified() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("g.txt");
    std::fs::write(&path, "aaaa").unwrap();
    set_mtime(&path, 0);
    let tracker = FileTracker::new();
    tracker.record_read(&path).unwrap();
    std::fs::write(&path, "bbbb").unwrap();
    set_mtime(&path, 1);
    assert_eq!(tracker.check_fresh(&path), Err(Freshness::Modified));
}

#[test]
fn deletion_counts_as_modified_and_forget_resets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gone.txt");
    std::fs::write(&path, "x").unwrap();
    let tracker = FileTracker::new();
    tracker.record_read(&path).unwrap();
    let key = canonical(&path);
    std::fs::remove_file(&path).unwrap();

    assert_eq!(tracker.check_fresh(&path), Err(Freshness::Modified));
    assert_eq!(tracker.changed_since_read(), vec![key.clone()]);
    assert_eq!(tracker.tracked(), vec![key]);
    tracker.forget(&path);
    assert_eq!(tracker.check_fresh(&path), Err(Freshness::NotRead));
    assert!(tracker.tracked().is_empty());
}

#[test]
fn equivalent_spellings_share_one_entry_and_clones_share_state() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let path = dir.path().join("sub/h.txt");
    std::fs::write(&path, "x").unwrap();
    let tracker = FileTracker::new();
    tracker
        .record_read(&dir.path().join("sub/./h.txt"))
        .unwrap();
    let clone = tracker.clone();
    assert!(clone.was_read(&dir.path().join("sub/../sub/h.txt")));
    assert_eq!(clone.tracked(), vec![canonical(&path)]);
}

#[test]
fn missing_files_cannot_be_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nope.txt");
    let tracker = FileTracker::new();
    assert!(tracker.record_read(&path).unwrap_err().is_not_found());
    assert!(!tracker.was_read(&path));
}
