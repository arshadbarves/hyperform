//! Where a pinned model lives and what each of its files must be.

use std::path::{Path, PathBuf};

use z_engine_config::Paths;
use z_engine_decisions::{NATIVE_MODELS, NativeModel, native_model};
use z_engine_host::{DownloadSpec, downloaded_bytes};

/// `<data dir>/models/laya/<revision>/`.
pub(crate) fn model_dir(paths: &Paths, model: &NativeModel) -> PathBuf {
    paths
        .data_dir
        .join("models")
        .join("laya")
        .join(model.revision)
}

/// The pinned model for `decisions.checkpoint` (or one of Laya's aliases).
pub(crate) fn resolve(checkpoint: &str) -> Result<&'static NativeModel, String> {
    native_model(checkpoint).ok_or_else(|| {
        let known: Vec<&str> = NATIVE_MODELS.iter().map(|model| model.checkpoint).collect();
        format!(
            "the native runtime has no model for checkpoint \"{checkpoint}\" (it has {})",
            known.join(", ")
        )
    })
}

/// Every file of `model`, fetched into `dir` at its pinned revision.
pub(crate) fn specs(dir: &Path, model: &NativeModel) -> Vec<DownloadSpec> {
    let spec = |file: &z_engine_decisions::ModelFile| DownloadSpec {
        url: model.url(file),
        dest: dir.join(file.path),
        size: file.size,
        sha256: file.sha256.to_string(),
    };
    model.files.iter().map(spec).collect()
}

/// Bytes of the model on disk, by size; checksums are checked on load.
pub(crate) async fn bytes_on_disk(specs: &[DownloadSpec]) -> u64 {
    let mut total = 0;
    for spec in specs {
        total += downloaded_bytes(spec).await;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_resolve_by_alias_and_live_under_their_revision() {
        let model = resolve("ml").unwrap();
        assert_eq!(model.checkpoint, "multilingual");
        let paths = Paths::with_roots("/z/config", "/z/data");
        let dir = model_dir(&paths, model);
        let want = Path::new("/z/data/models/laya").join(model.revision);
        assert_eq!(dir, want);
        let specs = specs(&dir, model);
        assert_eq!(specs.len(), model.files.len());
        assert!(specs.iter().all(|spec| spec.url.contains(model.revision)));
        let error = resolve("gpt").unwrap_err();
        assert!(
            error.contains("\"gpt\"") && error.contains("english"),
            "{error}"
        );
    }
}
