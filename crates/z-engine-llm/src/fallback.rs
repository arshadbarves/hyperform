//! Model fallback: when a response fails with a retryable error before any
//! content was streamed, the request is replayed on the next fallback model.
//! Once content has reached the consumer, a failure is final.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::client::ModelClient;
use crate::error::LlmError;
use crate::types::{ModelEvent, ModelRequest, ModelStream};

const CHANNEL_CAPACITY: usize = 64;

/// A [`ModelClient`] that tries `fallbacks` (model ids, in order) after the
/// requested model fails transiently. Each switch is reported as
/// `ModelEvent::Retrying` with a zero delay.
#[derive(Debug, Clone)]
pub struct FallbackClient {
    inner: Arc<dyn ModelClient>,
    fallbacks: Vec<String>,
}

impl FallbackClient {
    pub fn new(inner: Arc<dyn ModelClient>, fallbacks: Vec<String>) -> Self {
        Self { inner, fallbacks }
    }
}

impl ModelClient for FallbackClient {
    fn stream(&self, request: ModelRequest, cancel: CancellationToken) -> ModelStream {
        if self.fallbacks.is_empty() {
            return self.inner.stream(request, cancel);
        }
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let inner = Arc::clone(&self.inner);
        let fallbacks = self.fallbacks.clone();
        tokio::spawn(async move {
            tokio::select! {
                () = tx.closed() => {}
                () = run(inner, fallbacks, request, &tx, &cancel) => {}
            }
        });
        rx
    }

    fn provider(&self) -> &str {
        self.inner.provider()
    }
}

async fn run(
    inner: Arc<dyn ModelClient>,
    fallbacks: Vec<String>,
    mut request: ModelRequest,
    tx: &mpsc::Sender<Result<ModelEvent, LlmError>>,
    cancel: &CancellationToken,
) {
    let mut remaining = fallbacks.into_iter();
    let mut attempt = 0;
    loop {
        let mut stream = inner.stream(request.clone(), cancel.clone());
        let mut streamed_content = false;
        let error = loop {
            match stream.recv().await {
                None => return,
                Some(Ok(event)) => {
                    streamed_content |= is_content(&event);
                    if tx.send(Ok(event)).await.is_err() {
                        return;
                    }
                }
                Some(Err(error)) => break error,
            }
        };
        let replayable = !streamed_content && error.is_retryable() && !cancel.is_cancelled();
        let Some(model) = remaining.next().filter(|_| replayable) else {
            if tx.send(Err(error)).await.is_err() {
                tracing::debug!("fallback consumer gone before the final error");
            }
            return;
        };
        attempt += 1;
        tracing::warn!(from = %request.model, to = %model, %error, "falling back to another model");
        let reason = format!("falling back to {model}: {error}");
        let notice = ModelEvent::Retrying {
            attempt,
            delay_ms: 0,
            reason,
        };
        if tx.send(Ok(notice)).await.is_err() {
            return;
        }
        request.model = model;
    }
}

fn is_content(event: &ModelEvent) -> bool {
    matches!(
        event,
        ModelEvent::TextDelta(_)
            | ModelEvent::ThinkingDelta(_)
            | ModelEvent::ToolUseStart { .. }
            | ModelEvent::RedactedThinking(_)
    )
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use z_engine_protocol::Message;

    use super::*;
    use crate::types::StopReason;

    type Script = Vec<Result<ModelEvent, LlmError>>;

    #[derive(Debug, Default)]
    struct Scripted {
        models: Mutex<Vec<String>>,
        scripts: Mutex<VecDeque<Script>>,
    }

    impl ModelClient for Scripted {
        fn stream(&self, request: ModelRequest, _cancel: CancellationToken) -> ModelStream {
            self.models.lock().unwrap().push(request.model);
            let script = self.scripts.lock().unwrap().pop_front().unwrap_or_default();
            let (tx, rx) = mpsc::channel(script.len().max(1));
            for item in script {
                tx.try_send(item).unwrap();
            }
            rx
        }
    }

    async fn run_scripts(scripts: Vec<Script>, fallbacks: &[&str]) -> (Script, Vec<String>) {
        let inner = Arc::new(Scripted {
            scripts: Mutex::new(scripts.into()),
            ..Scripted::default()
        });
        let client = FallbackClient::new(
            inner.clone(),
            fallbacks.iter().map(|model| (*model).to_string()).collect(),
        );
        let request = ModelRequest::new("primary", vec![Message::user_text("hi")]);
        let mut stream = client.stream(request, CancellationToken::new());
        let mut items = Vec::new();
        while let Some(item) = stream.recv().await {
            items.push(item);
        }
        let models = inner.models.lock().unwrap().clone();
        (items, models)
    }

    fn overloaded() -> LlmError {
        LlmError::Overloaded {
            attempts: 5,
            detail: "busy".into(),
        }
    }

    #[tokio::test]
    async fn retryable_error_before_content_falls_back() {
        let inner_retry = ModelEvent::Retrying {
            attempt: 1,
            delay_ms: 500,
            reason: "rate limited (HTTP 429)".into(),
        };
        let (items, models) = run_scripts(
            vec![
                vec![Ok(inner_retry.clone()), Err(overloaded())],
                vec![
                    Ok(ModelEvent::TextDelta("ok".into())),
                    Ok(ModelEvent::Stop(StopReason::EndTurn)),
                ],
            ],
            &["backup"],
        )
        .await;
        assert_eq!(models, ["primary", "backup"]);
        assert_eq!(items[0], Ok(inner_retry));
        match &items[1] {
            Ok(ModelEvent::Retrying {
                attempt: 1,
                delay_ms: 0,
                reason,
            }) => assert!(reason.starts_with("falling back to backup: provider overloaded")),
            other => panic!("expected a fallback notice, got {other:?}"),
        }
        assert_eq!(items[3], Ok(ModelEvent::Stop(StopReason::EndTurn)));
    }

    #[tokio::test]
    async fn failure_after_content_is_final() {
        let broken = LlmError::Stream("reset".into());
        let (items, models) = run_scripts(
            vec![vec![
                Ok(ModelEvent::TextDelta("par".into())),
                Err(broken.clone()),
            ]],
            &["backup"],
        )
        .await;
        assert_eq!(models, ["primary"]);
        assert_eq!(
            items,
            [Ok(ModelEvent::TextDelta("par".into())), Err(broken)]
        );
    }

    #[tokio::test]
    async fn non_retryable_errors_and_exhausted_chains_surface() {
        let denied = LlmError::Http {
            status: 401,
            body: "bad key".into(),
        };
        let (items, models) = run_scripts(vec![vec![Err(denied.clone())]], &["backup"]).await;
        assert_eq!(
            (items, models),
            (vec![Err(denied)], vec!["primary".to_string()])
        );
        let (items, models) = run_scripts(
            vec![vec![Err(overloaded())], vec![Err(overloaded())]],
            &["backup"],
        )
        .await;
        assert_eq!(models, ["primary", "backup"]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[1], Err(overloaded()));
    }
}
