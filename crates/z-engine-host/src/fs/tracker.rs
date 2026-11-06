//! Read-before-edit bookkeeping: remembers what each file looked like when
//! the model last saw it, so edits can refuse stale content and reminders
//! can mention files changed behind the model's back.

use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use super::paths::identity;
use crate::HostError;

/// Why a file cannot be edited without re-reading it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// The file was never read (or was forgotten).
    NotRead,
    /// Size or mtime changed and the content hash differs, or it was deleted.
    Modified,
}

/// Thread-safe tracker; clones share state. Methods do small blocking file
/// I/O (metadata, and a hash when metadata changed).
#[derive(Debug, Clone, Default)]
pub struct FileTracker {
    stamps: Arc<Mutex<HashMap<PathBuf, Stamp>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    len: u64,
    mtime: Option<SystemTime>,
    digest: [u8; 32],
}

enum Observed {
    Unchanged,
    /// Metadata moved but the content is identical (e.g. `touch`).
    Touched(Stamp),
    Changed,
}

impl FileTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Remembers the current on-disk state of `path` as seen by the model.
    pub fn record_read(&self, path: &Path) -> Result<(), HostError> {
        let stamp = Stamp::capture(path).map_err(|e| HostError::io(path, e))?;
        self.lock().insert(identity(path), stamp);
        Ok(())
    }

    /// Refreshes the stamp after one of our own writes.
    pub fn record_write(&self, path: &Path) -> Result<(), HostError> {
        self.record_read(path)
    }

    pub fn check_fresh(&self, path: &Path) -> Result<(), Freshness> {
        let key = identity(path);
        let stamp = self.lock().get(&key).cloned().ok_or(Freshness::NotRead)?;
        match observe(&stamp, path) {
            Observed::Unchanged => Ok(()),
            Observed::Touched(now) => {
                self.lock().insert(key, now);
                Ok(())
            }
            Observed::Changed => Err(Freshness::Modified),
        }
    }

    pub fn was_read(&self, path: &Path) -> bool {
        self.lock().contains_key(&identity(path))
    }

    pub fn forget(&self, path: &Path) {
        self.lock().remove(&identity(path));
    }

    /// Every tracked path, sorted.
    pub fn tracked(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.lock().keys().cloned().collect();
        paths.sort();
        paths
    }

    /// Tracked files whose content changed (or that vanished) since the
    /// model last read or wrote them, sorted.
    pub fn changed_since_read(&self) -> Vec<PathBuf> {
        let stamps: Vec<(PathBuf, Stamp)> = self
            .lock()
            .iter()
            .map(|(path, stamp)| (path.clone(), stamp.clone()))
            .collect();
        let mut changed: Vec<PathBuf> = stamps
            .into_iter()
            .filter(|(path, stamp)| matches!(observe(stamp, path), Observed::Changed))
            .map(|(path, _)| path)
            .collect();
        changed.sort();
        changed
    }

    /// The map is only mutated by single inserts/removes, so a panic in
    /// another holder cannot leave it half-updated; recover the data.
    fn lock(&self) -> MutexGuard<'_, HashMap<PathBuf, Stamp>> {
        self.stamps.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Stamp {
    fn capture(path: &Path) -> std::io::Result<Stamp> {
        let meta = std::fs::metadata(path)?;
        let bytes = std::fs::read(path)?;
        Ok(Stamp {
            len: bytes.len() as u64,
            mtime: meta.modified().ok(),
            digest: Sha256::digest(&bytes).into(),
        })
    }

    fn metadata_matches(&self, meta: &Metadata) -> bool {
        self.len == meta.len() && self.mtime == meta.modified().ok()
    }
}

fn observe(stamp: &Stamp, path: &Path) -> Observed {
    let Ok(meta) = std::fs::metadata(path) else {
        return Observed::Changed;
    };
    if stamp.metadata_matches(&meta) {
        return Observed::Unchanged;
    }
    match Stamp::capture(path) {
        Ok(now) if now.digest == stamp.digest => Observed::Touched(now),
        _ => Observed::Changed,
    }
}
