//! `@file` search over a per-root file index, rebuilt when older than
//! [`MAX_AGE`]. The cache is process-wide: indexes depend only on the tree.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use z_engine_host::FileIndex;

use crate::engine::Engine;
use crate::error::EngineError;
use crate::sync::lock;

const MAX_AGE: Duration = Duration::from_secs(30);
const MAX_FILES: usize = 50_000;
const MAX_RESULTS: usize = 200;

type Cache = Mutex<HashMap<PathBuf, (Instant, Arc<FileIndex>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Cache::default)
}

impl Engine {
    /// Best matches for `query` under `project_root` (directories end with
    /// `/`); an empty query lists recently modified files.
    pub async fn list_files(
        &self,
        project_root: &Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>, EngineError> {
        let root = std::fs::canonicalize(project_root).map_err(|error| {
            EngineError::NotFound(format!("{}: {error}", project_root.display()))
        })?;
        let index = index_for(root).await?;
        Ok(index.search(query, limit.clamp(1, MAX_RESULTS)))
    }
}

async fn index_for(root: PathBuf) -> Result<Arc<FileIndex>, EngineError> {
    let fresh = lock(cache())
        .get(&root)
        .filter(|(built, _)| built.elapsed() < MAX_AGE)
        .map(|(_, index)| Arc::clone(index));
    if let Some(index) = fresh {
        return Ok(index);
    }
    let walk_root = root.clone();
    let index = tokio::task::spawn_blocking(move || FileIndex::build(&walk_root, MAX_FILES))
        .await
        .map_err(|error| EngineError::Invalid(format!("file index failed: {error}")))??;
    let index = Arc::new(index);
    lock(cache()).insert(root, (Instant::now(), Arc::clone(&index)));
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::super::testing::engine;

    #[tokio::test]
    async fn files_are_found_and_the_index_refreshes_when_stale() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let root = dir.path().join("project");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        let found = engine.list_files(&root, "main", 10).await.unwrap();
        assert!(found.contains(&"src/main.rs".to_string()), "{found:?}");

        std::fs::write(root.join("src/lib.rs"), "").unwrap();
        let cached = engine.list_files(&root, "lib.rs", 10).await.unwrap();
        assert!(cached.is_empty(), "served from the cache: {cached:?}");
        let key = std::fs::canonicalize(&root).unwrap();
        if let Some(entry) = super::lock(super::cache()).get_mut(&key) {
            entry.0 = entry.0.checked_sub(super::MAX_AGE).unwrap();
        }
        let rebuilt = engine.list_files(&root, "lib.rs", 10).await.unwrap();
        assert_eq!(rebuilt, ["src/lib.rs"]);
        assert!(
            engine
                .list_files(&dir.path().join("missing"), "", 5)
                .await
                .is_err()
        );
    }
}
