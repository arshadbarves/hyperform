//! Minimal `text/event-stream` decoding for MCP streamable HTTP. Only event
//! data matters: `data:` lines of one event join with `\n`, a blank line
//! dispatches, comments and other fields are ignored, and input may split
//! anywhere (a line is decoded only once its `\n` arrived). Bounded so an
//! endless event cannot exhaust memory. Each event's data is one JSON-RPC
//! message (or batch).

use std::collections::VecDeque;
use std::pin::Pin;

use futures::{Stream, StreamExt};

use crate::error::IntegrationError;
use crate::jsonrpc::{Inbound, RpcMessage};

pub(crate) const MAX_EVENT_BYTES: usize = 64 * 1024 * 1024;

type Messages = VecDeque<Result<RpcMessage, IntegrationError>>;

struct SseState<S> {
    bytes: Pin<Box<S>>,
    decoder: SseDecoder,
    queue: Messages,
    done: bool,
}

/// The JSON-RPC messages carried by a stream of SSE body chunks. A chunk
/// error or an oversized event ends the stream with that error.
pub(crate) fn sse_messages<S, B>(bytes: S) -> Inbound
where
    S: Stream<Item = Result<B, IntegrationError>> + Send + 'static,
    B: AsRef<[u8]> + Send,
{
    let state = SseState {
        bytes: Box::pin(bytes),
        decoder: SseDecoder::default(),
        queue: VecDeque::new(),
        done: false,
    };
    futures::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(item) = state.queue.pop_front() {
                return Some((item, state));
            }
            if state.done {
                return None;
            }
            let fed = match state.bytes.next().await {
                Some(Ok(chunk)) => state.decoder.feed(chunk.as_ref()),
                Some(Err(e)) => Err(e),
                None => {
                    state.done = true;
                    Ok(state.decoder.finish())
                }
            };
            match fed {
                Ok(events) => enqueue(&mut state.queue, &events),
                Err(e) => {
                    state.done = true;
                    state.queue.push_back(Err(e));
                }
            }
        }
    })
    .boxed()
}

fn enqueue(queue: &mut Messages, events: &[String]) {
    for data in events.iter().filter(|data| !data.trim().is_empty()) {
        queue.extend(RpcMessage::parse_frame(data.as_bytes()).into_iter().map(Ok));
    }
}

#[derive(Debug, Default)]
pub(crate) struct SseDecoder {
    line: Vec<u8>,
    data: String,
    has_data: bool,
}

impl SseDecoder {
    /// Feeds bytes; returns the data of every event they complete.
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Result<Vec<String>, IntegrationError> {
        let mut events = Vec::new();
        let mut rest = bytes;
        while let Some(end) = rest.iter().position(|&b| b == b'\n') {
            self.line.extend_from_slice(&rest[..end]);
            let line = std::mem::take(&mut self.line);
            self.process(&line, &mut events);
            rest = &rest[end + 1..];
        }
        self.line.extend_from_slice(rest);
        if self.line.len() + self.data.len() > MAX_EVENT_BYTES {
            return Err(IntegrationError::Protocol(format!(
                "an event exceeds the {MAX_EVENT_BYTES}-byte limit"
            )));
        }
        Ok(events)
    }

    /// Flushes an unterminated final line and event at end of stream.
    pub(crate) fn finish(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        if !self.line.is_empty() {
            let line = std::mem::take(&mut self.line);
            self.process(&line, &mut events);
        }
        self.dispatch(&mut events);
        events
    }

    fn process(&mut self, raw: &[u8], events: &mut Vec<String>) {
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        if raw.is_empty() {
            self.dispatch(events);
            return;
        }
        let line = String::from_utf8_lossy(raw);
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_ref(), ""),
        };
        if field == "data" {
            if self.has_data {
                self.data.push('\n');
            }
            self.data.push_str(value);
            self.has_data = true;
        }
    }

    fn dispatch(&mut self, events: &mut Vec<String>) {
        if std::mem::take(&mut self.has_data) {
            events.push(std::mem::take(&mut self.data));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(input: &str) -> Vec<String> {
        let mut decoder = SseDecoder::default();
        let mut events = decoder.feed(input.as_bytes()).unwrap();
        events.extend(decoder.finish());
        events
    }

    #[test]
    fn decodes_events_comments_and_multiline_data() {
        let input = ": keep-alive\nevent: message\nid: 1\ndata: {\"a\":1}\n\ndata: x\ndata:y\n\n";
        assert_eq!(
            decode(input),
            vec!["{\"a\":1}".to_string(), "x\ny".to_string()]
        );
        assert_eq!(decode(&input.replace('\n', "\r\n")), decode(input));
        assert!(decode("event: ping\n\nretry: 5\n\n").is_empty());
        assert_eq!(decode("data: tail"), vec!["tail".to_string()]);
    }

    #[test]
    fn bytewise_input_matches_wholesale_input() {
        let input = "data: {\"t\":\"🦀 é\"}\n\n: c\ndata: 2\n\n";
        let mut decoder = SseDecoder::default();
        let mut events = Vec::new();
        for byte in input.as_bytes() {
            events.extend(decoder.feed(std::slice::from_ref(byte)).unwrap());
        }
        events.extend(decoder.finish());
        assert_eq!(events, decode(input));
    }

    #[tokio::test]
    async fn streams_become_messages_and_end_on_errors() {
        let chunks: Vec<Result<&[u8], IntegrationError>> = vec![
            Ok(b": hi\n\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"n\"}\n"),
            Ok(b"\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":7}\n\ndata: [1"),
            Err(IntegrationError::Disconnected("reset".into())),
            Ok(b"data: {\"id\":2,\"result\":0}\n\n"),
        ];
        let items: Vec<_> = sse_messages(futures::stream::iter(chunks)).collect().await;
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], Ok(RpcMessage::Notification(n)) if n.method == "n"));
        assert!(matches!(&items[1], Ok(RpcMessage::Response(_))));
        assert!(matches!(&items[2], Err(IntegrationError::Disconnected(_))));
        let tail: Vec<Result<&[u8], IntegrationError>> = vec![Ok(b"data: {\"id\":3,\"result\":1}")];
        let items: Vec<_> = sse_messages(futures::stream::iter(tail)).collect().await;
        assert!(
            matches!(&items[..], [Ok(RpcMessage::Response(_))]),
            "flushed at the end"
        );
    }

    #[test]
    fn endless_lines_are_rejected() {
        let mut decoder = SseDecoder::default();
        let chunk = vec![b'x'; 1024 * 1024];
        let mut result = Ok(Vec::new());
        for _ in 0..=(MAX_EVENT_BYTES / chunk.len()) {
            result = decoder.feed(&chunk);
            if result.is_err() {
                break;
            }
        }
        assert!(matches!(result, Err(IntegrationError::Protocol(_))));
    }
}
