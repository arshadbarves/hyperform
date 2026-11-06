//! Message framing on byte streams: one JSON document per line (MCP stdio)
//! or `Content-Length` headers before each body (LSP base protocol). Reads
//! are bounded so a misbehaving server cannot exhaust memory with one
//! endless frame.

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::error::IntegrationError;

/// Largest accepted frame body.
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
/// Largest accepted header line of a `Content-Length` frame.
const MAX_HEADER_LINE: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// One message per `\n`-terminated line (MCP stdio).
    Newline,
    /// `Content-Length: N\r\n\r\n` followed by N bytes (LSP).
    ContentLength,
}

impl Framing {
    /// `body` must be compact JSON (no raw newlines) for [`Framing::Newline`].
    pub fn encode(self, body: &[u8]) -> Vec<u8> {
        let header = match self {
            Self::Newline => String::new(),
            Self::ContentLength => format!("Content-Length: {}\r\n\r\n", body.len()),
        };
        let mut out = Vec::with_capacity(header.len() + body.len() + 1);
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(body);
        if self == Self::Newline {
            out.push(b'\n');
        }
        out
    }
}

/// Writes one framed message and flushes.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    framing: Framing,
    body: &[u8],
) -> std::io::Result<()> {
    writer.write_all(&framing.encode(body)).await?;
    writer.flush().await
}

/// Reads frame bodies from a byte stream.
#[derive(Debug)]
pub struct FrameReader<R> {
    reader: BufReader<R>,
    framing: Framing,
    limit: usize,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
    pub fn new(reader: R, framing: Framing) -> Self {
        Self {
            reader: BufReader::new(reader),
            framing,
            limit: MAX_FRAME_BYTES,
        }
    }

    /// Lowers the largest accepted body (tests, constrained transports).
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// The next frame body; `Ok(None)` when the stream ended between frames.
    pub async fn read_frame(&mut self) -> Result<Option<Vec<u8>>, IntegrationError> {
        match self.framing {
            Framing::Newline => self.read_line_frame().await,
            Framing::ContentLength => self.read_length_frame().await,
        }
    }

    async fn read_line_frame(&mut self) -> Result<Option<Vec<u8>>, IntegrationError> {
        loop {
            let Some(mut line) = self.read_line(self.limit).await? else {
                return Ok(None);
            };
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if !line.iter().all(u8::is_ascii_whitespace) {
                return Ok(Some(line));
            }
        }
    }

    async fn read_length_frame(&mut self) -> Result<Option<Vec<u8>>, IntegrationError> {
        let mut length = None;
        let mut in_header = false;
        loop {
            let Some(mut line) = self.read_line(MAX_HEADER_LINE).await? else {
                if in_header {
                    return Err(IntegrationError::Disconnected(
                        "the stream ended inside a message header".into(),
                    ));
                }
                return Ok(None);
            };
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                if in_header {
                    break;
                }
                continue;
            }
            in_header = true;
            let text = String::from_utf8_lossy(&line);
            let Some((name, value)) = text.split_once(':') else {
                return Err(IntegrationError::Protocol(format!(
                    "malformed header line `{text}`"
                )));
            };
            if name.trim().eq_ignore_ascii_case("content-length") {
                let value = value.trim();
                length = Some(value.parse::<usize>().map_err(|_| {
                    IntegrationError::Protocol(format!("invalid Content-Length `{value}`"))
                })?);
            }
        }
        let length = length.ok_or_else(|| {
            IntegrationError::Protocol("a message header has no Content-Length".into())
        })?;
        if length > self.limit {
            return Err(IntegrationError::Protocol(format!(
                "a {length}-byte message exceeds the {}-byte limit",
                self.limit
            )));
        }
        let mut body = vec![0; length];
        self.reader.read_exact(&mut body).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                IntegrationError::Disconnected("the stream ended inside a message body".into())
            } else {
                IntegrationError::io("reading from the server", e)
            }
        })?;
        Ok(Some(body))
    }

    /// One `\n`-terminated line without the terminator. A final unterminated
    /// line is returned as is; `None` at end of stream.
    async fn read_line(&mut self, limit: usize) -> Result<Option<Vec<u8>>, IntegrationError> {
        let mut line = Vec::new();
        loop {
            let available = self
                .reader
                .fill_buf()
                .await
                .map_err(|e| IntegrationError::io("reading from the server", e))?;
            if available.is_empty() {
                return Ok((!line.is_empty()).then_some(line));
            }
            let (take, consumed, done) = match available.iter().position(|&b| b == b'\n') {
                Some(end) => (end, end + 1, true),
                None => (available.len(), available.len(), false),
            };
            if line.len() + take > limit {
                return Err(IntegrationError::Protocol(format!(
                    "a frame exceeds the {limit}-byte limit"
                )));
            }
            line.extend_from_slice(&available[..take]);
            self.reader.consume(consumed);
            if done {
                return Ok(Some(line));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;

    use super::*;

    async fn frames(input: &[u8], framing: Framing) -> Vec<Result<Vec<u8>, String>> {
        let mut reader = FrameReader::new(input, framing);
        let mut out = Vec::new();
        loop {
            match reader.read_frame().await {
                Ok(Some(frame)) => out.push(Ok(frame)),
                Ok(None) => return out,
                Err(e) => {
                    out.push(Err(e.to_string()));
                    return out;
                }
            }
        }
    }

    #[test]
    fn encodes_both_framings() {
        assert_eq!(Framing::Newline.encode(b"{}"), b"{}\n");
        assert_eq!(
            Framing::ContentLength.encode("{\"é\":1}".as_bytes()),
            b"Content-Length: 8\r\n\r\n{\"\xc3\xa9\":1}"
        );
    }

    #[tokio::test]
    async fn newline_frames_skip_blank_lines_and_strip_cr() {
        let got = frames(
            b"{\"a\":1}\r\n\n   \n{\"b\":2}\n{\"c\":3}",
            Framing::Newline,
        )
        .await;
        let bodies: Vec<_> = got.into_iter().map(Result::unwrap).collect();
        assert_eq!(
            bodies,
            vec![
                b"{\"a\":1}".to_vec(),
                b"{\"b\":2}".to_vec(),
                b"{\"c\":3}".to_vec()
            ]
        );
    }

    #[tokio::test]
    async fn content_length_frames_tolerate_extra_headers_and_case() {
        let input = b"content-length: 2\r\nContent-Type: application/vscode-jsonrpc\r\n\r\n{}\
Content-Length: 7\r\n\r\n[1,2,3]";
        let got = frames(input, Framing::ContentLength).await;
        assert_eq!(got, vec![Ok(b"{}".to_vec()), Ok(b"[1,2,3]".to_vec())]);
    }

    #[tokio::test]
    async fn content_length_errors_are_typed() {
        let missing = frames(b"X-Other: 1\r\n\r\n{}", Framing::ContentLength).await;
        assert!(matches!(&missing[..], [Err(e)] if e.contains("no Content-Length")));
        let truncated = frames(b"Content-Length: 10\r\n\r\n{}", Framing::ContentLength).await;
        assert!(matches!(&truncated[..], [Err(e)] if e.contains("inside a message body")));
        let header_eof = frames(b"Content-Length: 10\r\n", Framing::ContentLength).await;
        assert!(matches!(&header_eof[..], [Err(e)] if e.contains("inside a message header")));
    }

    #[tokio::test]
    async fn oversized_frames_are_rejected() {
        let mut reader = FrameReader::new(&b"0123456789\n"[..], Framing::Newline).with_limit(4);
        assert!(matches!(
            reader.read_frame().await,
            Err(IntegrationError::Protocol(_))
        ));
        let body = b"Content-Length: 100\r\n\r\n";
        let mut reader = FrameReader::new(&body[..], Framing::ContentLength).with_limit(10);
        assert!(matches!(
            reader.read_frame().await,
            Err(IntegrationError::Protocol(_))
        ));
    }

    #[tokio::test]
    async fn frames_split_into_single_bytes_reassemble() {
        let (mut tx, rx) = tokio::io::duplex(1);
        let input = [
            Framing::ContentLength.encode("{\"x\":\"🦀\"}".as_bytes()),
            Framing::ContentLength.encode(b"null"),
        ]
        .concat();
        let writer = tokio::spawn(async move {
            for byte in input {
                tx.write_all(&[byte]).await.unwrap();
            }
        });
        let mut reader = FrameReader::new(rx, Framing::ContentLength);
        let first = reader.read_frame().await.unwrap().unwrap();
        assert_eq!(first, "{\"x\":\"🦀\"}".as_bytes());
        assert_eq!(reader.read_frame().await.unwrap().unwrap(), b"null");
        writer.await.unwrap();
        assert!(reader.read_frame().await.unwrap().is_none());
    }
}
