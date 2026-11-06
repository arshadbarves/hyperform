//! One settings file as a raw TOML table: reading, schema detection, and
//! rendering. Every writer of settings files holds [`lock`] while it
//! reads, changes, and replaces a file.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use toml::{Table, Value};

use crate::error::ConfigError;
use crate::files::read_data_file;
use crate::settings::Settings;

/// The settings schema this crate reads and writes.
pub const SCHEMA_VERSION: i64 = 2;

const HEADER: &str = "# Z Engine settings. The app rewrites this file when settings change,\n# so comments added here are not kept.\n";

static FILE_LOCK: Mutex<()> = Mutex::new(());

/// Serializes read-modify-write cycles on settings files in this process.
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    // The mutex guards no data; a panic elsewhere cannot leave it inconsistent.
    FILE_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The parsed file, or `None` when it does not exist.
pub(crate) fn read(path: &Path) -> Result<Option<Table>, ConfigError> {
    let Some(text) = read_data_file(path)? else {
        return Ok(None);
    };
    let table = toml::from_str(&text).map_err(|error| ConfigError::parse(path, error))?;
    Ok(Some(table))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Schema {
    /// No `schema` key: written for v1 (an empty file counts as current).
    V1,
    Current,
    /// Written by a newer version; known keys are still read.
    Newer(i64),
    Invalid,
}

pub(crate) fn schema_of(table: &Table) -> Schema {
    match table.get("schema") {
        None if table.is_empty() => Schema::Current,
        None => Schema::V1,
        Some(Value::Integer(version)) if *version < SCHEMA_VERSION => Schema::V1,
        Some(Value::Integer(version)) if *version == SCHEMA_VERSION => Schema::Current,
        Some(Value::Integer(version)) => Schema::Newer(*version),
        Some(_) => Schema::Invalid,
    }
}

pub(crate) fn invalid_schema(path: &Path) -> ConfigError {
    ConfigError::parse(path, "`schema` must be an integer")
}

/// A new document containing only `schema = 2`.
pub(crate) fn new_table() -> Table {
    Table::from_iter([("schema".to_string(), Value::Integer(SCHEMA_VERSION))])
}

/// The table as [`Settings`], or a one-line description of the first
/// mismatch, e.g. "unknown effort `extreme` in `model.effort`".
pub(crate) fn deserialize(table: Table) -> Result<Settings, String> {
    table
        .try_into()
        .map_err(|error: toml::de::Error| error.to_string().trim_end().replace('\n', " "))
}

pub(crate) fn render(table: &Table) -> Result<String, ConfigError> {
    render_with("", table)
}

/// Like [`render`], with `preface` comment lines before the standard header.
pub(crate) fn render_with(preface: &str, table: &Table) -> Result<String, ConfigError> {
    let body = toml::to_string(table).map_err(|error| ConfigError::Serialize {
        what: "settings",
        message: error.to_string(),
    })?;
    Ok(format!("{preface}{HEADER}{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(text: &str) -> Schema {
        schema_of(&toml::from_str(text).unwrap())
    }

    #[test]
    fn detects_schema_versions() {
        assert_eq!(schema(""), Schema::Current);
        assert_eq!(schema("model = \"x\""), Schema::V1);
        assert_eq!(schema("schema = 1\nmodel = \"x\""), Schema::V1);
        assert_eq!(schema("schema = 2"), Schema::Current);
        assert_eq!(schema("schema = 3"), Schema::Newer(3));
        assert_eq!(schema("schema = \"2\""), Schema::Invalid);
    }

    #[test]
    fn rendered_documents_parse_back() {
        let mut table = new_table();
        let server: Table = toml::from_str("command = \"npx\"\nargs = [\"-y\"]").unwrap();
        table.insert(
            "mcp".into(),
            Value::Table(Table::from_iter([(
                "servers".to_string(),
                Value::Table(Table::from_iter([("fs".to_string(), Value::Table(server))])),
            )])),
        );
        let text = render(&table).unwrap();
        assert!(text.starts_with("# Z Engine settings"));
        let back: Table = toml::from_str(&text).unwrap();
        assert_eq!(back, table);
    }
}
