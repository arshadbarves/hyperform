//! `git status --porcelain=v1 -z` parsing.

/// One changed path. Porcelain paths are relative to the repository root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: String,
    /// Index (staged) status letter; `?` for untracked.
    pub index: char,
    /// Work-tree status letter.
    pub worktree: char,
    /// The source path of a rename or copy.
    pub orig_path: Option<String>,
}

impl StatusEntry {
    /// `XY path` (or `XY orig -> path`), as `git status --short` prints it.
    pub fn short_line(&self) -> String {
        match &self.orig_path {
            Some(orig) => format!("{}{} {orig} -> {}", self.index, self.worktree, self.path),
            None => format!("{}{} {}", self.index, self.worktree, self.path),
        }
    }
}

/// `XY path\0`, with renames and copies as `XY new\0old\0`.
pub(crate) fn parse_porcelain_z(raw: &str) -> Vec<StatusEntry> {
    let mut fields = raw.split('\0').filter(|field| !field.is_empty());
    let mut entries = Vec::new();
    while let Some(field) = fields.next() {
        let mut chars = field.chars();
        let (Some(index), Some(worktree)) = (chars.next(), chars.next()) else {
            continue;
        };
        let rest = chars.as_str();
        let path = rest.strip_prefix(' ').unwrap_or(rest).to_string();
        let moved = matches!(index, 'R' | 'C') || matches!(worktree, 'R' | 'C');
        let orig_path = if moved {
            fields.next().map(str::to_string)
        } else {
            None
        };
        entries.push(StatusEntry {
            path,
            index,
            worktree,
            orig_path,
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modified_untracked_and_renamed_entries() {
        let raw = " M src/a.rs\0R  new name.rs\0old.rs\0?? notes.md\0";
        let entries = parse_porcelain_z(raw);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].path, "src/a.rs");
        assert_eq!((entries[0].index, entries[0].worktree), (' ', 'M'));
        assert_eq!(entries[1].path, "new name.rs");
        assert_eq!(entries[1].orig_path.as_deref(), Some("old.rs"));
        assert_eq!(entries[1].short_line(), "R  old.rs -> new name.rs");
        assert_eq!(entries[2].short_line(), "?? notes.md");
    }
}
