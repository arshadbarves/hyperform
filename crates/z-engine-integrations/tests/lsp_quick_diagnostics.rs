//! Diagnostics after a write never stall the writer: nothing is started or
//! awaited for a server that is not up, a file the server never published
//! for is synced without waiting, and a server reporting work in progress
//! (indexing) is not waited on. A ready server that has seen the file
//! answers within the bound.

mod support;

use std::path::Path;
use std::time::{Duration, Instant};

use support::{fake_lsp, rust_project};
use z_engine_integrations::{LspManager, LspServerState};

const WAIT: Duration = Duration::from_secs(2);
/// Far below `WAIT`: an answer this fast did not wait for a publish.
const NO_WAIT: Duration = Duration::from_millis(1_500);

/// Polls until the server has published for `file` (the unchanged file
/// then answers fresh without waiting).
async fn until_published(manager: &LspManager, file: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let report = manager.diagnostics_if_running(file, WAIT).await.unwrap();
        if report.is_some_and(|report| report.fresh) {
            return;
        }
        assert!(Instant::now() < deadline, "the first publish never arrived");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn only_a_ready_server_that_knows_the_file_is_waited_on() {
    let (_dir, root) = rust_project();
    let manager = LspManager::new(&root, vec![fake_lsp(&["--quiet"])]);
    let lib = Path::new("src/lib.rs");
    let before = manager.diagnostics_if_running(lib, WAIT).await.unwrap();
    assert!(before.is_none(), "no server is started or awaited");
    assert!(
        manager
            .status()
            .iter()
            .all(|s| s.state != LspServerState::Running)
    );

    manager.start_for(lib).await.unwrap();
    std::fs::write(root.join("src/new.rs"), "fn fresh() {}\n").unwrap();
    let started = Instant::now();
    let report = manager
        .diagnostics_if_running(Path::new("src/new.rs"), WAIT)
        .await
        .unwrap()
        .expect("the server is up");
    assert!(!report.fresh);
    assert!(
        started.elapsed() < NO_WAIT,
        "a file never published for is not waited on ({:?})",
        started.elapsed()
    );
    manager.shutdown_all().await;
}

#[tokio::test]
async fn a_published_file_gets_fresh_diagnostics() {
    let (_dir, root) = rust_project();
    let manager = LspManager::new(&root, vec![fake_lsp(&[])]);
    let lib = Path::new("src/lib.rs");
    manager.start_for(lib).await.unwrap();
    until_published(&manager, lib).await;
    std::fs::write(root.join("src/lib.rs"), "fn broken() {} // ERROR here\n").unwrap();
    let report = manager
        .diagnostics_if_running(lib, WAIT)
        .await
        .unwrap()
        .unwrap();
    assert!(report.fresh);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.message.contains("ERROR"))
    );
    manager.shutdown_all().await;
}

#[tokio::test]
async fn a_busy_server_is_not_waited_on() {
    let (_dir, root) = rust_project();
    let manager = LspManager::new(&root, vec![fake_lsp(&["--busy"])]);
    let lib = Path::new("src/lib.rs");
    manager.start_for(lib).await.unwrap();
    until_published(&manager, lib).await;
    std::fs::write(root.join("src/lib.rs"), "fn changed() {}\n").unwrap();
    let started = Instant::now();
    let report = manager
        .diagnostics_if_running(lib, WAIT)
        .await
        .unwrap()
        .unwrap();
    assert!(!report.fresh);
    assert!(started.elapsed() < NO_WAIT, "{:?}", started.elapsed());
    manager.shutdown_all().await;
}
