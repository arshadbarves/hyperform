//! What a sandboxed command may write and whether it may reach the network.
//!
//! The default writable set is the project root, the additional
//! directories, extra configured directories, the temp directory, and the
//! tool caches in [`TOOL_CACHES`] that exist. Everything else stays
//! readable but not writable.

use std::path::{Path, PathBuf};

use crate::fs::{expand_tilde, resolve};

/// Home-relative caches that builds write, made writable when present.
/// Toolchains themselves (`~/.rustup`, `~/.nvm`, ...) only need reads.
pub const TOOL_CACHES: &[&str] = &[
    // cargo: downloaded crates, git checkouts, and the package-cache locks.
    ".cargo/registry",
    ".cargo/git",
    ".cargo/.package-cache",
    ".cargo/.package-cache-mutate",
    ".cargo/.global-cache",
    // npm's content cache.
    ".npm",
    // XDG cache (pip, yarn, pnpm, go-build, sccache, ...).
    ".cache",
    // Gradle and Maven repositories.
    ".gradle",
    ".m2",
];

/// Kept read-only inside every workspace root: files that make code run
/// outside the sandbox later (git hooks and config, harness hooks, MCP
/// servers, permission rules).
pub const PROTECTED: &[&str] = &[
    ".git/hooks",
    ".git/config",
    ".z-engine/settings.toml",
    ".z-engine/settings.local.toml",
    ".claude/settings.json",
    ".claude/settings.local.json",
    ".mcp.json",
];

/// macOS app caches (Homebrew, pip, Go, Xcode tools, ...).
#[cfg(target_os = "macos")]
const PLATFORM_CACHES: &[&str] = &["Library/Caches"];
#[cfg(not(target_os = "macos"))]
const PLATFORM_CACHES: &[&str] = &[];

/// Where writes are allowed and whether the network is reachable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SandboxProfile {
    /// Directories (or files) writable along with everything below them.
    /// Missing entries are skipped when the sandbox is built.
    pub writable: Vec<PathBuf>,
    /// Paths inside `writable` that stay read-only. Seatbelt also protects
    /// entries that do not exist yet; bubblewrap only existing ones.
    pub read_only: Vec<PathBuf>,
    /// Off blocks every connection except to localhost. Under bubblewrap
    /// the command gets its own network namespace, whose loopback does not
    /// reach servers on the host.
    pub allow_network: bool,
}

impl SandboxProfile {
    /// The default profile for a workspace: `root`, `additional_dirs`,
    /// `extra_writable` (`~/` expands to the home directory, relative
    /// entries resolve against `root`), the temp directory, and the
    /// existing tool caches. [`PROTECTED`] paths under `root` and the
    /// additional directories stay read-only. When `root` is a git
    /// worktree, the main repository's git directory is writable too (so
    /// commits work) except its hooks and config.
    pub fn for_workspace(
        root: &Path,
        additional_dirs: &[PathBuf],
        extra_writable: &[String],
        allow_network: bool,
    ) -> Self {
        let home = dirs::home_dir();
        let roots: Vec<PathBuf> = std::iter::once(root.to_path_buf())
            .chain(additional_dirs.iter().map(|dir| resolve(root, dir)))
            .collect();
        let mut read_only: Vec<PathBuf> = roots
            .iter()
            .flat_map(|dir| PROTECTED.iter().map(move |path| dir.join(path)))
            .collect();
        let mut writable = roots;
        if let Some(common) = git_common_dir(root) {
            read_only.push(common.join("hooks"));
            read_only.push(common.join("config"));
            writable.push(common);
        }
        writable.extend(
            extra_writable
                .iter()
                .map(|entry| entry.trim())
                .filter(|entry| !entry.is_empty())
                .map(|entry| resolve(root, expand_tilde(entry, home.as_deref()))),
        );
        writable.push(std::env::temp_dir());
        if let Some(home) = &home {
            writable.extend(tool_caches(home));
        }
        Self {
            writable,
            read_only,
            allow_network,
        }
    }

    /// Adds one writable path.
    #[must_use]
    pub fn with_writable(mut self, path: impl Into<PathBuf>) -> Self {
        self.writable.push(path.into());
        self
    }

    /// Existing writable paths, canonicalized, with entries nested inside
    /// another entry dropped. Sorted, so the generated sandbox is stable.
    pub(crate) fn resolved_writable(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .writable
            .iter()
            .filter_map(|path| std::fs::canonicalize(path).ok())
            .collect();
        paths.sort();
        paths.dedup();
        let mut kept: Vec<PathBuf> = Vec::new();
        for path in paths {
            if !kept.iter().any(|parent| path.starts_with(parent)) {
                kept.push(path);
            }
        }
        kept
    }

    /// Read-only paths with their longest existing ancestor canonicalized,
    /// so entries that do not exist yet still match resolved paths.
    pub(crate) fn resolved_read_only(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.read_only.iter().filter_map(|p| lenient(p)).collect();
        paths.sort();
        paths.dedup();
        paths
    }
}

fn lenient(path: &Path) -> Option<PathBuf> {
    let mut rest = Vec::new();
    for ancestor in path.ancestors() {
        if let Ok(base) = std::fs::canonicalize(ancestor) {
            return Some(rest.iter().rev().fold(base, |dir, name| dir.join(name)));
        }
        rest.push(ancestor.file_name()?);
    }
    None
}

/// The shared git directory of a worktree checkout (`.git` is a file
/// pointing into `<main>/.git/worktrees/<name>`); `None` for a normal
/// repository or no repository.
fn git_common_dir(root: &Path) -> Option<PathBuf> {
    let dot_git = root.join(".git");
    if !dot_git.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(&dot_git).ok()?;
    let gitdir = resolve(root, text.trim().strip_prefix("gitdir:")?.trim());
    let common = match std::fs::read_to_string(gitdir.join("commondir")) {
        Ok(relative) => resolve(&gitdir, relative.trim()),
        Err(_) => gitdir,
    };
    common.is_dir().then_some(common)
}

/// The entries of [`TOOL_CACHES`] (and the platform's caches) that exist
/// under `home`.
pub fn tool_caches(home: &Path) -> Vec<PathBuf> {
    TOOL_CACHES
        .iter()
        .chain(PLATFORM_CACHES)
        .map(|relative| home.join(relative))
        .filter(|path| path.exists())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_profile_resolves_extra_entries() {
        let root = Path::new("/work/proj");
        let profile = SandboxProfile::for_workspace(
            root,
            &[PathBuf::from("/data/shared")],
            &["out".into(), " ".into(), "/abs/dir".into()],
            false,
        );
        assert_eq!(profile.writable[0], root);
        assert!(profile.writable.contains(&PathBuf::from("/data/shared")));
        assert!(profile.writable.contains(&PathBuf::from("/work/proj/out")));
        assert!(profile.writable.contains(&PathBuf::from("/abs/dir")));
        assert!(profile.writable.contains(&std::env::temp_dir()));
        assert!(!profile.allow_network);
    }

    #[test]
    fn resolved_writable_drops_missing_and_nested_paths() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        let profile = SandboxProfile::default()
            .with_writable(&nested)
            .with_writable(dir.path())
            .with_writable(dir.path().join("missing"));
        let canonical = std::fs::canonicalize(dir.path()).unwrap();
        assert_eq!(profile.resolved_writable(), vec![canonical]);
    }

    #[test]
    fn protected_paths_are_read_only_even_before_they_exist() {
        let dir = tempfile::tempdir().unwrap();
        let profile = SandboxProfile::for_workspace(dir.path(), &[], &[], true);
        let canonical = std::fs::canonicalize(dir.path()).unwrap();
        let read_only = profile.resolved_read_only();
        assert!(read_only.contains(&canonical.join(".git/hooks")));
        assert!(read_only.contains(&canonical.join(".z-engine/settings.toml")));
        assert_eq!(read_only.len(), PROTECTED.len());
    }

    #[test]
    fn worktrees_can_write_the_shared_git_directory() {
        let main = tempfile::tempdir().unwrap();
        let common = main.path().join(".git");
        let gitdir = common.join("worktrees/task");
        std::fs::create_dir_all(&gitdir).unwrap();
        std::fs::write(gitdir.join("commondir"), "../..\n").unwrap();
        let tree = main.path().join(".z-engine/worktrees/task");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tree.join(".git"), format!("gitdir: {}\n", gitdir.display())).unwrap();
        let profile = SandboxProfile::for_workspace(&tree, &[], &[], true);
        assert!(profile.writable.contains(&common));
        assert!(profile.read_only.contains(&common.join("hooks")));
        assert!(profile.read_only.contains(&common.join("config")));
    }

    #[test]
    fn tool_caches_only_lists_existing_directories() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".cargo/registry")).unwrap();
        std::fs::create_dir_all(home.path().join(".npm")).unwrap();
        let caches = tool_caches(home.path());
        assert_eq!(
            caches,
            vec![
                home.path().join(".cargo/registry"),
                home.path().join(".npm")
            ]
        );
    }
}
