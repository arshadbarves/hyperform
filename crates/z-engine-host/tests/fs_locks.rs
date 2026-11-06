//! Per-path edit locks.

use std::time::{Duration, Instant};

use z_engine_host::PathLocks;

#[tokio::test]
async fn one_path_is_serialized_and_released_entries_disappear() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f.txt");
    let locks = PathLocks::new();
    let guard = locks.lock(&path).await;

    let (other, other_path) = (locks.clone(), path.clone());
    let waiter = tokio::spawn(async move {
        let _guard = other.lock(&other_path).await;
        Instant::now()
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!waiter.is_finished());
    assert_eq!(locks.len(), 1);

    let released = Instant::now();
    drop(guard);
    assert!(waiter.await.unwrap() >= released);
    assert!(locks.is_empty());
}

#[tokio::test]
async fn different_paths_do_not_block_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let locks = PathLocks::new();
    let a = locks.lock(&dir.path().join("a.txt")).await;
    let b = tokio::time::timeout(
        Duration::from_secs(1),
        locks.lock(&dir.path().join("b.txt")),
    )
    .await
    .expect("second path must not wait");
    assert_eq!(locks.len(), 2);
    drop((a, b));
    assert_eq!(locks.len(), 0);
}

#[tokio::test]
async fn equivalent_spellings_share_a_lock() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let locks = PathLocks::new();
    let _held = locks.lock(&dir.path().join("sub/f.txt")).await;
    let second = tokio::time::timeout(
        Duration::from_millis(100),
        locks.lock(&dir.path().join("sub/./../sub/f.txt")),
    )
    .await;
    assert!(second.is_err(), "the same file must not be locked twice");
}
