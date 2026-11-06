//! The bounded, breadth-first, gitignore-aware walk that finds manifests.
//! It never follows symlinks and never enters VCS, dependency, build
//! output or tool-state directories. Shallow manifests are found first, so
//! a limit cuts off the deepest parts of the tree.

use std::collections::VecDeque;
use std::fs::FileType;
use std::sync::Arc;

use super::collector::Collector;
use super::gitignore::{Rules, ignored};
use super::rel;
use super::types::DiscoveryOptions;

/// A manifest file: its root-relative directory and its file name.
#[derive(Debug, Clone)]
pub(crate) struct Found {
    pub(crate) dir: String,
    pub(crate) name: String,
}

struct Pending {
    dir: String,
    depth: usize,
    ignores: Vec<Arc<Rules>>,
}

pub(crate) fn walk(
    c: &mut Collector,
    options: &DiscoveryOptions,
    is_manifest: &dyn Fn(&str) -> bool,
) -> Vec<Found> {
    let mut found = Vec::new();
    let mut seen = 0usize;
    let mut too_deep = false;
    let mut queue = VecDeque::from([Pending {
        dir: String::new(),
        depth: 0,
        ignores: repository_excludes(c),
    }]);
    'walk: while let Some(pending) = queue.pop_front() {
        let mut ignores = pending.ignores;
        for name in [".gitignore", ".ignore"] {
            let file = rel::join(&pending.dir, name);
            if c.is_file(&file) {
                if let Some(text) = c.read(&file) {
                    ignores.push(Arc::new(Rules::parse(&pending.dir, &text)));
                }
            }
        }
        let Some(mut entries) = list(c, &pending.dir, options.max_entries) else {
            continue;
        };
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, kind) in entries {
            if seen >= options.max_entries {
                c.note(format!(
                    "discovery stopped after {} directory entries; projects deeper in the tree may be missing",
                    options.max_entries
                ));
                break 'walk;
            }
            seen += 1;
            let path = rel::join(&pending.dir, &name);
            if kind.is_dir() {
                if skipped_dir(&name) || ignored(&ignores, &path, true) {
                    continue;
                }
                if pending.depth >= options.max_depth {
                    too_deep = true;
                    continue;
                }
                queue.push_back(Pending {
                    dir: path,
                    depth: pending.depth + 1,
                    ignores: ignores.clone(),
                });
            } else if kind.is_file() && is_manifest(&name) && !ignored(&ignores, &path, false) {
                found.push(Found {
                    dir: pending.dir.clone(),
                    name,
                });
            }
        }
    }
    if too_deep {
        c.note(format!(
            "directories more than {} levels below the root were not scanned",
            options.max_depth
        ));
    }
    found
}

/// At most `limit + 1` entries of `dir` (symlinks and non-UTF-8 names left
/// out), or `None` with a note when it cannot be listed. Reading up to the
/// whole walk's limit, not just what remains of it, keeps the name order
/// (and so the result) independent of the order the OS lists entries in.
fn list(c: &mut Collector, dir: &str, limit: usize) -> Option<Vec<(String, FileType)>> {
    let entries = match std::fs::read_dir(c.path(dir)) {
        Ok(entries) => entries,
        Err(error) => {
            c.note(format!(
                "{}: cannot list directory: {error}",
                rel::display(dir)
            ));
            return None;
        }
    };
    let listed = entries
        .take(limit.saturating_add(1))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let kind = entry.file_type().ok()?;
            let name = entry.file_name().into_string().ok()?;
            (!kind.is_symlink()).then_some((name, kind))
        })
        .collect();
    Some(listed)
}

/// The repository's `.git/info/exclude`, when discovery starts at a
/// repository root.
fn repository_excludes(c: &mut Collector) -> Vec<Arc<Rules>> {
    let file = ".git/info/exclude";
    if !c.is_file(file) {
        return Vec::new();
    }
    c.read(file)
        .map(|text| vec![Arc::new(Rules::parse("", &text))])
        .unwrap_or_default()
}

/// Never scanned, ignored or not: VCS data, dependencies, build output,
/// caches, and z-engine state (worktrees hold copies of the project).
fn skipped_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | ".jj"
            | ".z-engine"
            | "node_modules"
            | "bower_components"
            | "target"
            | "dist"
            | "build"
            | "_build"
            | "out"
            | "vendor"
            | "bin"
            | "obj"
            | "coverage"
            | ".cache"
            | ".next"
            | ".nuxt"
            | ".svelte-kit"
            | ".turbo"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".tox"
            | ".nox"
            | ".mypy_cache"
            | ".pytest_cache"
            | ".ruff_cache"
            | ".gradle"
            | ".idea"
            | ".vs"
            | ".yarn"
            | ".pnpm-store"
            | "Pods"
            | ".dart_tool"
            | "CMakeFiles"
    ) || name.starts_with("cmake-build-")
}
