//! Checks from settings layered over discovered ones.

use std::path::{Component, Path, PathBuf};

use z_engine_host::{normalize, resolve};

use super::types::ProjectProfile;
use crate::{CheckSource, CheckSpec, ConfiguredCheck, DEFAULT_CHECK_TIMEOUT_SECS};

/// Adds `configured` checks to `profile`: a check whose id matches an
/// existing one replaces it in place; others are appended in order. A
/// check without an id or command is skipped with a note; one whose
/// directory does not exist under `root` is kept with a note.
pub fn merge_configured(profile: &mut ProjectProfile, configured: &[ConfiguredCheck], root: &Path) {
    for check in configured {
        let (raw_id, command) = (check.id.trim(), check.command.trim());
        if raw_id.is_empty() || command.is_empty() {
            profile.notes.push(format!(
                "configured check `{raw_id}` was ignored: it needs an id and a command"
            ));
            continue;
        }
        let id = if raw_id.contains(':') {
            raw_id.to_string()
        } else {
            format!("custom:{raw_id}")
        };
        let cwd = project_relative(check.cwd.as_deref(), root);
        if !resolve(root, &cwd).is_dir() {
            profile.notes.push(format!(
                "configured check `{id}`: directory `{}` does not exist",
                cwd.display()
            ));
        }
        let label = match check.label.trim() {
            "" => command.to_string(),
            label => label.to_string(),
        };
        let timeout_secs = match check.timeout_secs {
            0 => DEFAULT_CHECK_TIMEOUT_SECS,
            secs => secs,
        };
        let spec = CheckSpec {
            id,
            label,
            kind: check.kind,
            command: command.to_string(),
            cwd,
            source: CheckSource::Configured,
            timeout_secs,
        };
        match profile.checks.iter_mut().find(|c| c.id == spec.id) {
            Some(existing) => *existing = spec,
            None => profile.checks.push(spec),
        }
    }
}

/// A configured directory relative to the project root (`.` for the
/// root). Absolute directories inside `root` become relative; other
/// absolute directories stay absolute.
fn project_relative(cwd: Option<&str>, root: &Path) -> PathBuf {
    let Some(cwd) = cwd.map(str::trim).filter(|cwd| !cwd.is_empty()) else {
        return PathBuf::from(".");
    };
    let path = normalize(Path::new(cwd));
    let relative = if path.is_absolute() {
        match path.strip_prefix(normalize(root)) {
            Ok(inside) => inside.to_path_buf(),
            Err(_) => return path,
        }
    } else {
        path
    };
    if relative
        .components()
        .all(|c| matches!(c, Component::CurDir))
    {
        PathBuf::from(".")
    } else {
        relative
    }
}
