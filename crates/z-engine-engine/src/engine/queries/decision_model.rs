//! Settings' native decision model: whether the pinned model for the
//! configured checkpoint is on disk, its download (started only from
//! Settings, which polls the status for progress), and removing it.

use std::path::Path;

use serde::Serialize;
use z_engine_host::remove_download_dir;

use crate::decisions::native::{bytes_on_disk, model_dir, resolve, specs};
use crate::engine::Engine;
use crate::error::EngineError;
use crate::settings::load_session_settings;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionModelStatus {
    /// `decisions.checkpoint` as configured.
    pub checkpoint: String,
    /// A pinned model exists for the checkpoint.
    pub available: bool,
    /// The Hugging Face repository and pinned commit the files come from.
    pub repo: Option<String>,
    pub revision: Option<String>,
    /// `<data dir>/models/laya/<revision>/`.
    pub dir: Option<String>,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    /// Every file is on disk with its pinned size (checksums are checked on load).
    pub installed: bool,
    pub downloading: bool,
    /// Why the last download stopped, or why there is no model to download.
    pub error: Option<String>,
    /// This app can run the model (it was built with the `onnx` feature).
    pub native_built: bool,
}

impl Engine {
    /// The native model of `project_root`'s effective checkpoint (the
    /// user's alone when `None`).
    pub async fn decision_model_status(&self, project_root: Option<&Path>) -> DecisionModelStatus {
        let checkpoint = self.decision_checkpoint(project_root);
        let shared = self.shared();
        let mut status = DecisionModelStatus {
            checkpoint: checkpoint.clone(),
            available: false,
            repo: None,
            revision: None,
            dir: None,
            total_bytes: 0,
            downloaded_bytes: 0,
            installed: false,
            downloading: false,
            error: None,
            native_built: cfg!(feature = "onnx"),
        };
        let model = match resolve(&checkpoint) {
            Ok(model) => model,
            Err(reason) => {
                status.error = Some(reason);
                return status;
            }
        };
        let dir = model_dir(&shared.paths, model);
        let progress = shared.sidecars.native().progress(model.revision);
        let downloading = progress.as_ref().is_some_and(|progress| progress.running);
        let downloaded = match &progress {
            Some(progress) if downloading => progress.done,
            _ => bytes_on_disk(&specs(&dir, model)).await,
        };
        status.available = true;
        status.repo = Some(model.repo.to_string());
        status.revision = Some(model.revision.to_string());
        status.dir = Some(dir.display().to_string());
        status.total_bytes = model.total_size();
        status.downloaded_bytes = downloaded;
        status.installed = !downloading && downloaded == model.total_size();
        status.downloading = downloading;
        status.error = progress.and_then(|progress| progress.error);
        status
    }

    /// Starts (or resumes) downloading the checkpoint's model in the
    /// background; poll [`Engine::decision_model_status`] for progress.
    pub async fn download_decision_model(
        &self,
        project_root: Option<&Path>,
    ) -> Result<DecisionModelStatus, EngineError> {
        let model =
            resolve(&self.decision_checkpoint(project_root)).map_err(EngineError::Invalid)?;
        let shared = self.shared();
        let dir = model_dir(&shared.paths, model);
        shared
            .sidecars
            .native()
            .start_download(model, dir)
            .map_err(EngineError::Busy)?;
        Ok(self.decision_model_status(project_root).await)
    }

    /// Stops a running download; its partial files stay, to resume later.
    pub fn cancel_decision_model_download(&self) {
        self.shared().sidecars.native().cancel_download();
    }

    /// Deletes the checkpoint's model folder; refused while a download runs.
    pub async fn remove_decision_model(
        &self,
        project_root: Option<&Path>,
    ) -> Result<DecisionModelStatus, EngineError> {
        let model =
            resolve(&self.decision_checkpoint(project_root)).map_err(EngineError::Invalid)?;
        let shared = self.shared();
        shared
            .sidecars
            .native()
            .forget(model.revision)
            .map_err(EngineError::Busy)?;
        remove_download_dir(&model_dir(&shared.paths, model)).await?;
        Ok(self.decision_model_status(project_root).await)
    }

    fn decision_checkpoint(&self, project_root: Option<&Path>) -> String {
        let shared = self.shared();
        // The config directory is never a project, so it loads the user layer.
        let root = project_root.unwrap_or(&shared.paths.config_dir);
        let settings = load_session_settings(&shared.paths, root, &shared.env).settings;
        settings.settings.decisions.checkpoint.clone()
    }
}

#[cfg(test)]
mod tests {
    use z_engine_decisions::native_model;

    use super::super::testing::engine;

    #[tokio::test]
    async fn the_default_checkpoint_has_a_model_that_is_not_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let status = engine.decision_model_status(None).await;
        let model = native_model("multilingual").unwrap();
        assert!(status.available && !status.installed && !status.downloading);
        assert_eq!(status.revision.as_deref(), Some(model.revision));
        assert_eq!(
            (status.total_bytes, status.downloaded_bytes),
            (model.total_size(), 0)
        );
        assert_eq!(status.native_built, cfg!(feature = "onnx"));
        let models = engine.paths().data_dir.join("models").join("laya");
        assert!(status.dir.unwrap().starts_with(&*models.to_string_lossy()));
    }

    #[tokio::test]
    async fn partial_files_count_and_removing_deletes_them() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let status = engine.decision_model_status(None).await;
        let folder = std::path::PathBuf::from(status.dir.unwrap());
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("tokenizer.json.part"), [0_u8; 10]).unwrap();
        let status = engine.decision_model_status(None).await;
        assert_eq!(status.downloaded_bytes, 10);
        let status = engine.remove_decision_model(None).await.unwrap();
        assert_eq!(status.downloaded_bytes, 0);
        assert!(!folder.exists());
    }

    #[tokio::test]
    async fn an_unknown_checkpoint_has_nothing_to_download() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let settings = "schema = 2\n[decisions]\ncheckpoint = \"gpt\"\n";
        std::fs::write(&engine.paths().user_settings_file, settings).unwrap();
        let status = engine.decision_model_status(None).await;
        assert!(!status.available);
        assert!(status.error.unwrap().contains("\"gpt\""));
        assert!(engine.download_decision_model(None).await.is_err());
    }

    #[tokio::test]
    async fn testing_the_native_runtime_says_what_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let settings = "schema = 2\n[decisions]\nruntime = \"native\"\n";
        std::fs::write(&engine.paths().user_settings_file, settings).unwrap();
        let test = engine.test_decision_model(None).await;
        let error = test.report.error.unwrap();
        let want = if cfg!(feature = "onnx") {
            "not downloaded"
        } else {
            "onnx"
        };
        assert!(!test.report.ok && error.contains(want), "{error}");
    }
}
