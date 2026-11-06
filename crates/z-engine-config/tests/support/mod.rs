//! Shared fixtures: temporary config/data roots and a project directory.
//! Nothing here reads the real home directory or process environment.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use z_engine_config::{EnvOverrides, LoadedSettings, Paths, load_with_env};

pub struct Fixture {
    pub tmp: TempDir,
    pub paths: Paths,
    /// A project root containing `.git`, which bounds the instruction walk.
    pub project: PathBuf,
}

pub fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::with_roots(tmp.path().join("config"), tmp.path().join("data"));
    let project = tmp.path().join("project");
    fs::create_dir_all(project.join(".git")).unwrap();
    Fixture {
        tmp,
        paths,
        project,
    }
}

impl Fixture {
    /// Loads with no environment overrides.
    pub fn load(&self) -> LoadedSettings {
        load_with_env(&self.paths, Some(&self.project), &EnvOverrides::default())
    }
}

pub fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

pub fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}
