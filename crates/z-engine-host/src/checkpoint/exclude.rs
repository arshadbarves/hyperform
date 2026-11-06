//! What the shadow repository must not store: `.git` entries, agent
//! worktrees, the shadow repository itself, whatever the user's own
//! repository ignores, and files over 8 MiB. Trees with more than 50,000
//! candidate files are refused.

use std::path::Path;

use ignore::WalkBuilder;

use crate::HostError;
use crate::fs::atomic_write_sync;

pub(super) const MAX_CANDIDATES: usize = 50_000;
pub(super) const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
const WORKTREES: &str = ".hyperform/worktrees";

/// Paths kept out of one snapshot (root-relative, `/`-separated).
#[derive(Debug, Clone, Default)]
pub(super) struct Exclusions {
    /// Files over the size limit.
    pub(super) large: Vec<String>,
    /// Ignored by the user's repository (directories end with `/`).
    pub(super) user_ignored: Vec<String>,
}

impl Exclusions {
    pub(super) fn covers(&self, path: &str) -> bool {
        self.large.iter().any(|large| large == path)
            || self
                .user_ignored
                .iter()
                .any(|ignored| match ignored.strip_suffix('/') {
                    Some(dir) => path.starts_with(ignored) || path == dir,
                    None => path == ignored,
                })
    }
}

/// Walks the tree as `git add -A` will see it and returns the files too
/// large to snapshot, sorted; more than `max_files` candidates is refused.
/// Blocking.
pub(super) fn large_files(
    work_tree: &Path,
    git_dir: &Path,
    max_files: usize,
) -> Result<Vec<String>, HostError> {
    let worktrees = work_tree.join(WORKTREES);
    let shadow = git_dir.to_path_buf();
    let mut builder = WalkBuilder::new(work_tree);
    builder
        .hidden(false)
        .ignore(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .parents(true)
        .require_git(false)
        .follow_links(false)
        .filter_entry(move |entry| {
            entry.file_name() != ".git"
                && !entry.path().starts_with(&worktrees)
                && !entry.path().starts_with(&shadow)
        });
    let mut candidates = 0usize;
    let mut large = Vec::new();
    for entry in builder.build().filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        candidates += 1;
        if candidates > max_files {
            return Err(HostError::Blocked(format!(
                "{} has more than {max_files} files; checkpoints are disabled for it",
                work_tree.display()
            )));
        }
        if entry.metadata().is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
            if let Ok(rel) = entry.path().strip_prefix(work_tree) {
                large.push(slashed(rel));
            }
        }
    }
    large.sort();
    Ok(large)
}

/// The shadow repository's `info/exclude`.
pub(super) fn render(git_dir: &Path, work_tree: &Path, exclusions: &Exclusions) -> String {
    let mut lines = vec![
        "# Managed by HyperForm checkpoints; rewritten on every snapshot.".to_string(),
        ".git".to_string(),
        format!("/{WORKTREES}/"),
    ];
    if let Ok(rel) = git_dir.strip_prefix(work_tree) {
        lines.push(format!("/{}/", escape(&slashed(rel))));
    }
    let listed = exclusions.user_ignored.iter().chain(&exclusions.large);
    lines.extend(listed.map(|path| format!("/{}", escape(path))));
    let mut content = lines.join("\n");
    content.push('\n');
    content
}

pub(super) fn write_if_changed(path: &Path, content: &str) -> Result<(), HostError> {
    match std::fs::read_to_string(path) {
        Ok(existing) if existing == content => Ok(()),
        _ => atomic_write_sync(path, content.as_bytes()),
    }
}

fn slashed(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

/// Makes a literal path safe as a gitignore pattern (a trailing `/` of a
/// directory is kept).
fn escape(path: &str) -> String {
    let (body, dir) = match path.strip_suffix('/') {
        Some(body) => (body, "/"),
        None => (path, ""),
    };
    let mut out = String::with_capacity(body.len() + 4);
    for c in body.chars() {
        if matches!(c, '\\' | '*' | '?' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    if out.starts_with('#') || out.starts_with('!') {
        out.insert(0, '\\');
    }
    if out.ends_with(' ') {
        out.insert(out.len() - 1, '\\');
    }
    out.push_str(dir);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_makes_paths_literal() {
        assert_eq!(escape("a*b?.txt"), r"a\*b\?.txt");
        assert_eq!(escape("!important"), r"\!important");
        assert_eq!(escape("dir [1]/"), r"dir \[1\]/");
        assert_eq!(escape("trailing "), r"trailing\ ");
    }

    #[test]
    fn render_lists_fixed_user_and_large_entries() {
        let exclusions = Exclusions {
            large: vec!["big.bin".to_string()],
            user_ignored: vec!["node_modules/".to_string()],
        };
        let content = render(
            Path::new("/p/.hyperform/checkpoints/abc.git"),
            Path::new("/p"),
            &exclusions,
        );
        let lines: Vec<&str> = content.lines().skip(1).collect();
        assert_eq!(
            lines,
            [
                ".git",
                "/.hyperform/worktrees/",
                "/.hyperform/checkpoints/abc.git/",
                "/node_modules/",
                "/big.bin"
            ]
        );
        assert!(exclusions.covers("node_modules/pkg/index.js"));
        assert!(exclusions.covers("big.bin"));
        assert!(!exclusions.covers("src/main.rs"));
    }
}
