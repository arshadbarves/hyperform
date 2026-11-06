//! Diagnostics a server published, per file, with a generation counter so a
//! caller can wait for a publish that happened after its own sync.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::watch;

use super::uri::uri_to_path;
use crate::sync::lock;

#[derive(Debug, Default)]
struct Published {
    generation: u64,
    items: Vec<Value>,
}

#[derive(Debug, Default)]
struct Files {
    by_path: HashMap<PathBuf, Published>,
    counter: u64,
}

#[derive(Debug)]
pub(crate) struct DiagnosticsStore {
    files: Mutex<Files>,
    /// The generation of the latest publish, for waiters.
    latest: watch::Sender<u64>,
}

impl Default for DiagnosticsStore {
    fn default() -> Self {
        Self {
            files: Mutex::new(Files::default()),
            latest: watch::Sender::new(0),
        }
    }
}

impl DiagnosticsStore {
    /// Records a `textDocument/publishDiagnostics` notification.
    pub(crate) fn publish(&self, params: Option<&Value>) {
        let Some(path) = params
            .and_then(|p| p.get("uri"))
            .and_then(Value::as_str)
            .and_then(uri_to_path)
        else {
            tracing::debug!("ignored diagnostics for an unusable URI");
            return;
        };
        let items = params
            .and_then(|p| p.get("diagnostics"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let generation = {
            let mut files = lock(&self.files);
            files.counter += 1;
            let generation = files.counter;
            files.by_path.insert(path, Published { generation, items });
            generation
        };
        // Announced only after the insert, so a woken waiter sees it.
        self.latest.send_replace(generation);
    }

    /// Generation of the latest publish for `path`; 0 when none arrived.
    pub(crate) fn generation(&self, path: &Path) -> u64 {
        lock(&self.files)
            .by_path
            .get(path)
            .map_or(0, |published| published.generation)
    }

    pub(crate) fn get(&self, path: &Path) -> Vec<Value> {
        lock(&self.files)
            .by_path
            .get(path)
            .map(|published| published.items.clone())
            .unwrap_or_default()
    }

    /// Waits up to `wait` for a publish for `path` newer than `after`;
    /// returns whether one arrived.
    pub(crate) async fn wait_newer(&self, path: &Path, after: u64, wait: Duration) -> bool {
        let mut latest = self.latest.subscribe();
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            if self.generation(path) > after {
                return true;
            }
            match tokio::time::timeout_at(deadline, latest.changed()).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) | Err(_) => return self.generation(path) > after,
            }
        }
    }
}

// The fixtures use Unix-style `file:///p/a.rs` URIs.
#[cfg(all(test, unix))]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;

    fn params(uri: &str, count: usize) -> Value {
        json!({"uri": uri, "diagnostics": vec![json!({"message": "m"}); count]})
    }

    #[tokio::test]
    async fn waits_for_a_newer_publish_of_the_same_file() {
        let store = Arc::new(DiagnosticsStore::default());
        let path = Path::new("/p/a.rs");
        assert_eq!(store.generation(path), 0);
        assert!(!store.wait_newer(path, 0, Duration::from_millis(20)).await);
        let publisher = Arc::clone(&store);
        let task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            publisher.publish(Some(&params("file:///p/b.rs", 1)));
            publisher.publish(Some(&params("file:///p/a.rs", 2)));
        });
        assert!(store.wait_newer(path, 0, Duration::from_secs(5)).await);
        task.await.unwrap();
        assert_eq!(store.get(path).len(), 2);
        let seen = store.generation(path);
        assert!(
            !store
                .wait_newer(path, seen, Duration::from_millis(20))
                .await
        );
        store.publish(Some(&json!({"uri": "untitled:1", "diagnostics": []})));
        assert_eq!(store.get(Path::new("/p/b.rs")).len(), 1);
    }
}
