//! Targeted edits of one settings layer file for the GUI settings screens.
//!
//! Each edit re-reads the file, applies the change, checks the result still
//! loads, and replaces the file atomically. Unrelated keys are kept;
//! comments are not, because the file is rendered again from its parsed
//! table. A missing `settings.toml` is seeded from the v1 `config.toml`
//! beside it (which is never written), else created with `schema = 2`;
//! removals from a missing file with nothing to seed do nothing. Writing
//! `settings.local.toml` keeps `.z-engine/.gitignore` covering it.

use std::path::Path;

use serde::Serialize;
use toml::{Table, Value};

use crate::document;
use crate::error::ConfigError;
use crate::files::write_atomic;
use crate::gitignore::ensure_local_gitignore;
use crate::migrate::{as_v2, read_legacy};
use crate::paths::{LEGACY_CONFIG_FILE, LOCAL_SETTINGS_FILE, legacy_source_of};
use crate::settings::{HookConfig, McpServerConfig, RuleKind, is_hook_event};

pub fn set_value(file: &Path, key_path: &[&str], value: Value) -> Result<(), ConfigError> {
    check_key_path(key_path)?;
    edit(file, true, |table| {
        let (parent, leaf) = parent_table(table, key_path)?;
        let changed = parent.get(leaf) != Some(&value);
        parent.insert(leaf.to_string(), value);
        Ok(changed)
    })
}

pub fn remove_value(file: &Path, key_path: &[&str]) -> Result<(), ConfigError> {
    check_key_path(key_path)?;
    edit(file, false, |table| Ok(remove_path(table, key_path)))
}

/// Appends `value` to a string array unless it is already present.
pub fn add_to_array(file: &Path, key_path: &[&str], value: &str) -> Result<(), ConfigError> {
    check_key_path(key_path)?;
    edit(file, true, |table| {
        let (parent, leaf) = parent_table(table, key_path)?;
        let slot = parent
            .entry(leaf)
            .or_insert_with(|| Value::Array(Vec::new()));
        let Value::Array(items) = slot else {
            return Err(ConfigError::key_path(key_path, "is not an array"));
        };
        if items.iter().any(|item| item.as_str() == Some(value)) {
            return Ok(false);
        }
        items.push(Value::String(value.to_string()));
        Ok(true)
    })
}

/// Removes every occurrence of `value`; an emptied array is kept, because
/// an empty list still overrides lower layers for non-union keys.
pub fn remove_from_array(file: &Path, key_path: &[&str], value: &str) -> Result<(), ConfigError> {
    check_key_path(key_path)?;
    edit(file, false, |table| {
        let Some(Value::Array(items)) = value_mut(table, key_path) else {
            return Ok(false);
        };
        let before = items.len();
        items.retain(|item| item.as_str() != Some(value));
        Ok(items.len() != before)
    })
}

pub fn set_mcp_server(
    file: &Path,
    name: &str,
    server: &McpServerConfig,
) -> Result<(), ConfigError> {
    let name = name.trim();
    if let Some(problem) = server.transport_error() {
        return Err(ConfigError::Invalid {
            path: file.to_path_buf(),
            message: format!("mcp server `{name}`: {problem}"),
        });
    }
    set_value(
        file,
        &["mcp", "servers", name],
        to_value("mcp server", server)?,
    )
}

pub fn remove_mcp_server(file: &Path, name: &str) -> Result<(), ConfigError> {
    remove_value(file, &["mcp", "servers", name.trim()])
}

/// Replaces this layer's hooks for `event`; an empty list removes them.
pub fn set_hooks(file: &Path, event: &str, hooks: &[HookConfig]) -> Result<(), ConfigError> {
    if !is_hook_event(event) {
        return Err(ConfigError::key_path(
            &["hooks", event],
            "unknown hook event",
        ));
    }
    if hooks.is_empty() {
        return remove_value(file, &["hooks", event]);
    }
    let hooks = hooks.iter().map(|hook| to_value("hook", hook));
    set_value(
        file,
        &["hooks", event],
        Value::Array(hooks.collect::<Result<_, _>>()?),
    )
}

pub fn add_permission_rule(file: &Path, kind: RuleKind, rule: &str) -> Result<(), ConfigError> {
    let rule = rule.trim();
    if rule.is_empty() {
        return Err(ConfigError::key_path(
            &["permissions", kind.key()],
            "empty rule",
        ));
    }
    add_to_array(file, &["permissions", kind.key()], rule)
}

pub fn remove_permission_rule(file: &Path, kind: RuleKind, rule: &str) -> Result<(), ConfigError> {
    remove_from_array(file, &["permissions", kind.key()], rule.trim())
}

/// `change` returns whether it modified the table; unchanged files are not
/// rewritten. A missing `settings.toml` starts from its v1 `config.toml`
/// (converted in memory) so imported settings are kept.
fn edit(
    file: &Path,
    create: bool,
    change: impl FnOnce(&mut Table) -> Result<bool, ConfigError>,
) -> Result<(), ConfigError> {
    if file
        .file_name()
        .is_some_and(|name| name == LEGACY_CONFIG_FILE)
    {
        return Err(ConfigError::Invalid {
            path: file.to_path_buf(),
            message: "v1 config.toml files are read-only; edit settings.toml".to_string(),
        });
    }
    let _guard = document::lock();
    let (mut table, existed) = match document::read(file)? {
        Some(table) => (as_v2(file, table)?.0, true),
        None => match legacy_source_of(file)
            .map(|legacy| read_legacy(&legacy))
            .transpose()?
        {
            Some(Some((table, _))) => (table, false),
            _ if create => (document::new_table(), false),
            _ => return Ok(()),
        },
    };
    if !change(&mut table)? && (existed || !create) {
        return Ok(());
    }
    document::deserialize(table.clone()).map_err(|message| ConfigError::Invalid {
        path: file.to_path_buf(),
        message,
    })?;
    write_atomic(file, document::render(&table)?.as_bytes(), false)?;
    if file
        .file_name()
        .is_some_and(|name| name == LOCAL_SETTINGS_FILE)
    {
        if let Some(dir) = file.parent() {
            ensure_local_gitignore(dir)?;
        }
    }
    Ok(())
}

fn check_key_path(key_path: &[&str]) -> Result<(), ConfigError> {
    if key_path.is_empty() || key_path.iter().any(|key| key.trim().is_empty()) {
        return Err(ConfigError::key_path(key_path, "empty key"));
    }
    if key_path == ["schema"] {
        return Err(ConfigError::key_path(
            key_path,
            "the schema version is managed by the app",
        ));
    }
    Ok(())
}

/// The table holding the last key (intermediate tables are created) and
/// that key.
fn parent_table<'t, 'k>(
    table: &'t mut Table,
    key_path: &[&'k str],
) -> Result<(&'t mut Table, &'k str), ConfigError> {
    let (leaf, parents) = key_path
        .split_last()
        .ok_or_else(|| ConfigError::key_path(key_path, "empty key"))?;
    let mut current = table;
    for (depth, key) in parents.iter().enumerate() {
        let slot = current
            .entry(*key)
            .or_insert_with(|| Value::Table(Table::new()));
        current = match slot {
            Value::Table(child) => child,
            _ => return Err(ConfigError::key_path(&key_path[..=depth], "is not a table")),
        };
    }
    Ok((current, leaf))
}

fn value_mut<'t>(table: &'t mut Table, key_path: &[&str]) -> Option<&'t mut Value> {
    let (leaf, parents) = key_path.split_last()?;
    let mut current = table;
    for key in parents {
        current = current.get_mut(*key)?.as_table_mut()?;
    }
    current.get_mut(*leaf)
}

/// Removes the key and any tables it leaves empty.
fn remove_path(table: &mut Table, key_path: &[&str]) -> bool {
    match key_path {
        [] => false,
        [leaf] => table.remove(*leaf).is_some(),
        [head, rest @ ..] => {
            let Some(Value::Table(child)) = table.get_mut(*head) else {
                return false;
            };
            let removed = remove_path(child, rest);
            if removed && child.is_empty() {
                table.remove(*head);
            }
            removed
        }
    }
}

/// Serializes an entry without its empty lists and maps, which are the
/// defaults, to keep hand-read files short.
fn to_value<T: Serialize>(what: &'static str, entry: &T) -> Result<Value, ConfigError> {
    let value = Value::try_from(entry).map_err(|error| ConfigError::Serialize {
        what,
        message: error.to_string(),
    })?;
    Ok(match value {
        Value::Table(fields) => Value::Table(
            fields
                .into_iter()
                .filter(|(_, field)| !is_empty_collection(field))
                .collect(),
        ),
        other => other,
    })
}

fn is_empty_collection(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.is_empty(),
        Value::Table(fields) => fields.is_empty(),
        _ => false,
    }
}
