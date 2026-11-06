//! Layered settings: defaults < user < project < project-local < environment.
//!
//! Layers merge as raw TOML (see `merge.rs`) and the result is deserialized
//! after every layer, so a layer that does not fit the schema is skipped and
//! reported instead of failing the load. Loading never writes: when a v2
//! `settings.toml` is missing, its v1 `config.toml` is imported in memory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::{Table, Value};
use ts_rs::TS;

use crate::document::{self, Schema};
use crate::error::ConfigError;
use crate::merge::{merge_layer, nested};
use crate::migrate::{as_v2, read_legacy};
use crate::paths::{Paths, legacy_project_config_file, project_local_file, project_settings_file};
use crate::settings::{Settings, normalize, unknown_keys};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub enum LayerScope {
    Default,
    User,
    Project,
    ProjectLocal,
    Env,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct LayerInfo {
    pub scope: LayerScope,
    pub path: Option<PathBuf>,
    /// The file exists; for the env layer, an override variable is set.
    pub exists: bool,
    /// Why the layer was skipped.
    pub error: Option<String>,
    /// Set when a v1 file was imported in memory for this layer.
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct LoadedSettings {
    pub settings: Settings,
    /// Lowest precedence first.
    pub layers: Vec<LayerInfo>,
    pub warnings: Vec<String>,
}

/// Environment variables that override every file layer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvOverrides {
    /// `HYPERFORM_MODEL` -> `model.main`
    pub model: Option<String>,
    /// `HYPERFORM_BASE_URL` -> `provider.base_url`
    pub base_url: Option<String>,
    /// `HYPERFORM_PROVIDER` -> `provider.kind`
    pub provider: Option<String>,
    /// `HYPERFORM_SHELL` -> `shell.path`
    pub shell: Option<String>,
}

impl EnvOverrides {
    pub fn from_process() -> Self {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        Self {
            model: var("HYPERFORM_MODEL"),
            base_url: var("HYPERFORM_BASE_URL"),
            provider: var("HYPERFORM_PROVIDER"),
            shell: var("HYPERFORM_SHELL"),
        }
    }

    fn table(&self) -> Table {
        let overrides = [
            (["model", "main"], &self.model),
            (["provider", "base_url"], &self.base_url),
            (["provider", "kind"], &self.provider),
            (["shell", "path"], &self.shell),
        ];
        let mut table = Table::new();
        for (path, value) in overrides {
            if let Some(value) = value {
                merge_layer(&mut table, nested(&path, Value::String(value.clone())));
            }
        }
        table
    }
}

/// Loads with the process environment's overrides. Never fails: problems
/// are reported per layer and in `warnings`.
pub fn load(paths: &Paths, project_root: Option<&Path>) -> LoadedSettings {
    load_with_env(paths, project_root, &EnvOverrides::from_process())
}

pub fn load_with_env(
    paths: &Paths,
    project_root: Option<&Path>,
    env: &EnvOverrides,
) -> LoadedSettings {
    let mut layering = Layering::default();
    layering.layers.push(LayerInfo {
        scope: LayerScope::Default,
        path: None,
        exists: true,
        error: None,
        note: None,
    });
    let user_legacy = Some(paths.user_config_file.as_path());
    layering.file(LayerScope::User, &paths.user_settings_file, user_legacy);
    if let Some(root) = project_root {
        let legacy = legacy_project_config_file(root);
        layering.file(
            LayerScope::Project,
            &project_settings_file(root),
            Some(&legacy),
        );
        layering.file(LayerScope::ProjectLocal, &project_local_file(root), None);
    }
    let table = env.table();
    let exists = !table.is_empty();
    let error = if exists {
        layering.apply(table).err()
    } else {
        None
    };
    if let Some(error) = &error {
        layering
            .warnings
            .push(format!("environment overrides were skipped: {error}"));
    }
    layering.layers.push(LayerInfo {
        scope: LayerScope::Env,
        path: None,
        exists,
        error,
        note: None,
    });
    layering.finish()
}

#[derive(Default)]
struct Layering {
    merged: Table,
    settings: Settings,
    layers: Vec<LayerInfo>,
    warnings: Vec<String>,
}

impl Layering {
    /// Reads the v2 file, or when it is missing imports `legacy` in memory.
    fn file(&mut self, scope: LayerScope, path: &Path, legacy: Option<&Path>) {
        let mut info = LayerInfo {
            scope,
            path: Some(path.to_path_buf()),
            exists: false,
            error: None,
            note: None,
        };
        let mut read = self.read(path);
        if let (Ok(None), Some(legacy)) = (&read, legacy) {
            read = self.read_legacy(legacy, path, &mut info);
        }
        match read {
            Ok(None) => {}
            Ok(Some(table)) => {
                info.exists = true;
                info.error = self.apply(table).err();
            }
            Err(error) => {
                info.exists = true;
                info.error = Some(without_path(error));
            }
        }
        if let (Some(error), Some(shown)) = (&info.error, &info.path) {
            let shown = shown.display();
            self.warnings.push(format!("{shown} was skipped: {error}"));
        }
        self.layers.push(info);
    }

    fn read(&mut self, path: &Path) -> Result<Option<Table>, ConfigError> {
        let Some(table) = document::read(path)? else {
            return Ok(None);
        };
        if let Schema::Newer(version) = document::schema_of(&table) {
            self.warnings.push(format!(
                "{}: written for settings schema {version}; unknown keys are ignored",
                path.display()
            ));
        }
        let (table, notes) = as_v2(path, table)?;
        Ok(Some(self.accept(path, table, notes)))
    }

    /// Never writes: the import is saved to `target` by the first edit.
    fn read_legacy(
        &mut self,
        legacy: &Path,
        target: &Path,
        info: &mut LayerInfo,
    ) -> Result<Option<Table>, ConfigError> {
        info.path = Some(legacy.to_path_buf());
        let Some((table, notes)) = read_legacy(legacy)? else {
            info.path = Some(target.to_path_buf());
            return Ok(None);
        };
        let note = format!(
            "imported from the v1 file {} in memory; saved to {} on the first change",
            legacy.display(),
            target.display()
        );
        self.warnings.push(note.clone());
        info.note = Some(note);
        Ok(Some(self.accept(legacy, table, notes)))
    }

    /// Records conversion notes and unknown keys for a layer's table.
    fn accept(&mut self, path: &Path, table: Table, notes: Vec<String>) -> Table {
        let shown = path.display();
        self.warnings
            .extend(notes.into_iter().map(|note| format!("{shown}: {note}")));
        let unknown = unknown_keys(&table).into_iter();
        self.warnings
            .extend(unknown.map(|key| format!("{shown}: unknown key `{key}`")));
        table
    }

    fn apply(&mut self, layer: Table) -> Result<(), String> {
        let mut candidate = self.merged.clone();
        merge_layer(&mut candidate, layer);
        self.settings = document::deserialize(candidate.clone())?;
        self.merged = candidate;
        Ok(())
    }

    fn finish(mut self) -> LoadedSettings {
        self.warnings.extend(normalize(&mut self.settings));
        LoadedSettings {
            settings: self.settings,
            layers: self.layers,
            warnings: self.warnings,
        }
    }
}

/// The reason a layer file failed, without the path its layer records.
fn without_path(error: ConfigError) -> String {
    match error {
        ConfigError::Parse { message, .. } => message,
        ConfigError::Io { source, .. } => source.to_string(),
        ConfigError::TooLarge { limit, .. } => format!("larger than {limit} bytes"),
        other => other.to_string(),
    }
}
