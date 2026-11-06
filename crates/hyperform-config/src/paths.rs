//! On-disk locations. User configuration and data default to `hyperform`
//! (`~/.config/hyperform`, `%APPDATA%\hyperform` on Windows).
//! Settings live in `settings.toml` files; legacy `config.toml` files are
//! only ever read, as import sources.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::ConfigError;
use crate::files::{exists, write_atomic};
use crate::migrate::{MigrationOutcome, import_v1_file};

pub const APP_DIR: &str = "hyperform";
pub const PROJECT_DIR: &str = ".hyperform";
pub const CONFIG_DIR_ENV: &str = "HYPERFORM_CONFIG_DIR";
pub const DATA_DIR_ENV: &str = "HYPERFORM_DATA_DIR";
pub const SETTINGS_FILE: &str = "settings.toml";
pub const LOCAL_SETTINGS_FILE: &str = "settings.local.toml";
/// v1's settings file name, read only for import.
pub const LEGACY_CONFIG_FILE: &str = "config.toml";

const DEFAULT_USER_SETTINGS: &str = include_str!("default_config.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    /// v1 `<ulid>.jsonl` session files and v2 per-session directories.
    pub sessions_dir: PathBuf,
    /// Work plans: `<project key>/<plan id>/` directories.
    pub plans_dir: PathBuf,
    pub checkpoints_dir: PathBuf,
    pub cache_dir: PathBuf,
    /// v2 user settings, `settings.toml`.
    pub user_settings_file: PathBuf,
    /// v1 user `config.toml`: the legacy import source, never written.
    pub user_config_file: PathBuf,
    pub auth_file: PathBuf,
    pub trust_file: PathBuf,
    pub models_file: PathBuf,
    /// Bounds the instruction-file walk and locates `~/.claude`; `None`
    /// disables both (the [`Paths::with_roots`] default).
    pub home_dir: Option<PathBuf>,
}

impl Paths {
    /// `$HYPERFORM_CONFIG_DIR`, else platform defaults.
    pub fn discover() -> Result<Self, ConfigError> {
        Self::discover_from(
            |name| std::env::var_os(name),
            dirs::home_dir(),
            dirs::data_dir(),
        )
    }

    fn discover_from(
        var: impl Fn(&str) -> Option<OsString>,
        home: Option<PathBuf>,
        platform_data: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        let from_env = |name: &str| {
            var(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        let config_dir = from_env(CONFIG_DIR_ENV)
            .or_else(|| default_config_dir(home.as_deref()))
            .ok_or(ConfigError::NoDirectory {
                kind: "config",
                env_var: CONFIG_DIR_ENV,
            })?;
        let data_dir = from_env(DATA_DIR_ENV)
            .or_else(|| platform_data.map(|dir| dir.join(APP_DIR)))
            .ok_or(ConfigError::NoDirectory {
                kind: "data",
                env_var: DATA_DIR_ENV,
            })?;
        Ok(Self {
            home_dir: home,
            ..Self::with_roots(config_dir, data_dir)
        })
    }

    /// Every path under explicit roots, with no home directory.
    pub fn with_roots(config_dir: impl Into<PathBuf>, data_dir: impl Into<PathBuf>) -> Self {
        let config_dir = config_dir.into();
        let data_dir = data_dir.into();
        Self {
            sessions_dir: data_dir.join("sessions"),
            plans_dir: data_dir.join("plans"),
            checkpoints_dir: data_dir.join("checkpoints"),
            cache_dir: data_dir.join("cache"),
            user_settings_file: config_dir.join(SETTINGS_FILE),
            user_config_file: config_dir.join(LEGACY_CONFIG_FILE),
            auth_file: config_dir.join("auth.json"),
            trust_file: config_dir.join("trust.json"),
            models_file: config_dir.join("models.json"),
            config_dir,
            data_dir,
            home_dir: None,
        }
    }

    /// `~/.claude`, read for Claude Code compatibility.
    pub fn claude_user_dir(&self) -> Option<PathBuf> {
        self.home_dir.as_ref().map(|home| home.join(".claude"))
    }

    /// Creates the directories and, when `settings.toml` is missing, writes
    /// it: imported from the v1 `config.toml` if one exists, else the
    /// commented default.
    pub fn ensure(&self) -> Result<MigrationOutcome, ConfigError> {
        let dirs = [
            &self.config_dir,
            &self.data_dir,
            &self.sessions_dir,
            &self.plans_dir,
            &self.checkpoints_dir,
            &self.cache_dir,
        ];
        for dir in dirs {
            fs::create_dir_all(dir).map_err(|error| ConfigError::io(dir, error))?;
        }

        if exists(&self.user_settings_file)? {
            return Ok(MigrationOutcome::default());
        }
        match import_v1_file(&self.user_config_file, &self.user_settings_file) {
            Ok(outcome) if outcome.migrated => Ok(outcome),
            Ok(outcome) => {
                let default = DEFAULT_USER_SETTINGS.as_bytes();
                write_atomic(&self.user_settings_file, default, false)?;
                Ok(outcome)
            }
            Err(error) => {
                let source = self.user_config_file.display();
                tracing::warn!(path = %source, %error, "v1 user settings were not imported");
                Ok(MigrationOutcome {
                    notes: vec![format!("{source} was not imported: {error}")],
                    ..MigrationOutcome::default()
                })
            }
        }
    }
}

/// `<root>/.hyperform`
pub fn project_dir(root: &Path) -> PathBuf {
    root.join(PROJECT_DIR)
}

/// `<root>/<project_dir>/settings.toml`, shared with the team.
pub fn project_settings_file(root: &Path) -> PathBuf {
    project_dir(root).join(SETTINGS_FILE)
}

/// `<root>/<project_dir>/settings.local.toml`, personal and git-ignored.
pub fn project_local_file(root: &Path) -> PathBuf {
    project_dir(root).join(LOCAL_SETTINGS_FILE)
}

/// Legacy project file: read only for import.
pub fn legacy_project_config_file(root: &Path) -> PathBuf {
    project_dir(root).join(LEGACY_CONFIG_FILE)
}

/// The v1 file a v2 `settings.toml` imports from (`config.toml` beside it).
pub(crate) fn legacy_source_of(settings_file: &Path) -> Option<PathBuf> {
    let name = settings_file.file_name()?;
    (name == SETTINGS_FILE).then(|| settings_file.with_file_name(LEGACY_CONFIG_FILE))
}

#[cfg(windows)]
fn default_config_dir(home: Option<&Path>) -> Option<PathBuf> {
    dirs::config_dir()
        .or_else(|| home.map(Path::to_path_buf))
        .map(|dir| dir.join(APP_DIR))
}

#[cfg(not(windows))]
fn default_config_dir(home: Option<&Path>) -> Option<PathBuf> {
    home.map(|home| home.join(".config").join(APP_DIR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EnvOverrides, Settings, load_with_env};

    #[test]
    fn roots_lay_out_every_file() {
        let paths = Paths::with_roots("/c", "/d");
        assert_eq!(paths.user_settings_file, Path::new("/c/settings.toml"));
        assert_eq!(paths.user_config_file, Path::new("/c/config.toml"));
        assert_eq!(paths.auth_file, Path::new("/c/auth.json"));
        assert_eq!(paths.trust_file, Path::new("/c/trust.json"));
        assert_eq!(paths.models_file, Path::new("/c/models.json"));
        assert_eq!(paths.sessions_dir, Path::new("/d/sessions"));
        assert_eq!(paths.plans_dir, Path::new("/d/plans"));
        assert_eq!(paths.checkpoints_dir, Path::new("/d/checkpoints"));
        assert_eq!(paths.cache_dir, Path::new("/d/cache"));
        assert!(paths.claude_user_dir().is_none());
        let root = Path::new("/p");
        assert_eq!(
            project_settings_file(root),
            Path::new("/p/.hyperform/settings.toml")
        );
        assert_eq!(
            project_local_file(root),
            Path::new("/p/.hyperform/settings.local.toml")
        );
        assert_eq!(
            legacy_project_config_file(root),
            Path::new("/p/.hyperform/config.toml")
        );
        assert_eq!(
            legacy_source_of(&project_settings_file(root)),
            Some(legacy_project_config_file(root))
        );
        assert_eq!(legacy_source_of(&project_local_file(root)), None);
    }

    #[test]
    fn env_overrides_win_over_platform_dirs() {
        let var = |name: &str| (name == CONFIG_DIR_ENV).then(|| OsString::from("/custom"));
        let paths =
            Paths::discover_from(var, Some("/home/u".into()), Some("/data".into())).unwrap();
        assert_eq!(paths.config_dir, Path::new("/custom"));
        assert_eq!(paths.data_dir, Path::new("/data/hyperform"));
        assert_eq!(
            paths.claude_user_dir(),
            Some(PathBuf::from("/home/u/.claude"))
        );
        let missing = Paths::discover_from(|_| None, Some("/home/u".into()), None);
        assert!(matches!(
            missing,
            Err(ConfigError::NoDirectory { kind: "data", .. })
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn default_config_dir_matches_hyperform() {
        let paths =
            Paths::discover_from(|_| None, Some("/home/u".into()), Some("/d".into())).unwrap();
        assert_eq!(paths.config_dir, Path::new("/home/u/.config/hyperform"));
    }

    #[test]
    fn ensure_creates_default_settings_that_load_as_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::with_roots(tmp.path().join("config"), tmp.path().join("data"));
        let outcome = paths.ensure().unwrap();
        assert!(!outcome.migrated && outcome.source.is_none());
        assert!(paths.sessions_dir.is_dir() && paths.cache_dir.is_dir());
        assert!(paths.plans_dir.is_dir());
        let text = fs::read_to_string(&paths.user_settings_file).unwrap();
        assert!(text.contains("schema = 2"));
        assert!(!paths.user_config_file.exists());
        let loaded = load_with_env(&paths, None, &EnvOverrides::default());
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        let kept = "schema = 2\n[model]\nmain = \"kept\"\n";
        fs::write(&paths.user_settings_file, kept).unwrap();
        paths.ensure().unwrap();
        assert_eq!(fs::read_to_string(&paths.user_settings_file).unwrap(), kept);
    }
}
