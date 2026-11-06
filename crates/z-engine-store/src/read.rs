//! Tolerant JSONL reading shared by session logs, agent transcripts, plan
//! logs, and v1 files. Each line is decoded on its own, so a corrupt or unknown record
//! costs one line, not the session. Reading never writes to the file.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::de::DeserializeOwned;
use serde_json::error::Category;

use crate::error::StoreError;
use crate::record::LogRecord;

/// Records of one log plus what had to be skipped to read them.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadOutcome {
    pub records: Vec<LogRecord>,
    /// The final line was an unterminated fragment that is not valid JSON
    /// (a write cut short by a crash). It was ignored.
    pub torn_tail: bool,
    /// Lines skipped because they were not JSON or not a known record.
    pub corrupt_lines: usize,
}

/// Read every record of a v2 log (a session log or an agent transcript).
pub fn read_records(path: &Path) -> Result<ReadOutcome, StoreError> {
    let lines = read_jsonl(path)?;
    Ok(ReadOutcome {
        records: lines.items,
        torn_tail: lines.torn_tail,
        corrupt_lines: lines.corrupt_lines,
    })
}

#[derive(Debug)]
pub(crate) struct JsonLines<T> {
    pub(crate) items: Vec<T>,
    pub(crate) torn_tail: bool,
    pub(crate) corrupt_lines: usize,
}

/// How one line decodes.
#[derive(Debug)]
pub(crate) enum Line<T> {
    Blank,
    Item(T),
    /// Not valid JSON (possibly cut short).
    Malformed(String),
    /// Valid JSON, but not a `T` (for example an unknown record kind).
    Unrecognized(String),
}

pub(crate) fn parse_line<T: DeserializeOwned>(bytes: &[u8]) -> Line<T> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Line::Blank;
    }
    match serde_json::from_slice(bytes) {
        Ok(item) => Line::Item(item),
        Err(error) if error.classify() == Category::Data => Line::Unrecognized(error.to_string()),
        Err(error) => Line::Malformed(error.to_string()),
    }
}

pub(crate) fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<JsonLines<T>, StoreError> {
    let file = File::open(path).map_err(|error| StoreError::open(path, error))?;
    let mut reader = BufReader::new(file);
    let mut out = JsonLines {
        items: Vec::new(),
        torn_tail: false,
        corrupt_lines: 0,
    };
    let mut line = Vec::new();
    let mut number = 0usize;
    loop {
        line.clear();
        let read = reader
            .read_until(b'\n', &mut line)
            .map_err(|error| StoreError::io(path, error))?;
        if read == 0 {
            break;
        }
        number += 1;
        // Only the final line can lack its newline.
        let terminated = line.last() == Some(&b'\n');
        match parse_line(&line) {
            Line::Blank => {}
            Line::Item(item) => out.items.push(item),
            Line::Malformed(reason) if !terminated => {
                tracing::warn!(path = %path.display(), line = number, %reason, "ignoring torn final record");
                out.torn_tail = true;
            }
            Line::Malformed(reason) | Line::Unrecognized(reason) => {
                tracing::warn!(path = %path.display(), line = number, %reason, "skipping unreadable record");
                out.corrupt_lines += 1;
            }
        }
    }
    Ok(out)
}
