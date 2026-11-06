//! Lexical path handling. Nothing here touches the filesystem, so symlinks
//! are the caller's concern: pass paths that are already resolved.

use std::path::{Component, Path, PathBuf};

/// Directories (first component under an allowed root) and files whose writes
/// always need an explicit answer: repository metadata (`.git` config and
/// hooks run code) and harness or MCP settings (they grant permissions or
/// spawn servers). `.git` is protected at any depth.
const PROTECTED_DIRS: &[&str] = &[".hyperform"];
const PROTECTED_FILES: &[&str] = &[
    ".claude/settings.json",
    ".claude/settings.local.json",
    ".mcp.json",
];

/// Removes `.` and resolves `..` lexically. `..` never climbs above the root
/// of an absolute path; a relative path keeps leading `..` components.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(".."),
            },
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `path` taken from `base` when relative, then normalized.
pub(crate) fn resolve(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize(path)
    } else {
        normalize(&base.join(path))
    }
}

/// A relative path that climbs above its starting directory at some point
/// (`../x`, `a/../../x`). Absolute paths count as escaping.
pub(crate) fn escapes_start(path: &Path) -> bool {
    let mut depth = 0usize;
    for component in path.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => match depth.checked_sub(1) {
                Some(up) => depth = up,
                None => return true,
            },
            Component::RootDir | Component::Prefix(_) => return true,
        }
    }
    false
}

/// `path` (normalized) lies under one of `roots` in a protected location.
/// Names compare case-insensitively because default macOS and Windows
/// filesystems treat `.GIT` and `.git` as the same directory.
pub(crate) fn is_protected<'a>(path: &Path, roots: impl IntoIterator<Item = &'a Path>) -> bool {
    roots
        .into_iter()
        .filter_map(|root| path.strip_prefix(root).ok())
        .any(|relative| {
            let names: Vec<String> = relative
                .components()
                .filter_map(|component| match component {
                    Component::Normal(name) => Some(name.to_string_lossy().to_ascii_lowercase()),
                    _ => None,
                })
                .collect();
            names.iter().any(|name| name == ".git")
                || names
                    .first()
                    .is_some_and(|first| PROTECTED_DIRS.contains(&first.as_str()))
                || PROTECTED_FILES.contains(&names.join("/").as_str())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_resolves_dots_lexically() {
        assert_eq!(normalize(Path::new("/a/./b/../c")), PathBuf::from("/a/c"));
        assert_eq!(normalize(Path::new("/../../etc")), PathBuf::from("/etc"));
        assert_eq!(normalize(Path::new("a/../../b")), PathBuf::from("../b"));
        assert_eq!(normalize(Path::new("./a/b/..")), PathBuf::from("a"));
        assert_eq!(normalize(Path::new("../..")), PathBuf::from("../.."));
    }

    #[test]
    fn resolve_joins_relative_paths_onto_the_base() {
        let base = Path::new("/work/proj");
        assert_eq!(
            resolve(base, Path::new("src/lib.rs")),
            base.join("src/lib.rs")
        );
        assert_eq!(
            resolve(base, Path::new("../other")),
            PathBuf::from("/work/other")
        );
        assert_eq!(
            resolve(base, Path::new("/etc/../tmp")),
            PathBuf::from("/tmp")
        );
    }

    #[test]
    fn escapes_start_tracks_the_running_depth() {
        assert!(!escapes_start(Path::new("a/b")));
        assert!(!escapes_start(Path::new("a/../b")));
        assert!(!escapes_start(Path::new("./a")));
        assert!(escapes_start(Path::new("../a")));
        assert!(escapes_start(Path::new("a/../../b")));
        assert!(escapes_start(Path::new("/abs")));
    }

    #[test]
    fn protected_paths_cover_git_and_harness_settings() {
        let root = Path::new("/p");
        let protected = |p: &str| is_protected(Path::new(p), [root]);
        assert!(protected("/p/.git/config"));
        assert!(protected("/p/.git"));
        assert!(protected("/p/vendor/lib/.git/hooks/pre-commit"));
        assert!(protected("/p/.GIT/config"));
        assert!(protected("/p/.hyperform/settings.json"));
        assert!(protected("/p/.z-engine/settings.json"));
        assert!(protected("/p/.claude/settings.json"));
        assert!(protected("/p/.claude/settings.local.json"));
        assert!(protected("/p/.mcp.json"));
        assert!(!protected("/p/.claude/agents/reviewer.md"));
        assert!(!protected("/p/docs/.z-engine/notes.md"));
        assert!(!protected("/p/src/git/mod.rs"));
        assert!(!protected("/elsewhere/.git/config"));
    }
}
