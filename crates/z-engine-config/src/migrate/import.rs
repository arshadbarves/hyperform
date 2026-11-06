//! One-way import of v1 `config.toml` files. v1 files are only ever read:
//! the converted settings go to a new v2 `settings.toml` or stay in memory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::Table;
use ts_rs::TS;

use super::convert_v1;
use crate::document::{self, Schema};
use crate::error::ConfigError;
use crate::files::{exists, write_atomic};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct MigrationOutcome {
    /// A v2 file was written from a v1 file.
    pub migrated: bool,
    /// The v1 file that was imported; it is left unchanged.
    pub source: Option<PathBuf>,
    /// What changed meaning or was dropped.
    pub notes: Vec<String>,
}

/// A parsed table in v2 shape: v1 files (no `schema`) are converted in
/// memory, with notes describing the conversion.
pub(crate) fn as_v2(path: &Path, table: Table) -> Result<(Table, Vec<String>), ConfigError> {
    match document::schema_of(&table) {
        Schema::V1 => Ok(convert_v1(table)),
        Schema::Invalid => Err(document::invalid_schema(path)),
        Schema::Current | Schema::Newer(_) => Ok((table, Vec::new())),
    }
}

/// A legacy file converted in memory; `None` when it does not exist.
pub(crate) fn read_legacy(path: &Path) -> Result<Option<(Table, Vec<String>)>, ConfigError> {
    match document::read(path)? {
        Some(table) => as_v2(path, table).map(Some),
        None => Ok(None),
    }
}

/// Writes `target` from the v1 file `legacy` when `target` does not exist
/// and `legacy` does; otherwise does nothing. `legacy` is never modified.
pub fn import_v1_file(legacy: &Path, target: &Path) -> Result<MigrationOutcome, ConfigError> {
    let _guard = document::lock();
    if exists(target)? {
        return Ok(MigrationOutcome::default());
    }
    let Some((table, notes)) = read_legacy(legacy)? else {
        return Ok(MigrationOutcome::default());
    };
    let preface = format!(
        "# Imported from {} on {}; that v1 file is left unchanged.\n",
        legacy.display(),
        chrono::Local::now().format("%Y-%m-%d")
    );
    write_atomic(
        target,
        document::render_with(&preface, &table)?.as_bytes(),
        false,
    )?;
    Ok(MigrationOutcome {
        migrated: true,
        source: Some(legacy.to_path_buf()),
        notes,
    })
}
