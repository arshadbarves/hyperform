//! Documents a client has opened on its server: the text the server holds
//! and its version. The server is re-synced with the full text whenever
//! the file on disk differs from that text (a superset of "the mtime
//! changed" that cannot miss same-second edits).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::sync::lock;

/// What a sync had to tell the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    /// `didOpen` was sent.
    Opened,
    /// `didChange` with the full new text was sent.
    Changed,
    /// The server already had this text.
    Unchanged,
}

#[derive(Debug)]
struct OpenDocument {
    version: i32,
    text: Arc<str>,
}

#[derive(Debug, Default)]
pub(crate) struct Documents {
    open: Mutex<HashMap<PathBuf, OpenDocument>>,
}

impl Documents {
    /// Records `text` as the server's content of `path` and returns the
    /// notification needed with the document's new version.
    pub(crate) fn update(&self, path: &Path, text: &str) -> (SyncOutcome, i32) {
        let mut open = lock(&self.open);
        match open.get_mut(path) {
            Some(document) if &*document.text == text => (SyncOutcome::Unchanged, document.version),
            Some(document) => {
                document.version += 1;
                document.text = Arc::from(text);
                (SyncOutcome::Changed, document.version)
            }
            None => {
                let document = OpenDocument {
                    version: 1,
                    text: Arc::from(text),
                };
                open.insert(path.to_path_buf(), document);
                (SyncOutcome::Opened, 1)
            }
        }
    }

    /// The text the server holds for `path`.
    pub(crate) fn text(&self, path: &Path) -> Option<Arc<str>> {
        lock(&self.open)
            .get(path)
            .map(|document| Arc::clone(&document.text))
    }

    pub(crate) fn count(&self) -> usize {
        lock(&self.open).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_advance_only_on_real_changes() {
        let documents = Documents::default();
        let path = Path::new("/p/a.rs");
        assert_eq!(documents.update(path, "one"), (SyncOutcome::Opened, 1));
        assert_eq!(documents.update(path, "one"), (SyncOutcome::Unchanged, 1));
        assert_eq!(documents.update(path, "two"), (SyncOutcome::Changed, 2));
        assert_eq!(documents.text(path).as_deref(), Some("two"));
        assert_eq!(documents.count(), 1);
        assert!(documents.text(Path::new("/p/b.rs")).is_none());
    }
}
