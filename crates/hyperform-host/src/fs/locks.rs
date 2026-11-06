//! Per-file async locks that serialize read-modify-write edits of the same
//! path while letting edits of different paths run concurrently.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::OwnedMutexGuard;

use super::paths::identity;

type Entries = HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>;

/// Clones share the lock table. Entries nobody holds or waits on are
/// dropped lazily on the next `lock` call.
#[derive(Debug, Clone, Default)]
pub struct PathLocks {
    entries: Arc<Mutex<Entries>>,
}

impl PathLocks {
    pub fn new() -> Self {
        Self::default()
    }

    /// Waits for exclusive access to `path` (keyed by its canonical form).
    pub async fn lock(&self, path: &Path) -> OwnedMutexGuard<()> {
        let key = identity(path);
        let entry = {
            let mut entries = self.table();
            purge(&mut entries);
            Arc::clone(entries.entry(key).or_default())
        };
        entry.lock_owned().await
    }

    /// Number of paths currently locked or awaited.
    pub fn len(&self) -> usize {
        let mut entries = self.table();
        purge(&mut entries);
        entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Single inserts/removes only; a poisoned table is still consistent.
    fn table(&self) -> MutexGuard<'_, Entries> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// An entry referenced only by the table has no holder and no waiter
/// (both keep an `Arc` clone), so it can go.
fn purge(entries: &mut Entries) {
    entries.retain(|_, lock| Arc::strong_count(lock) > 1);
}
