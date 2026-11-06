//! Importing a v1 transcript as a v2 session directory, and summarizing a
//! v1 file for the session list without importing it.

use std::fs;
use std::path::Path;

use z_engine_protocol::{SessionId, SessionSummary};

use super::convert::{Source, convert};
use super::v1::V1Event;
use crate::append::Appender;
use crate::durable::{modified_ms, sync_dir};
use crate::error::StoreError;
use crate::layout::{LOG_FILE, Layout, META_FILE, id_timestamp_ms, v1_session_id};
use crate::meta::{SessionMeta, write_meta_file};
use crate::read::read_jsonl;
use crate::record::LogRecord;
use crate::replay::replay;

#[derive(Debug)]
struct Converted {
    records: Vec<LogRecord>,
    meta: SessionMeta,
}

/// Import `v1_file` (`<ULID>.jsonl`) into `sessions_dir/<ULID>/` with
/// `meta.legacy = true`, keeping the ULID as the session id. The v1 file is
/// never modified. When the v2 directory already exists nothing is written,
/// so importing twice is a no-op. The directory is assembled under a
/// temporary name and renamed into place, so a crash cannot leave a
/// half-imported session that later imports would skip.
pub fn import_v1(sessions_dir: &Path, v1_file: &Path) -> Result<SessionId, StoreError> {
    let id = v1_session_id(v1_file).ok_or_else(|| {
        StoreError::Invalid(format!(
            "{} is not a v1 session file (<id>.jsonl)",
            v1_file.display()
        ))
    })?;
    let target = Layout::new(sessions_dir).session_dir(&id)?;
    if target.exists() {
        return Ok(id);
    }
    let converted = convert_file(&id, v1_file)?;
    fs::create_dir_all(sessions_dir).map_err(|error| StoreError::io(sessions_dir, error))?;
    let staging = sessions_dir.join(format!(".import-{id}-{}", ulid::Ulid::new()));
    let result =
        write_session(&staging, &converted).and_then(|()| move_into_place(&staging, &target));
    discard_staging(&staging);
    result?;
    tracing::info!(session = %id, records = converted.records.len(), "imported v1 session");
    Ok(id)
}

/// Summary of a v1 file that has not been imported yet, identical to the
/// meta its import would write.
pub(crate) fn summarize_v1(id: &SessionId, path: &Path) -> Result<SessionSummary, StoreError> {
    Ok(SessionSummary::from(&convert_file(id, path)?.meta))
}

fn convert_file(id: &SessionId, path: &Path) -> Result<Converted, StoreError> {
    let lines = read_jsonl::<V1Event>(path)?;
    let modified = modified_ms(path);
    let created_at = id_timestamp_ms(id).or(modified).unwrap_or(0);
    let updated_at = modified.unwrap_or(created_at).max(created_at);
    let records = convert(&Source {
        session_id: id,
        events: &lines.items,
        created_at,
        updated_at,
        skipped_lines: lines.corrupt_lines,
    });
    let meta = SessionMeta::from_state(id, &replay(&records), updated_at, true);
    Ok(Converted { records, meta })
}

fn write_session(dir: &Path, converted: &Converted) -> Result<(), StoreError> {
    fs::create_dir(dir).map_err(|error| StoreError::io(dir, error))?;
    let mut log = Appender::create_new(&dir.join(LOG_FILE))?;
    for record in &converted.records {
        log.append(record)?;
    }
    log.sync()?;
    write_meta_file(&dir.join(META_FILE), &converted.meta)
}

fn move_into_place(staging: &Path, target: &Path) -> Result<(), StoreError> {
    if let Err(error) = fs::rename(staging, target) {
        // A concurrent import of the same file may have finished first.
        if !target.is_dir() {
            return Err(StoreError::io(target, error));
        }
    }
    sync_dir(target.parent().unwrap_or(target))
}

fn discard_staging(staging: &Path) {
    match fs::remove_dir_all(staging) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            tracing::warn!(path = %staging.display(), %error, "could not remove import staging directory");
        }
    }
}
