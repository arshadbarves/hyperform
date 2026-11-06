//! The native provider: Laya in this process through ONNX Runtime. The
//! model loads once on a blocking thread; until it is ready, every call is
//! unavailable (so the hybrid falls back to rules). Each call runs on a
//! blocking thread, bounded by the timeout and stopped on cancellation.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use ort::session::RunOptions;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::NAME;
use super::runtime::Runtime;
use crate::answer::Answer;
use crate::error::DecisionError;
use crate::native_model::NativeModel;
use crate::provider::DecisionProvider;
use crate::question::DecisionRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxConfig {
    /// The checkpoint's folder (`<data dir>/models/laya/<revision>/`).
    pub dir: PathBuf,
    /// Checkpoint and revision name the provider revision (cache key).
    pub checkpoint: String,
    pub revision: String,
    /// Tokens per question, within the checkpoint's limit.
    pub max_len: u32,
    /// The whole call, all batches included.
    pub timeout: Duration,
    /// Questions per session run.
    pub max_batch: usize,
    /// `(relative path, SHA-256)` of every file, checked before any is read.
    pub expected_sha256: Vec<(String, String)>,
}

impl OnnxConfig {
    /// The config for a pinned model downloaded to `dir`: every file's pin
    /// is checked at load, and `max_len` is cut to the checkpoint's limit.
    pub fn for_model(
        model: &NativeModel,
        dir: PathBuf,
        max_len: u32,
        timeout: Duration,
        max_batch: usize,
    ) -> Self {
        Self {
            dir,
            checkpoint: model.checkpoint.to_string(),
            revision: model.revision.to_string(),
            max_len: max_len.min(model.max_len_cap),
            timeout,
            max_batch,
            expected_sha256: model
                .files
                .iter()
                .map(|file| (file.path.to_string(), file.sha256.to_string()))
                .collect(),
        }
    }

    /// Loads the same model; the call limits (timeout, batch) may differ.
    pub fn same_model(&self, other: &Self) -> bool {
        self.dir == other.dir
            && self.checkpoint == other.checkpoint
            && self.revision == other.revision
            && self.max_len == other.max_len
            && self.expected_sha256 == other.expected_sha256
    }
}

#[derive(Debug)]
enum Load {
    Loading,
    Ready(Arc<Runtime>),
    Failed(String),
}

#[derive(Debug)]
pub struct OnnxProvider {
    config: OnnxConfig,
    state: watch::Receiver<Load>,
}

impl OnnxProvider {
    /// Starts loading the model on a blocking thread and returns at once.
    /// Must be called inside a Tokio runtime.
    pub fn open(config: OnnxConfig) -> Self {
        let (loaded, state) = watch::channel(Load::Loading);
        let load = config.clone();
        tokio::task::spawn_blocking(move || {
            let started = Instant::now();
            let state = match Runtime::load(&load) {
                Ok(runtime) => {
                    let millis = started.elapsed().as_millis();
                    tracing::info!(checkpoint = %load.checkpoint, millis, "native decision model loaded");
                    Load::Ready(Arc::new(runtime))
                }
                Err(error) => {
                    tracing::warn!(checkpoint = %load.checkpoint, %error, "native decision model failed to load");
                    Load::Failed(error)
                }
            };
            // Nobody is left to tell when the provider is already gone.
            let _ = loaded.send(state);
        });
        Self { config, state }
    }

    pub fn config(&self) -> &OnnxConfig {
        &self.config
    }

    /// The same loaded (or loading) model under other call limits.
    pub fn with_call_limits(&self, timeout: Duration, max_batch: usize) -> Self {
        let config = OnnxConfig {
            timeout,
            max_batch,
            ..self.config.clone()
        };
        Self {
            config,
            state: self.state.clone(),
        }
    }

    /// The model is still being verified and loaded.
    pub fn loading(&self) -> bool {
        let stopped = self.state.has_changed().is_err();
        matches!(*self.state.borrow(), Load::Loading) && !stopped
    }

    /// Waits up to `wait` for the model to load; the reason it cannot answer yet, if any.
    pub async fn ready(&self, wait: Duration) -> Result<(), DecisionError> {
        let mut state = self.state.clone();
        let loaded = state.wait_for(|load| !matches!(load, Load::Loading));
        // Either way the state below says what happened.
        let _ = tokio::time::timeout(wait, loaded).await;
        self.runtime().map(drop)
    }

    fn runtime(&self) -> Result<Arc<Runtime>, DecisionError> {
        let stopped = self.state.has_changed().is_err();
        match &*self.state.borrow() {
            Load::Ready(runtime) => Ok(Arc::clone(runtime)),
            Load::Failed(error) => Err(DecisionError::Unavailable(error.clone())),
            Load::Loading if stopped => Err(DecisionError::Unavailable(
                "the native model stopped loading".into(),
            )),
            Load::Loading => Err(DecisionError::Unavailable(
                "the native model is still loading".into(),
            )),
        }
    }
}

#[async_trait]
impl DecisionProvider for OnnxProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn revision(&self) -> String {
        let config = &self.config;
        format!(
            "onnx:{}:{}:{}",
            config.checkpoint, config.revision, config.max_len
        )
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        request.validate()?;
        let runtime = self.runtime()?;
        let run = RunOptions::new()
            .map(Arc::new)
            .map_err(|error| DecisionError::Unavailable(error.to_string()))?;
        let job = {
            let (run, request) = (Arc::clone(&run), request.clone());
            let max_batch = self.config.max_batch;
            tokio::task::spawn_blocking(move || runtime.answer(&request, max_batch, &run))
        };
        let millis = u64::try_from(self.config.timeout.as_millis()).unwrap_or(u64::MAX);
        let outcome = tokio::select! {
            () = cancel.cancelled() => Err(DecisionError::Cancelled),
            result = tokio::time::timeout(self.config.timeout, job) => match result {
                Ok(Ok(answers)) => answers,
                Ok(Err(stopped)) => Err(DecisionError::Unavailable(format!(
                    "the native model stopped: {stopped}"
                ))),
                Err(_) => Err(DecisionError::Timeout(millis)),
            },
        };
        if matches!(
            outcome,
            Err(DecisionError::Cancelled | DecisionError::Timeout(_))
        ) {
            // The run may still be going; ONNX Runtime stops it at its next check.
            let _ = run.terminate();
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::native_model::native_model;
    use crate::question::Question;

    fn config(dir: PathBuf) -> OnnxConfig {
        let model = native_model("english").unwrap();
        OnnxConfig::for_model(model, dir, 8192, Duration::from_secs(1), 8)
    }

    #[test]
    fn the_config_pins_every_file_and_caps_max_len() {
        let config = config(PathBuf::from("/models"));
        assert_eq!(config.max_len, 512);
        assert_eq!(config.expected_sha256.len(), 5);
    }

    #[tokio::test]
    async fn a_missing_model_is_unavailable_not_a_panic() {
        let dir = std::env::temp_dir().join("z-engine-onnx-missing-model");
        let provider = OnnxProvider::open(config(dir));
        let error = provider.ready(Duration::from_secs(5)).await.unwrap_err();
        assert!(error.to_string().contains("config.json"), "{error}");
        let quick = provider.with_call_limits(Duration::from_millis(10), 1);
        assert!(quick.config().same_model(provider.config()));
        assert!(
            quick.ready(Duration::ZERO).await.is_err(),
            "shares the failed load"
        );
        let question = Question::yes_no("q", "Ok?\n- yes: Y.\n- no: N.\n").unwrap();
        let request = DecisionRequest::new(json!({})).ask(question);
        let cancel = CancellationToken::new();
        let reply = provider.decide(&request, &cancel).await;
        assert!(matches!(reply, Err(DecisionError::Unavailable(_))));
        assert!(provider.revision().starts_with("onnx:english:"));
    }
}
