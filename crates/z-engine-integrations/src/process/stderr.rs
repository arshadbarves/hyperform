//! A bounded tail of a server's stderr for diagnostics: the last lines, each
//! capped, while the whole stream is drained so the server never blocks on
//! a full pipe.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, BufReader};
use tokio::task::JoinHandle;

use crate::sync::lock;

const MAX_LINES: usize = 50;
const MAX_LINE_BYTES: usize = 512;

/// Shared so the tail survives reconnects of the same server.
#[derive(Debug, Clone, Default)]
pub(crate) struct StderrLog {
    lines: Arc<Mutex<VecDeque<String>>>,
}

impl StderrLog {
    pub(crate) fn tail(&self) -> Vec<String> {
        lock(&self.lines).iter().cloned().collect()
    }

    fn push(&self, line: String) {
        let mut lines = lock(&self.lines);
        if lines.len() == MAX_LINES {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    /// Drains `stream` into the log until end of stream.
    pub(crate) fn spawn_reader<R>(&self, stream: R, program: String) -> JoinHandle<()>
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        let log = self.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stream);
            loop {
                match read_capped_line(&mut reader).await {
                    Ok(Some(line)) => {
                        tracing::trace!(%program, line = %line, "server stderr");
                        log.push(line);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        tracing::debug!(%program, error = %e, "stopped reading server stderr");
                        break;
                    }
                }
            }
        })
    }
}

/// One line, truncated to [`MAX_LINE_BYTES`]; the rest of a long line is
/// consumed and dropped.
async fn read_capped_line<R>(reader: &mut R) -> std::io::Result<Option<String>>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();
    let mut started = false;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(started.then(|| finish(&line)));
        }
        started = true;
        let (take, consumed, done) = match available.iter().position(|&b| b == b'\n') {
            Some(end) => (end, end + 1, true),
            None => (available.len(), available.len(), false),
        };
        let room = MAX_LINE_BYTES.saturating_sub(line.len());
        line.extend_from_slice(&available[..take.min(room)]);
        reader.consume(consumed);
        if done {
            return Ok(Some(finish(&line)));
        }
    }
}

fn finish(line: &[u8]) -> String {
    String::from_utf8_lossy(line)
        .trim_end_matches('\r')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn keeps_a_bounded_tail_of_capped_lines() {
        let mut input = String::new();
        for i in 0..(MAX_LINES + 5) {
            input.push_str(&format!("line {i}\r\n"));
        }
        input.push_str(&"x".repeat(MAX_LINE_BYTES * 3));
        let log = StderrLog::default();
        log.spawn_reader(std::io::Cursor::new(input.into_bytes()), "t".into())
            .await
            .unwrap();
        let tail = log.tail();
        assert_eq!(tail.len(), MAX_LINES);
        assert_eq!(tail[0], "line 6");
        assert_eq!(tail[MAX_LINES - 2], format!("line {}", MAX_LINES + 4));
        assert_eq!(tail[MAX_LINES - 1].len(), MAX_LINE_BYTES);
    }
}
