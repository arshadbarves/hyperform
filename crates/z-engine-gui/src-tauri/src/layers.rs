//! The settings files the app writes (user, project, local) and how each
//! reads as a layer: a missing v2 file shows the v1 `config.toml` it would
//! be imported from, converted in memory exactly as the loader does.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use toml::Table;
use z_engine_config::{
    Paths, import_v1_file, legacy_project_config_file, project_local_file, project_settings_file,
};

use crate::ipc::{IpcResult, fail};

/// Settings files larger than this are not shown.
const MAX_LAYER_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Scope {
    User,
    Project,
    Local,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LayerFile {
    pub(crate) path: String,
    pub(crate) exists: bool,
    /// The layer's TOML table as JSON.
    pub(crate) raw: Value,
}

/// The file a write to `scope` changes; project scopes need a root.
pub(crate) fn layer_file(paths: &Paths, scope: Scope, root: Option<&Path>) -> IpcResult<PathBuf> {
    match (scope, root) {
        (Scope::User, _) => Ok(paths.user_settings_file.clone()),
        (Scope::Project, Some(root)) => Ok(project_settings_file(root)),
        (Scope::Local, Some(root)) => Ok(project_local_file(root)),
        (_, None) => Err("project settings need a project root".to_string()),
    }
}

/// The v1 file imported when the layer's v2 file is missing.
fn legacy_file(paths: &Paths, scope: Scope, root: Option<&Path>) -> Option<PathBuf> {
    match (scope, root) {
        (Scope::User, _) => Some(paths.user_config_file.clone()),
        (Scope::Project, Some(root)) => Some(legacy_project_config_file(root)),
        _ => None,
    }
}

pub(crate) fn read_layer(paths: &Paths, scope: Scope, root: Option<&Path>) -> IpcResult<LayerFile> {
    let file = layer_file(paths, scope, root)?;
    let exists = file.is_file();
    let table = if exists {
        let table = read_table(&file)?;
        if is_v1(&table) {
            convert(&file)?
        } else {
            table
        }
    } else {
        match legacy_file(paths, scope, root).filter(|legacy| legacy.is_file()) {
            Some(legacy) => convert(&legacy)?,
            None => Table::new(),
        }
    };
    Ok(LayerFile {
        path: file.to_string_lossy().into_owned(),
        exists,
        raw: serde_json::to_value(table).map_err(fail)?,
    })
}

fn read_table(path: &Path) -> IpcResult<Table> {
    let size = std::fs::metadata(path).map_err(fail)?.len();
    if size > MAX_LAYER_BYTES {
        return Err(format!("{} is larger than 1 MiB", path.display()));
    }
    let text = std::fs::read_to_string(path).map_err(fail)?;
    text.parse::<Table>()
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// A non-empty table without `schema` was written for v1.
fn is_v1(table: &Table) -> bool {
    !table.is_empty() && !table.contains_key("schema")
}

/// The v2 table of a v1 file, via the config crate's import into a
/// scratch file (the source is never modified).
fn convert(v1: &Path) -> IpcResult<Table> {
    let scratch = tempfile::tempdir().map_err(fail)?;
    let target = scratch.path().join("settings.toml");
    import_v1_file(v1, &target).map_err(fail)?;
    if !target.is_file() {
        return Ok(Table::new());
    }
    read_table(&target)
}

/// A JSON value from the webview as TOML; `null` has no TOML form.
pub(crate) fn toml_value(value: Value) -> IpcResult<toml::Value> {
    if value.is_null() {
        return Err("null is not a settings value; remove the key instead".to_string());
    }
    toml::Value::try_from(value).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(dir: &Path) -> Paths {
        Paths::with_roots(dir.join("config"), dir.join("data"))
    }

    #[test]
    fn scopes_select_their_files() {
        let paths = Paths::with_roots("/c", "/d");
        let root = Path::new("/p");
        let file = |scope, root| layer_file(&paths, scope, root).unwrap();
        assert_eq!(file(Scope::User, None), Path::new("/c/settings.toml"));
        assert_eq!(
            file(Scope::Project, Some(root)),
            Path::new("/p/.z-engine/settings.toml")
        );
        assert_eq!(
            file(Scope::Local, Some(root)),
            Path::new("/p/.z-engine/settings.local.toml")
        );
        assert!(layer_file(&paths, Scope::Local, None).is_err());
        let scope: Scope = serde_json::from_str("\"local\"").unwrap();
        assert_eq!(scope, Scope::Local);
    }

    #[test]
    fn layers_read_v2_files_and_import_v1_in_memory() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let root = dir.path().join("project");
        std::fs::create_dir_all(root.join(".z-engine")).unwrap();
        let missing = read_layer(&paths, Scope::Local, Some(&root)).unwrap();
        assert!(!missing.exists);
        assert_eq!(missing.raw, serde_json::json!({}));

        let local = project_local_file(&root);
        std::fs::write(&local, "schema = 2\n[model]\nmain = \"m\"\n").unwrap();
        let layer = read_layer(&paths, Scope::Local, Some(&root)).unwrap();
        assert!(layer.exists);
        assert_eq!(layer.raw["model"]["main"], "m");

        let legacy = legacy_project_config_file(&root);
        std::fs::write(&legacy, "model = \"old-model\"\n").unwrap();
        let imported = read_layer(&paths, Scope::Project, Some(&root)).unwrap();
        assert!(!imported.exists, "created on the first change");
        assert_eq!(imported.raw["model"]["main"], "old-model");
        assert!(!project_settings_file(&root).exists());
        assert_eq!(
            std::fs::read_to_string(&legacy).unwrap(),
            "model = \"old-model\"\n"
        );
    }

    #[test]
    fn json_values_convert_to_toml() {
        let value = toml_value(serde_json::json!(["a", 1, true])).unwrap();
        assert_eq!(value.as_array().map(Vec::len), Some(3));
        assert!(toml_value(Value::Null).is_err());
    }
}
