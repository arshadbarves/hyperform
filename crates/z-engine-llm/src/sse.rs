//! Incremental `text/event-stream` decoding shared by both adapters.
//!
//! Events end at a blank line; several `data:` lines in one event join with
//! `\n`; `:` lines are comments (OpenRouter keep-alives); `\r\n` endings are
//! tolerated. Input may be split anywhere, even inside a UTF-8 sequence,
//! because a line is only decoded once its terminating `\n` has arrived.

/// One dispatched server-sent event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// The `event:` field; Anthropic names every event, OpenAI omits it.
    pub event: Option<String>,
    pub data: String,
}

impl SseEvent {
    /// The OpenAI-style `data: [DONE]` end-of-stream sentinel.
    pub fn is_done(&self) -> bool {
        self.data.trim() == "[DONE]"
    }
}

/// Byte-level decoder: feed network chunks, receive completed events.
#[derive(Debug, Default)]
pub struct SseDecoder {
    /// Bytes of the current line; never contains `\n`.
    line: Vec<u8>,
    event: Option<String>,
    data: String,
    has_data: bool,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed arbitrary bytes; returns the events they complete.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        let mut out = Vec::new();
        let mut rest = bytes;
        while let Some(end) = rest.iter().position(|&byte| byte == b'\n') {
            self.line.extend_from_slice(&rest[..end]);
            let mut line = std::mem::take(&mut self.line);
            self.process_line(&line, &mut out);
            line.clear();
            self.line = line;
            rest = &rest[end + 1..];
        }
        self.line.extend_from_slice(rest);
        out
    }

    /// Flush an unterminated line and event at end of input.
    pub fn finish(&mut self) -> Vec<SseEvent> {
        let mut out = Vec::new();
        if !self.line.is_empty() {
            let line = std::mem::take(&mut self.line);
            self.process_line(&line, &mut out);
        }
        self.dispatch(&mut out);
        out
    }

    fn process_line(&mut self, raw: &[u8], out: &mut Vec<SseEvent>) {
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        if raw.is_empty() {
            self.dispatch(out);
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
        match field {
            "data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                self.has_data = true;
            }
            "event" => self.event = (!value.is_empty()).then(|| value.to_string()),
            // `id` and `retry` do not affect decoding.
            _ => {}
        }
    }

    fn dispatch(&mut self, out: &mut Vec<SseEvent>) {
        let event = self.event.take();
        if self.has_data {
            self.has_data = false;
            out.push(SseEvent {
                event,
                data: std::mem::take(&mut self.data),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = include_str!("../tests/fixtures/sse/text.sse");
    const KEEPALIVE: &str = include_str!("../tests/fixtures/sse/keepalive_comments.sse");
    const FIXTURES: &[&str] = &[
        TEXT,
        KEEPALIVE,
        include_str!("../tests/fixtures/sse/tool_single.sse"),
        include_str!("../tests/fixtures/sse/tool_multi_interleaved.sse"),
        include_str!("../tests/fixtures/sse/usage_final.sse"),
        include_str!("../tests/fixtures/sse/malformed_tool_json.sse"),
        include_str!("../tests/fixtures/sse/anthropic_thinking_tool.sse"),
    ];

    fn decode_all(input: &str) -> Vec<SseEvent> {
        let mut decoder = SseDecoder::new();
        let mut events = decoder.feed(input.as_bytes());
        events.extend(decoder.finish());
        events
    }

    fn decode_bytewise(input: &str) -> Vec<SseEvent> {
        let mut decoder = SseDecoder::new();
        let mut events = Vec::new();
        for byte in input.as_bytes() {
            events.extend(decoder.feed(std::slice::from_ref(byte)));
        }
        events.extend(decoder.finish());
        events
    }

    #[test]
    fn text_fixture_yields_data_events_then_done() {
        let events = decode_all(TEXT);
        assert_eq!(events.len(), 5);
        assert!(events.iter().all(|event| event.event.is_none()));
        assert!(events[1].data.contains(r#""content":"Hello""#));
        assert!(!events[3].is_done());
        assert!(events[4].is_done());
    }

    #[test]
    fn bytewise_feeding_matches_wholesale_feeding() {
        for fixture in FIXTURES {
            assert_eq!(decode_bytewise(fixture), decode_all(fixture));
        }
    }

    #[test]
    fn crlf_line_endings_are_tolerated() {
        for fixture in FIXTURES {
            assert_eq!(
                decode_all(&fixture.replace('\n', "\r\n")),
                decode_all(fixture)
            );
        }
    }

    #[test]
    fn keepalive_comments_are_ignored() {
        let events = decode_all(KEEPALIVE);
        assert_eq!(events.len(), 4);
        assert!(
            events
                .iter()
                .all(|event| !event.data.contains("OPENROUTER"))
        );
    }

    #[test]
    fn multi_line_data_joins_with_newline() {
        let events = decode_all("data: first\ndata:second\ndata\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "first\nsecond\n");
    }

    #[test]
    fn event_names_are_captured_and_reset_per_event() {
        let events = decode_all("event: ping\ndata: {}\n\ndata: plain\n\n");
        assert_eq!(events[0].event.as_deref(), Some("ping"));
        assert_eq!(events[1].event, None);
    }

    #[test]
    fn event_without_data_is_not_dispatched() {
        assert!(decode_all("event: noop\n\nid: 7\nretry: 100\n\n").is_empty());
    }

    #[test]
    fn utf8_sequence_split_across_chunks_is_reassembled() {
        let input = "data: h\u{e9}llo \u{1f980}\n\n".as_bytes();
        let mut decoder = SseDecoder::new();
        let mut events = decoder.feed(&input[..9]);
        events.extend(decoder.feed(&input[9..14]));
        events.extend(decoder.feed(&input[14..]));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "h\u{e9}llo \u{1f980}");
    }

    #[test]
    fn finish_flushes_an_unterminated_event() {
        let mut decoder = SseDecoder::new();
        assert!(decoder.feed(b"data: [DONE]").is_empty());
        let events = decoder.finish();
        assert_eq!(events.len(), 1);
        assert!(events[0].is_done());
        assert!(decoder.finish().is_empty());
    }
}
