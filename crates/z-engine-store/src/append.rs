//! Typed JSONL appender behind `SessionLog`, `AgentLog` and `PlanLog`: one
//! flushed JSON line per record, tail repair on open, and a write failure
//! shared by every in-process handle to the same file until a reopen
//! repairs it.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use serde::Serialize;

use crate::durable::ensure_same_file;
use crate::error::StoreError;
use crate::read::{Line, parse_line};

const TAIL_CHUNK: u64 = 64 * 1024;

/// Shared by every handle to one file. Its mutex also serializes their
/// writes, so a reopening handle never repairs a line another is writing.
#[derive(Debug, Default)]
struct FileState {
    /// Set by a failed write or sync: the file may end mid-line, so records
    /// are refused until a reopen repairs the tail.
    failure: Option<String>,
}

type SharedState = Arc<Mutex<FileState>>;

#[derive(Debug)]
pub(crate) struct Appender {
    file: File,
    path: PathBuf,
    state: SharedState,
}

impl Appender {
    /// Create a new log file; fails if one already exists.
    pub(crate) fn create_new(path: &Path) -> Result<Self, StoreError> {
        let state = shared_state(path)?;
        let file = {
            let _guard = lock(&state)?;
            OpenOptions::new()
                .append(true)
                .create_new(true)
                .open(path)
                .map_err(|error| StoreError::io(path, error))?
        };
        Ok(Self {
            file,
            path: path.to_path_buf(),
            state,
        })
    }

    /// Open a log for appending (creating it when `create` is set) after
    /// leaving it at a line boundary, which clears a pending failure.
    pub(crate) fn open(path: &Path, create: bool) -> Result<Self, StoreError> {
        let state = shared_state(path)?;
        let file = {
            let mut guard = lock(&state)?;
            let mut file = OpenOptions::new()
                .read(true)
                .append(true)
                .create(create)
                .open(path)
                .map_err(|error| StoreError::open(path, error))?;
            repair_tail(&mut file, path)?;
            guard.failure = None;
            file
        };
        Ok(Self {
            file,
            path: path.to_path_buf(),
            state,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn append<T: Serialize>(&mut self, record: &T) -> Result<(), StoreError> {
        let mut line =
            serde_json::to_vec(record).map_err(|error| StoreError::json(&self.path, 0, &error))?;
        line.push(b'\n');
        let (file, path) = (&mut self.file, &self.path);
        with_healthy(&self.state, path, || {
            ensure_same_file(file, path)?;
            file.write_all(&line)
                .and_then(|()| file.flush())
                .map_err(|error| StoreError::io(path, error))
        })
    }

    pub(crate) fn sync(&mut self) -> Result<(), StoreError> {
        let (file, path) = (&mut self.file, &self.path);
        with_healthy(&self.state, path, || {
            ensure_same_file(file, path)?;
            file.sync_all().map_err(|error| StoreError::io(path, error))
        })
    }
}

/// Run `write` under the file's lock unless a failure is pending; a failure
/// of `write` becomes pending for every handle to the file.
fn with_healthy(
    state: &SharedState,
    path: &Path,
    write: impl FnOnce() -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    let mut state = lock(state)?;
    if let Some(failure) = &state.failure {
        return Err(StoreError::Invalid(format!(
            "{} is unusable after an earlier failure ({failure}); reopen the log",
            path.display()
        )));
    }
    let result = write();
    if let Err(error) = &result {
        state.failure = Some(error.to_string());
    }
    result
}

fn lock(state: &SharedState) -> Result<MutexGuard<'_, FileState>, StoreError> {
    state
        .lock()
        .map_err(|_| StoreError::Invalid("log lock poisoned".into()))
}

/// The state of `path` shared by every handle in this process.
fn shared_state(path: &Path) -> Result<SharedState, StoreError> {
    static STATES: OnceLock<Mutex<HashMap<PathBuf, SharedState>>> = OnceLock::new();
    let key = canonical(path)?;
    let mut states = STATES
        .get_or_init(Mutex::default)
        .lock()
        .map_err(|_| StoreError::Invalid("log registry poisoned".into()))?;
    // Entries held only by the registry belong to files with no open handle.
    states.retain(|_, state| Arc::strong_count(state) > 1);
    Ok(Arc::clone(states.entry(key).or_default()))
}

fn canonical(path: &Path) -> Result<PathBuf, StoreError> {
    let name = path
        .file_name()
        .ok_or_else(|| StoreError::Invalid(format!("{} names no file", path.display())))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let dir = fs::canonicalize(parent).map_err(|error| StoreError::open(parent, error))?;
    Ok(dir.join(name))
}

/// Leave the file ending at a line boundary. A torn final fragment (no
/// newline, not valid JSON) is exactly what readers ignore, so it is cut
/// off; a complete record that only lacks its newline gets one.
fn repair_tail(file: &mut File, path: &Path) -> Result<(), StoreError> {
    let io = |error| StoreError::io(path, error);
    let len = file.metadata().map_err(io)?.len();
    let start = last_line_start(file, len).map_err(io)?;
    if start == len {
        return Ok(());
    }
    let mut fragment = Vec::new();
    file.seek(SeekFrom::Start(start)).map_err(io)?;
    file.read_to_end(&mut fragment).map_err(io)?;
    match parse_line::<serde_json::Value>(&fragment) {
        Line::Malformed(_) => {
            tracing::warn!(path = %path.display(), bytes = len - start, "dropping torn final record before appending");
            file.set_len(start).map_err(io)?;
        }
        _ => file.write_all(b"\n").map_err(io)?,
    }
    file.sync_all().map_err(io)
}

/// Offset just past the last newline, or 0 when there is none.
fn last_line_start(file: &mut File, len: u64) -> std::io::Result<u64> {
    let mut end = len;
    let mut chunk = Vec::new();
    while end > 0 {
        let start = end.saturating_sub(TAIL_CHUNK);
        chunk.clear();
        file.seek(SeekFrom::Start(start))?;
        Read::by_ref(file)
            .take(end - start)
            .read_to_end(&mut chunk)?;
        if let Some(index) = chunk.iter().rposition(|&byte| byte == b'\n') {
            return Ok(start + index as u64 + 1);
        }
        end = start;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::LogRecord;

    fn note(text: &str) -> LogRecord {
        LogRecord::Note { text: text.into() }
    }

    #[test]
    fn a_failed_write_blocks_every_handle_until_a_reopen_repairs_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.jsonl");
        let mut healthy = Appender::create_new(&path).unwrap();
        let mut failing = Appender {
            file: File::open(&path).unwrap(),
            path: path.clone(),
            state: shared_state(&path).unwrap(),
        };
        assert!(matches!(
            failing.append(&note("a")),
            Err(StoreError::Io { .. })
        ));
        assert!(matches!(
            healthy.append(&note("b")),
            Err(StoreError::Invalid(_))
        ));
        assert!(matches!(healthy.sync(), Err(StoreError::Invalid(_))));

        Appender::open(&path, false)
            .unwrap()
            .append(&note("c"))
            .unwrap();
        healthy.append(&note("d")).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);
    }

    #[test]
    fn open_cuts_a_torn_fragment_longer_than_one_scan_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.jsonl");
        let complete = "{\"kind\":\"note\",\"text\":\"kept\"}\n";
        let torn = format!("{{\"kind\":\"note\",\"text\":\"{}", "x".repeat(150_000));
        fs::write(&path, format!("{complete}{torn}")).unwrap();
        let mut appender = Appender::open(&path, false).unwrap();
        appender.append(&note("after")).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            format!("{complete}{{\"kind\":\"note\",\"text\":\"after\"}}\n")
        );
    }

    #[test]
    fn open_terminates_a_complete_record_missing_its_newline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.jsonl");
        fs::write(&path, "{\"kind\":\"note\",\"text\":\"kept\"}").unwrap();
        Appender::open(&path, false)
            .unwrap()
            .append(&note("after"))
            .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.ends_with('\n'));
    }
}
