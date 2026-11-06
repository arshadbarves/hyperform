//! Registered workspace roots (Codex-desktop style projects), stored as in
//! v1: `<data dir>/workspaces.json` = `{ "roots": [...] }`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ipc::{IpcResult, fail};

pub(crate) const FILE_NAME: &str = "workspaces.json";

#[derive(Serialize, Deserialize, Default)]
struct WorkspacesFile {
    roots: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) struct Workspaces {
    file: PathBuf,
}

impl Workspaces {
    pub(crate) fn new(data_dir: &Path) -> Self {
        Self {
            file: data_dir.join(FILE_NAME),
        }
    }

    /// A missing or unreadable file lists nothing.
    pub(crate) fn load(&self) -> Vec<PathBuf> {
        std::fs::read_to_string(&self.file)
            .ok()
            .and_then(|text| serde_json::from_str::<WorkspacesFile>(&text).ok())
            .map(|file| file.roots)
            .unwrap_or_default()
    }

    /// Deduplicates (keeping order) and writes through a temp file, so a
    /// crash cannot truncate the registry.
    pub(crate) fn save(&self, roots: &[PathBuf]) -> IpcResult<()> {
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let mut unique: Vec<PathBuf> = Vec::new();
        for root in roots {
            if !unique.contains(root) {
                unique.push(root.clone());
            }
        }
        let text = serde_json::to_string_pretty(&WorkspacesFile { roots: unique }).map_err(fail)?;
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(fail)?;
        std::fs::rename(&tmp, &self.file).map_err(fail)
    }

    pub(crate) fn add(&self, root: PathBuf) -> IpcResult<()> {
        let mut roots = self.load();
        if !roots.contains(&root) {
            roots.push(root);
            self.save(&roots)?;
        }
        Ok(())
    }

    pub(crate) fn remove(&self, targets: &[PathBuf]) -> IpcResult<()> {
        let roots: Vec<PathBuf> = self
            .load()
            .into_iter()
            .filter(|root| !targets.contains(root))
            .collect();
        self.save(&roots)
    }

    /// The launch directory when it is a project, else the first saved
    /// workspace, else the home directory.
    pub(crate) fn initial_root(&self, home: Option<&Path>) -> PathBuf {
        if let Ok(cwd) = std::env::current_dir() {
            if is_valid_project_root(&cwd) {
                return cwd;
            }
        }
        self.load()
            .into_iter()
            .find(|root| is_valid_project_root(root))
            .or_else(|| home.map(Path::to_path_buf))
            .unwrap_or_else(std::env::temp_dir)
    }
}

/// An existing directory other than the filesystem root.
pub(crate) fn is_valid_project_root(path: &Path) -> bool {
    path.parent().is_some() && path.is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_round_trip_deduplicated_and_removable() {
        let dir = tempfile::tempdir().unwrap();
        let store = Workspaces::new(dir.path());
        assert!(store.load().is_empty());
        store.add("/a".into()).unwrap();
        store.add("/b".into()).unwrap();
        store.add("/a".into()).unwrap();
        assert_eq!(store.load(), [PathBuf::from("/a"), PathBuf::from("/b")]);
        let text = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(text.contains("\"roots\""), "v1 format: {text}");
        store.remove(&["/a".into()]).unwrap();
        assert_eq!(store.load(), [PathBuf::from("/b")]);
        assert!(!is_valid_project_root(Path::new("/")));
        assert!(is_valid_project_root(dir.path()));
    }
}
