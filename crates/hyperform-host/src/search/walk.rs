//! The shared gitignore-aware walk: honors `.gitignore`, `.ignore`,
//! `.rgignore`, `.git/info/exclude` and the global excludes file (also
//! outside git repositories), includes dotfiles, never enters `.git`, does
//! not follow symlinks, and visits entries in file-name order.

use std::path::Path;

use ignore::WalkBuilder;

pub(crate) fn walker(root: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .parents(true)
        .require_git(false)
        .follow_links(false)
        .add_custom_ignore_filename(".rgignore")
        .sort_by_file_name(|a, b| a.cmp(b))
        .filter_entry(|entry| entry.file_name() != ".git");
    builder
}
