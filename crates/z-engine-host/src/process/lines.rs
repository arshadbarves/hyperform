//! Line framing for child-process pipes. Lines are delivered with their
//! terminator; an over-long line is split at a character boundary so one
//! newline-free stream cannot grow memory without bound.

use tokio::io::{AsyncRead, AsyncReadExt};

const MAX_LINE_BYTES: usize = 64 * 1024;
const READ_CHUNK: usize = 8 * 1024;

/// Reads `pipe` to EOF, calling `on_line` for every line as it completes.
pub(crate) async fn pump<R>(mut pipe: R, mut on_line: impl FnMut(&str))
where
    R: AsyncRead + Unpin,
{
    let mut splitter = LineSplitter::default();
    let mut buf = vec![0u8; READ_CHUNK];
    loop {
        match pipe.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => splitter.push(&buf[..n], &mut on_line),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                tracing::debug!(error = %e, "pipe read failed");
                break;
            }
        }
    }
    splitter.finish(&mut on_line);
}

#[derive(Debug, Default)]
pub(crate) struct LineSplitter {
    pending: Vec<u8>,
}

impl LineSplitter {
    pub(crate) fn push(&mut self, bytes: &[u8], on_line: &mut impl FnMut(&str)) {
        self.pending.extend_from_slice(bytes);
        let mut start = 0;
        while let Some(pos) = self.pending[start..].iter().position(|&b| b == b'\n') {
            let end = start + pos + 1;
            emit(&self.pending[start..end], on_line);
            start = end;
        }
        self.pending.drain(..start);
        while self.pending.len() >= MAX_LINE_BYTES {
            let cut = char_boundary(&self.pending, MAX_LINE_BYTES);
            emit(&self.pending[..cut], on_line);
            self.pending.drain(..cut);
        }
    }

    pub(crate) fn finish(&mut self, on_line: &mut impl FnMut(&str)) {
        if !self.pending.is_empty() {
            emit(&self.pending, on_line);
            self.pending.clear();
        }
    }
}

fn emit(bytes: &[u8], on_line: &mut impl FnMut(&str)) {
    on_line(&String::from_utf8_lossy(bytes));
}

/// The largest cut at or below `limit` that does not split a UTF-8 sequence.
fn char_boundary(bytes: &[u8], limit: usize) -> usize {
    if limit >= bytes.len() {
        return bytes.len();
    }
    let mut cut = limit;
    while cut > 0 && (bytes[cut] & 0xC0) == 0x80 {
        cut -= 1;
    }
    if cut == 0 { limit } else { cut }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(chunks: &[&[u8]]) -> Vec<String> {
        let mut out = Vec::new();
        let mut splitter = LineSplitter::default();
        let mut sink = |line: &str| out.push(line.to_string());
        for chunk in chunks {
            splitter.push(chunk, &mut sink);
        }
        splitter.finish(&mut sink);
        out
    }

    #[test]
    fn lines_keep_terminators_across_chunks() {
        assert_eq!(
            split(&[b"a\nb", b"c\n", b"tail"]),
            vec!["a\n", "bc\n", "tail"]
        );
    }

    #[test]
    fn long_lines_split_on_char_boundaries() {
        let long = "é".repeat(MAX_LINE_BYTES);
        let lines = split(&[long.as_bytes()]);
        assert!(lines.len() > 1);
        assert_eq!(lines.concat(), long);
        assert!(lines.iter().all(|l| !l.contains('\u{FFFD}')));
    }
}
