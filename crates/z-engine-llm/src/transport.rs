//! Runs one streaming request: send with retries, decode SSE, normalize the
//! events with a provider parser, and stop promptly when the caller cancels
//! or drops the receiver.

use reqwest::header::{CONTENT_TYPE, HeaderMap};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::LlmError;
use crate::retry::{RetryNotice, RetryPolicy, describe, send_with_retry};
use crate::sse::{SseDecoder, SseEvent};
use crate::types::{ModelEvent, ModelStream};
use crate::zen;

pub(crate) const CHANNEL_CAPACITY: usize = 64;

pub(crate) type StreamItem = Result<ModelEvent, LlmError>;

/// Normalizes one provider's server-sent events into [`ModelEvent`]s.
pub(crate) trait StreamParser: Send + 'static {
    /// Handle one event. A `Stop` or an error item ends the stream.
    fn push(&mut self, event: &SseEvent) -> Vec<StreamItem>;
    /// The body ended: emit the pending stop or report truncation.
    fn finish(&mut self) -> Vec<StreamItem>;
}

/// Where and how an adapter sends its requests.
#[derive(Debug, Clone)]
pub(crate) struct Endpoint {
    pub http: reqwest::Client,
    pub url: String,
    /// Static headers; credentials are marked sensitive.
    pub headers: HeaderMap,
    pub label: &'static str,
    /// Stable OpenCode Zen session id; `None` for other gateways.
    pub zen_session: Option<String>,
    pub policy: RetryPolicy,
}

impl Endpoint {
    /// POST `body` and stream the response through `parser`.
    pub fn stream<P: StreamParser>(
        &self,
        body: &Value,
        extra_headers: HeaderMap,
        session_key: Option<&str>,
        parser: P,
        cancel: CancellationToken,
    ) -> ModelStream {
        let body = body.to_string().into_bytes();
        let mut headers = self.headers.clone();
        for (name, value) in &extra_headers {
            headers.append(name, value.clone());
        }
        let session = self.zen_session.as_ref().map(|stable| {
            session_key
                .and_then(zen::session_id_for)
                .unwrap_or_else(|| stable.clone())
        });
        let http = self.http.clone();
        let url = self.url.clone();
        let build = move || {
            let request = http.post(&url).headers(headers.clone()).body(body.clone());
            match &session {
                Some(session) => zen::apply_headers(request, session),
                None => request,
            }
        };
        spawn_stream(build, self.policy, parser, cancel)
    }
}

fn spawn_stream<B, P>(
    build: B,
    policy: RetryPolicy,
    parser: P,
    cancel: CancellationToken,
) -> ModelStream
where
    B: Fn() -> reqwest::RequestBuilder + Send + Sync + 'static,
    P: StreamParser,
{
    let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
    tokio::spawn(async move {
        tokio::select! {
            () = tx.closed() => {}
            () = cancel.cancelled() => {
                if tx.try_send(Err(LlmError::Cancelled)).is_err() {
                    tracing::debug!("stream consumer gone or full at cancellation");
                }
            }
            () = run(&build, &policy, parser, &tx, &cancel) => {}
        }
    });
    rx
}

async fn run<B, P>(
    build: &B,
    policy: &RetryPolicy,
    mut parser: P,
    tx: &mpsc::Sender<StreamItem>,
    cancel: &CancellationToken,
) where
    B: Fn() -> reqwest::RequestBuilder,
    P: StreamParser,
{
    let notify = |notice: RetryNotice| {
        let tx = tx.clone();
        async move {
            if tx.send(Ok(notice.into())).await.is_err() {
                tracing::debug!("retry notice dropped: consumer gone");
            }
        }
    };
    let mut response = match send_with_retry(build, policy, cancel, notify).await {
        Ok(response) => response,
        Err(error) => {
            forward(tx, vec![Err(error)]).await;
            return;
        }
    };
    if is_json(&response) {
        // A gateway that answered without streaming: parse the body as one event.
        let items = match response.text().await {
            Ok(data) => {
                let mut items = parser.push(&SseEvent { event: None, data });
                if !items.iter().any(is_terminal) {
                    items.extend(parser.finish());
                }
                items
            }
            Err(error) => vec![Err(LlmError::Stream(describe(&error)))],
        };
        forward(tx, items).await;
        return;
    }
    let mut decoder = SseDecoder::new();
    loop {
        match response.chunk().await {
            Ok(Some(bytes)) => {
                for event in decoder.feed(&bytes) {
                    if forward(tx, parser.push(&event)).await {
                        return;
                    }
                }
            }
            Ok(None) => break,
            Err(error) => {
                forward(tx, vec![Err(LlmError::Stream(describe(&error)))]).await;
                return;
            }
        }
    }
    for event in decoder.finish() {
        if forward(tx, parser.push(&event)).await {
            return;
        }
    }
    forward(tx, parser.finish()).await;
}

/// Send `items` in order; true once a terminal item went out or the
/// consumer is gone.
async fn forward(tx: &mpsc::Sender<StreamItem>, items: Vec<StreamItem>) -> bool {
    for item in items {
        let terminal = is_terminal(&item);
        if tx.send(item).await.is_err() || terminal {
            return true;
        }
    }
    false
}

fn is_terminal(item: &StreamItem) -> bool {
    matches!(item, Ok(ModelEvent::Stop(_)) | Err(_))
}

fn is_json(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim_start().starts_with("application/json"))
}

/// Run a whole SSE body through `parser` as the driver would.
#[cfg(test)]
pub(crate) fn parse_all<P: StreamParser>(mut parser: P, body: &str) -> Vec<StreamItem> {
    let mut decoder = SseDecoder::new();
    let events = decoder.feed(body.as_bytes());
    let mut out = Vec::new();
    for event in events.iter().chain(decoder.finish().iter()) {
        for item in parser.push(event) {
            let terminal = is_terminal(&item);
            out.push(item);
            if terminal {
                return out;
            }
        }
    }
    out.extend(parser.finish());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_and_errors_are_terminal() {
        use crate::types::StopReason;
        assert!(is_terminal(&Ok(ModelEvent::Stop(StopReason::EndTurn))));
        assert!(is_terminal(&Err(LlmError::Cancelled)));
        assert!(!is_terminal(&Ok(ModelEvent::TextDelta("x".into()))));
    }
}
