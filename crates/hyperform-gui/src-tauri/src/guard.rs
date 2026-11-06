//! Defence in depth for commands that take file paths from the webview:
//! extension and instruction files may only be read, written or deleted
//! in the folders the engine discovers them in, files are only opened or
//! revealed inside a project, and never through `..` or a symlink that
//! leads elsewhere.

use std::path::{Component, Path, PathBuf};

use hyperform_config::{PROJECT_DIR, Paths};
use serde::Deserialize;

use crate::ipc::{IpcResult, fail};

const INSTRUCTION_NAMES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "AGENTS.local.md",
    "CLAUDE.local.md",
];
const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ExtensionKind {
    Agents,
    Commands,
    Skills,
    Rules,
    OutputStyles,
}

impl ExtensionKind {
    const ALL: [ExtensionKind; 5] = [
        Self::Agents,
        Self::Commands,
        Self::Skills,
        Self::Rules,
        Self::OutputStyles,
    ];

    fn dir(self) -> &'static str {
        match self {
            Self::Agents => "agents",
            Self::Commands => "commands",
            Self::Skills => "skills",
            Self::Rules => "rules",
            Self::OutputStyles => "output-styles",
        }
    }
}

/// Where a new extension file goes: `name` is relative to the kind's
/// folder without `.md` (`a:b` for `a/b.md`), or the folder of a skill.
pub(crate) fn extension_target(base: &Path, kind: ExtensionKind, name: &str) -> IpcResult<PathBuf> {
    let parts: Vec<&str> = name.trim().split(':').collect();
    if parts.iter().any(|part| !plain_name(part)) {
        return Err(format!("invalid extension name `{name}`"));
    }
    let mut path = base.join(kind.dir());
    match kind {
        ExtensionKind::Skills if parts.len() == 1 => {
            path.push(parts[0]);
            path.push(SKILL_FILE);
        }
        ExtensionKind::Skills => return Err("a skill name cannot contain `:`".to_string()),
        _ => {
            let (last, dirs) = parts.split_last().ok_or("empty extension name")?;
            path.extend(dirs);
            path.push(format!("{last}.md"));
        }
    }
    Ok(path)
}

/// The hyperform config dir, `~/.claude`, and each project's `.hyperform`
/// and `.claude` folders.
pub(crate) fn extension_bases(paths: &Paths, roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut bases = vec![paths.config_dir.clone()];
    bases.extend(paths.claude_user_dir());
    for root in roots {
        bases.push(root.join(PROJECT_DIR));
        bases.push(root.join(".claude"));
    }
    bases
}

/// `path` as a markdown file below `<base>/<kind>/` of one of `bases`.
pub(crate) fn extension_file(path: &str, bases: &[PathBuf]) -> IpcResult<PathBuf> {
    let path = plain_absolute(path)?;
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        return Err(format!("{} is not a markdown file", path.display()));
    }
    let inside = bases.iter().any(|base| {
        ExtensionKind::ALL
            .iter()
            .any(|kind| contained(&path, &base.join(kind.dir())))
    });
    if inside {
        Ok(path)
    } else {
        Err(format!("{} is not an extension file", path.display()))
    }
}

/// `path` as an instruction file named like one the engine reads, in one
/// of `dirs` or listed in `listed`.
pub(crate) fn instruction_file(
    path: &str,
    dirs: &[PathBuf],
    listed: &[String],
) -> IpcResult<PathBuf> {
    let path = plain_absolute(path)?;
    let named = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| INSTRUCTION_NAMES.contains(&name));
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let allowed = named
        && (dirs.iter().any(|dir| same_dir(&parent, dir))
            || listed.iter().any(|known| Path::new(known) == path));
    if allowed {
        Ok(path)
    } else {
        Err(format!("{} is not an instruction file", path.display()))
    }
}

/// `path` as an existing file or folder inside one of `roots`, once
/// symlinks resolve.
pub(crate) fn project_path(path: &str, roots: &[PathBuf]) -> IpcResult<PathBuf> {
    let path = plain_absolute(path)?;
    let real =
        std::fs::canonicalize(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let inside = roots.iter().any(|root| {
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        real.starts_with(root)
    });
    if inside {
        Ok(real)
    } else {
        Err(format!("{} is not inside a project", path.display()))
    }
}

/// Removes a deleted skill's folder when nothing else is left in it.
pub(crate) fn remove_extension(path: &Path) -> IpcResult<()> {
    std::fs::remove_file(path).map_err(fail)?;
    if path.file_name().is_some_and(|name| name == SKILL_FILE) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir(dir);
        }
    }
    Ok(())
}

fn plain_name(part: &str) -> bool {
    !part.is_empty()
        && !part.starts_with('.')
        && !part.contains(['/', '\\', '\0'])
        && part.trim() == part
}

fn plain_absolute(path: &str) -> IpcResult<PathBuf> {
    let path = Path::new(path);
    let plain = path
        .components()
        .all(|part| !matches!(part, Component::ParentDir | Component::CurDir));
    if path.is_absolute() && plain {
        Ok(path.to_path_buf())
    } else {
        Err(format!("{} must be an absolute path", path.display()))
    }
}

/// Lexically below `dir`, and still below it once symlinks resolve (for
/// the deepest part of `path` that exists).
fn contained(path: &Path, dir: &Path) -> bool {
    if !path.starts_with(dir) {
        return false;
    }
    let Some(real_dir) = canonical_prefix(dir) else {
        return true;
    };
    canonical_prefix(path).is_none_or(|real| real.starts_with(real_dir))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || canonical_prefix(a).is_some_and(|real| canonical_prefix(b) == Some(real))
}

/// The canonical form of the deepest existing ancestor, with the rest
/// appended.
fn canonical_prefix(path: &Path) -> Option<PathBuf> {
    let mut rest = Vec::new();
    let mut current = path;
    loop {
        if let Ok(real) = std::fs::canonicalize(current) {
            return Some(rest.iter().rev().fold(real, |acc, part| acc.join(part)));
        }
        rest.push(current.file_name()?.to_os_string());
        current = current.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_names_map_to_files() {
        let base = Path::new("/c");
        let target = |kind, name| extension_target(base, kind, name);
        assert_eq!(
            target(ExtensionKind::Agents, "fix").unwrap(),
            Path::new("/c/agents/fix.md")
        );
        assert_eq!(
            target(ExtensionKind::Commands, "git:commit").unwrap(),
            Path::new("/c/commands/git/commit.md")
        );
        assert_eq!(
            target(ExtensionKind::Skills, "pdf").unwrap(),
            Path::new("/c/skills/pdf/SKILL.md")
        );
        assert_eq!(
            target(ExtensionKind::OutputStyles, "terse").unwrap(),
            Path::new("/c/output-styles/terse.md")
        );
        for bad in ["", "..", "a/b", "a::b", ".hidden", "x\\y"] {
            assert!(target(ExtensionKind::Rules, bad).is_err(), "{bad}");
        }
        assert!(target(ExtensionKind::Skills, "a:b").is_err());
        let kind: ExtensionKind = serde_json::from_str("\"output-styles\"").unwrap();
        assert_eq!(kind, ExtensionKind::OutputStyles);
    }

    #[test]
    fn extension_paths_stay_in_extension_folders() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(config.join("agents")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let bases = vec![config.clone()];
        let ok = config.join("agents/a.md");
        assert_eq!(extension_file(ok.to_str().unwrap(), &bases).unwrap(), ok);
        for bad in [
            config.join("agents/../../outside/a.md"),
            config.join("settings.toml"),
            config.join("agents/a.txt"),
            outside.join("a.md"),
        ] {
            assert!(
                extension_file(bad.to_str().unwrap(), &bases).is_err(),
                "{bad:?}"
            );
        }
        assert!(extension_file("agents/a.md", &bases).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, config.join("agents/link")).unwrap();
            let escape = config.join("agents/link/a.md");
            assert!(extension_file(escape.to_str().unwrap(), &bases).is_err());
        }
    }

    #[test]
    fn instruction_files_must_be_named_and_placed_like_the_engine_reads_them() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let dirs = vec![root.clone()];
        let file = |name: &str| root.join(name).to_string_lossy().into_owned();
        assert!(instruction_file(&file("AGENTS.md"), &dirs, &[]).is_ok());
        assert!(instruction_file(&file("CLAUDE.local.md"), &dirs, &[]).is_ok());
        assert!(instruction_file(&file("README.md"), &dirs, &[]).is_err());
        let nested = file("sub/AGENTS.md");
        assert!(instruction_file(&nested, &dirs, &[]).is_err());
        assert!(instruction_file(&nested, &dirs, std::slice::from_ref(&nested)).is_ok());
        assert!(instruction_file("/etc/AGENTS.md", &dirs, &[]).is_err());
    }

    #[test]
    fn opened_paths_must_exist_inside_a_project() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/a.rs"), "").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "").unwrap();
        let roots = vec![root.clone()];
        let path = |p: PathBuf| p.to_string_lossy().into_owned();
        assert!(project_path(&path(root.join("src/a.rs")), &roots).is_ok());
        assert!(project_path(&path(root.clone()), &roots).is_ok());
        for bad in [
            root.join("src/missing.rs"),
            root.join("src/../../secret.txt"),
            dir.path().join("secret.txt"),
        ] {
            assert!(project_path(&path(bad.clone()), &roots).is_err(), "{bad:?}");
        }
        assert!(project_path("src/a.rs", &roots).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.path().join("secret.txt"), root.join("link")).unwrap();
            assert!(project_path(&path(root.join("link")), &roots).is_err());
        }
    }
}
