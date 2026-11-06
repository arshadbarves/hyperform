//! Path arithmetic shared by every tool: lexical normalization, `~`
//! expansion, root-relative resolution, and the symlink-aware containment
//! check that guards writes.

use std::path::{Component, Path, PathBuf};

/// Resolves `.` and `..` lexically, without touching the filesystem.
/// `..` never climbs above the root of an absolute path; leading `..` of a
/// relative path is kept.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                Some(Component::ParentDir | Component::CurDir) | None => out.push(".."),
            },
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// Expands a leading `~` (alone or followed by a separator) to `home`, or to
/// the current user's home directory when `home` is `None`. Other inputs,
/// including `~user`, are returned unchanged.
pub fn expand_tilde(input: &str, home: Option<&Path>) -> PathBuf {
    let rest = if input == "~" {
        Some("")
    } else {
        input
            .strip_prefix("~/")
            .or_else(|| input.strip_prefix("~\\").filter(|_| cfg!(windows)))
    };
    let Some(rest) = rest else {
        return PathBuf::from(input);
    };
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => match dirs::home_dir() {
            Some(home) => home,
            None => return PathBuf::from(input),
        },
    };
    if rest.is_empty() {
        home
    } else {
        home.join(rest)
    }
}

/// Absolute inputs are kept, relative inputs are joined to `root`; the
/// result is normalized lexically.
pub fn resolve(root: &Path, input: impl AsRef<Path>) -> PathBuf {
    let input = input.as_ref();
    if input.is_absolute() {
        normalize(input)
    } else {
        normalize(&root.join(input))
    }
}

/// Whether `path` (relative paths are taken against `root`) physically lies
/// inside `root`. The longest existing ancestor is canonicalized so symlinks
/// and `..` after a symlink resolve the way the OS will; the missing tail is
/// applied lexically. A dangling symlink on the way is never considered
/// contained, because its eventual target is unknown.
pub fn is_within(root: &Path, path: &Path) -> bool {
    let target = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    match (physical(root), physical(&target)) {
        (Some(root), Some(target)) => target.starts_with(root),
        _ => false,
    }
}

/// `path` relative to `root` for display (`.` for the root itself); paths
/// outside `root` are shown in full.
pub fn relative_display(root: &Path, path: &Path) -> String {
    let relative = path
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .ok()
        .or_else(|| {
            let (root, path) = (physical(root)?, physical(path)?);
            path.strip_prefix(root).map(Path::to_path_buf).ok()
        });
    match relative {
        Some(rel) if rel.as_os_str().is_empty() => ".".to_string(),
        Some(rel) => rel.display().to_string(),
        None => path.display().to_string(),
    }
}

/// Stable identity for a file that may be deleted or not yet exist: the
/// canonical parent directory joined with the file name.
pub(crate) fn identity(path: &Path) -> PathBuf {
    let path = normalize(path);
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
            match std::fs::canonicalize(parent) {
                Ok(parent) => parent.join(name),
                Err(_) => path,
            }
        }
        _ => path,
    }
}

/// Canonical form of the longest existing prefix of `path` with the rest
/// appended lexically; `None` when a missing component is a dangling
/// symlink or nothing on the path exists.
fn physical(path: &Path) -> Option<PathBuf> {
    let components: Vec<Component<'_>> = path.components().collect();
    for split in (1..=components.len()).rev() {
        let prefix: PathBuf = components[..split].iter().collect();
        if let Ok(canonical) = std::fs::canonicalize(&prefix) {
            let mut out = canonical;
            for component in &components[split..] {
                out.push(component.as_os_str());
            }
            return Some(normalize(&out));
        }
        let dangling = std::fs::symlink_metadata(&prefix).is_ok_and(|m| m.file_type().is_symlink());
        if dangling {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_is_lexical() {
        assert_eq!(normalize(Path::new("/a/./b/../c")), PathBuf::from("/a/c"));
        assert_eq!(normalize(Path::new("/../x")), PathBuf::from("/x"));
        assert_eq!(normalize(Path::new("a/../../b")), PathBuf::from("../b"));
        assert_eq!(normalize(Path::new("./")), PathBuf::from("."));
    }

    #[test]
    fn tilde_expansion_uses_the_given_home() {
        let home = Path::new("/home/me");
        assert_eq!(expand_tilde("~", Some(home)), PathBuf::from("/home/me"));
        assert_eq!(
            expand_tilde("~/x/y", Some(home)),
            PathBuf::from("/home/me/x/y")
        );
        assert_eq!(
            expand_tilde("~other/x", Some(home)),
            PathBuf::from("~other/x")
        );
        assert_eq!(expand_tilde("a/~/b", Some(home)), PathBuf::from("a/~/b"));
    }

    #[test]
    fn resolve_keeps_absolute_and_joins_relative() {
        let root = Path::new("/proj");
        assert_eq!(
            resolve(root, "src/../lib.rs"),
            PathBuf::from("/proj/lib.rs")
        );
        assert_eq!(resolve(root, "/etc/hosts"), PathBuf::from("/etc/hosts"));
    }
}
