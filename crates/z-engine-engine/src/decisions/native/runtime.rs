//! The engine-wide native runtime: one model download at a time with its
//! progress for Settings, and (with the `onnx` feature) the loaded model
//! sessions share, replaced when the model it loads changes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use z_engine_decisions::NativeModel;
#[cfg(feature = "onnx")]
use z_engine_decisions::{DecisionProvider, OnnxConfig, OnnxProvider};
use z_engine_host::{DownloadSpec, Downloader, HostError};

use super::files::specs;
use crate::sync::lock;

const CANCELLED: &str = "the download was cancelled; downloading again resumes it";

/// The running or last download of one model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadProgress {
    pub done: u64,
    pub running: bool,
    /// Why the last attempt stopped; `None` while running or after success.
    pub error: Option<String>,
}

#[derive(Debug)]
struct Job {
    id: u64,
    revision: &'static str,
    done: Arc<AtomicU64>,
    cancel: CancellationToken,
    /// `None` while running.
    outcome: Option<Result<(), String>>,
}

#[derive(Debug, Default)]
struct Inner {
    next_id: AtomicU64,
    job: Mutex<Option<Job>>,
    #[cfg(feature = "onnx")]
    loaded: Mutex<Option<Arc<OnnxProvider>>>,
}

/// Cheap to clone; clones share the download and the loaded model.
#[derive(Debug, Clone, Default)]
pub(crate) struct NativeRuntime {
    inner: Arc<Inner>,
}

impl NativeRuntime {
    /// Downloads `model` into `dir` in the background; a download of it
    /// already running continues, one of another model is an error.
    pub(crate) fn start_download(
        &self,
        model: &'static NativeModel,
        dir: PathBuf,
    ) -> Result<(), String> {
        let mut job = lock(&self.inner.job);
        if let Some(current) = job.as_ref().filter(|job| job.outcome.is_none()) {
            if current.cancel.is_cancelled() {
                return Err("the last download is still stopping; try again in a moment".into());
            }
            if current.revision == model.revision {
                return Ok(());
            }
            return Err("another decision model is downloading".into());
        }
        let downloader = Downloader::new().map_err(|error| error.to_string())?;
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (done, cancel) = (Arc::new(AtomicU64::new(0)), CancellationToken::new());
        *job = Some(Job {
            id,
            revision: model.revision,
            done: Arc::clone(&done),
            cancel: cancel.clone(),
            outcome: None,
        });
        drop(job);
        let runtime = self.clone();
        tokio::spawn(async move {
            let outcome = fetch_all(&downloader, &specs(&dir, model), &done, &cancel).await;
            match &outcome {
                Ok(()) => {
                    tracing::info!(checkpoint = model.checkpoint, "decision model downloaded")
                }
                Err(error) => tracing::warn!(%error, "decision model download stopped"),
            }
            runtime.finish(id, model.revision, outcome);
        });
        Ok(())
    }

    fn finish(&self, id: u64, revision: &str, outcome: Result<(), String>) {
        let succeeded = outcome.is_ok();
        if let Some(job) = lock(&self.inner.job).as_mut().filter(|job| job.id == id) {
            job.outcome = Some(outcome);
        }
        if succeeded {
            self.unload(revision);
        }
    }

    pub(crate) fn cancel_download(&self) {
        if let Some(job) = lock(&self.inner.job).as_ref() {
            job.cancel.cancel();
        }
    }

    /// The running or last download of `revision`.
    pub(crate) fn progress(&self, revision: &str) -> Option<DownloadProgress> {
        let job = lock(&self.inner.job);
        let job = job.as_ref().filter(|job| job.revision == revision)?;
        Some(DownloadProgress {
            done: job.done.load(Ordering::Relaxed),
            running: job.outcome.is_none(),
            error: job.outcome.clone().and_then(Result::err),
        })
    }

    /// Forgets `revision`'s loaded model and last download before its files
    /// are removed; refused while any download runs.
    pub(crate) fn forget(&self, revision: &str) -> Result<(), String> {
        let mut job = lock(&self.inner.job);
        if job.as_ref().is_some_and(|job| job.outcome.is_none()) {
            return Err("cancel the download first".into());
        }
        if job.as_ref().is_some_and(|job| job.revision == revision) {
            *job = None;
        }
        drop(job);
        self.unload(revision);
        Ok(())
    }

    #[cfg(feature = "onnx")]
    fn unload(&self, revision: &str) {
        let mut loaded = lock(&self.inner.loaded);
        if loaded
            .as_ref()
            .is_some_and(|provider| provider.config().revision == revision)
        {
            *loaded = None;
        }
    }

    #[cfg(not(feature = "onnx"))]
    fn unload(&self, _revision: &str) {}

    /// The shared model for `config`, loading it when it is not loaded yet,
    /// under `config`'s own call limits.
    #[cfg(feature = "onnx")]
    pub(crate) fn provider(&self, config: OnnxConfig) -> Arc<dyn DecisionProvider> {
        let mut loaded = lock(&self.inner.loaded);
        let shared = match loaded.as_ref() {
            Some(provider) if provider.config().same_model(&config) => Arc::clone(provider),
            _ => {
                let provider = Arc::new(OnnxProvider::open(config.clone()));
                *loaded = Some(Arc::clone(&provider));
                provider
            }
        };
        Arc::new(shared.with_call_limits(config.timeout, config.max_batch))
    }

    /// Waits up to `wait` for the loaded model; true if it is still loading.
    pub(crate) async fn wait_loaded(&self, wait: Duration) -> bool {
        #[cfg(feature = "onnx")]
        {
            let provider = lock(&self.inner.loaded).clone();
            if let Some(provider) = provider {
                let _ = provider.ready(wait).await;
                return provider.loading();
            }
        }
        let _ = wait;
        false
    }
}

async fn fetch_all(
    downloader: &Downloader,
    specs: &[DownloadSpec],
    done: &AtomicU64,
    cancel: &CancellationToken,
) -> Result<(), String> {
    let mut finished = 0;
    for spec in specs {
        let offset = finished;
        let progress = move |bytes: u64| done.store(offset + bytes, Ordering::Relaxed);
        downloader
            .fetch(spec, &progress, cancel)
            .await
            .map_err(|error| match error {
                HostError::Cancelled => CANCELLED.to_string(),
                other => other.to_string(),
            })?;
        finished += spec.size;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_downloading_until_settings_starts_it() {
        let runtime = NativeRuntime::default();
        let revision = z_engine_decisions::NATIVE_MODELS[0].revision;
        assert_eq!(runtime.progress(revision), None);
        runtime.cancel_download();
        assert_eq!(runtime.forget(revision), Ok(()));
    }

    #[test]
    fn a_running_download_blocks_forgetting_and_reports_progress() {
        let runtime = NativeRuntime::default();
        let model = &z_engine_decisions::NATIVE_MODELS[0];
        let done = Arc::new(AtomicU64::new(42));
        *lock(&runtime.inner.job) = Some(Job {
            id: 7,
            revision: model.revision,
            done: Arc::clone(&done),
            cancel: CancellationToken::new(),
            outcome: None,
        });
        let progress = runtime.progress(model.revision).unwrap();
        assert!(progress.running && progress.done == 42);
        assert!(runtime.forget(model.revision).is_err());
        runtime.finish(7, model.revision, Err(CANCELLED.into()));
        let progress = runtime.progress(model.revision).unwrap();
        assert_eq!(progress.error.as_deref(), Some(CANCELLED));
        assert!(!progress.running);
        assert_eq!(runtime.forget(model.revision), Ok(()));
        assert_eq!(runtime.progress(model.revision), None);
    }
}
