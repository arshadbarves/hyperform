//! Files a request may need, found without the model: files it names,
//! files defining something it mentions (from the repository map), and,
//! on a chat's first turn, files changed in the working tree. At most
//! `MAX_CANDIDATES`, in that order.

use std::collections::HashSet;

pub(super) const MAX_CANDIDATES: usize = 8;
const MAX_DEFINES: usize = 12;
/// Shorter names (`new`, `main`) match too much to mean anything.
const MIN_NAME_CHARS: usize = 5;

pub(super) const NAMED: &str = "named in the request";
pub(super) const DEFINES: &str = "defines something the request mentions";
pub(super) const CHANGED: &str = "changed in the working tree";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    /// Relative to the project root, `/`-separated.
    pub path: String,
    pub why: &'static str,
    pub defines: Vec<String>,
}

/// One entry per file of a rendered repository map: its path and the
/// names it defines.
pub(super) fn outline(map: &str) -> Vec<(String, Vec<String>)> {
    let mut files: Vec<(String, Vec<String>)> = Vec::new();
    for line in map.lines() {
        if !line.starts_with(' ') {
            let path = line.strip_suffix(':').filter(|path| !path.contains(' '));
            if let Some(path) = path.filter(|path| !path.is_empty()) {
                files.push((path.to_string(), Vec::new()));
            }
            continue;
        }
        let Some((_, rest)) = line.trim_start().split_once(' ') else {
            continue;
        };
        let name = rest.rsplit_once(" (L").map_or(rest, |(name, _)| name);
        if let Some((_, names)) = files.last_mut() {
            names.push(name.to_string());
        }
    }
    files
}

/// Paths in `git status --short` output, skipping deletions.
pub(super) fn changed_files(status_short: &str) -> Vec<String> {
    let entries = status_short.lines().filter(|line| line.len() > 3);
    entries
        .filter(|line| !line[..2].contains('D'))
        .map(|line| {
            let path = &line[3..];
            let path = path.rsplit_once(" -> ").map_or(path, |(_, new)| new);
            path.trim_matches('"').to_string()
        })
        .collect()
}

/// Candidates for `request`; `exists` says whether a relative path is a
/// file of the project.
pub(super) fn candidates(
    request: &str,
    outline: &[(String, Vec<String>)],
    changed: &[String],
    exists: impl Fn(&str) -> bool,
) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = Vec::new();
    let mut add = |path: &str, why: &'static str| {
        if found.len() < MAX_CANDIDATES && !found.iter().any(|c| c.path == path) {
            let defines = outline.iter().find(|(p, _)| p == path);
            let defines = defines.map_or_else(Vec::new, |(_, names)| {
                names.iter().take(MAX_DEFINES).cloned().collect()
            });
            found.push(Candidate {
                path: path.to_string(),
                why,
                defines,
            });
        }
    };
    for token in path_tokens(request) {
        if exists(&token) {
            add(&token, NAMED);
            continue;
        }
        let tail = format!("/{token}");
        let same_name = outline.iter().filter(|(path, _)| path.ends_with(&tail));
        for (path, _) in same_name.take(2) {
            add(path, NAMED);
        }
    }
    let mentioned = words(request);
    for (path, names) in outline {
        let named = names.iter().any(|name| {
            name.chars().count() >= MIN_NAME_CHARS && mentioned.contains(name.as_str())
        });
        if named {
            add(path, DEFINES);
        }
    }
    for path in changed {
        add(path, CHANGED);
    }
    found
}

/// Tokens that look like file paths: a `/` or a short extension.
fn path_tokens(request: &str) -> Vec<String> {
    let trim = |c: char| !(c.is_alphanumeric() || matches!(c, '/' | '_' | '-' | '.'));
    request
        .split_whitespace()
        .map(|token| token.trim_matches(trim).trim_end_matches('.'))
        .filter(|token| {
            let has_ext = token.rsplit_once('.').is_some_and(|(stem, ext)| {
                !stem.is_empty()
                    && (1..=5).contains(&ext.len())
                    && ext.chars().all(char::is_alphanumeric)
            });
            token.contains('/') || has_ext
        })
        .map(|token| token.trim_start_matches("./").to_string())
        .collect()
}

/// Identifier-like words of `text`.
fn words(text: &str) -> HashSet<&str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "Repository map:\nsrc/parser.rs:\n  fn parse_header (L3)\n  struct Lexer (L9)\n    fn new (L10)\nsrc/main.rs:\n  fn main (L1)\n(2 more files omitted)";

    #[test]
    fn the_map_outline_lists_paths_and_names() {
        let files = outline(MAP);
        assert_eq!(files.len(), 2);
        assert_eq!(
            files[0],
            (
                "src/parser.rs".into(),
                vec!["parse_header".into(), "Lexer".into(), "new".into()]
            )
        );
        assert_eq!(files[1].1, ["main"]);
    }

    #[test]
    fn git_status_lines_give_changed_paths() {
        let status = " M src/a.rs\n?? notes.md\nD  gone.rs\nR  old.rs -> src/new.rs\n";
        assert_eq!(
            changed_files(status),
            ["src/a.rs", "notes.md", "src/new.rs"]
        );
    }

    #[test]
    fn named_files_then_definitions_then_changes() {
        let files = outline(MAP);
        let changed = vec!["src/main.rs".to_string(), "README.md".to_string()];
        let exists = |path: &str| path == "Cargo.toml";
        let found = candidates(
            "Fix `Lexer` in parser.rs, see Cargo.toml.",
            &files,
            &changed,
            exists,
        );
        let paths: Vec<(&str, &str)> = found.iter().map(|c| (c.path.as_str(), c.why)).collect();
        assert_eq!(
            paths,
            [
                ("src/parser.rs", NAMED),
                ("Cargo.toml", NAMED),
                ("src/main.rs", CHANGED),
                ("README.md", CHANGED)
            ]
        );
        assert_eq!(found[0].defines.len(), 3);
        let by_name = candidates("rename parse_header", &files, &[], |_| false);
        assert_eq!(by_name[0].why, DEFINES);
        assert!(
            candidates("make main faster", &files, &[], |_| false).is_empty(),
            "short names"
        );
    }

    #[test]
    fn at_most_eight_candidates() {
        let changed: Vec<String> = (0..20).map(|i| format!("f{i}.rs")).collect();
        assert_eq!(
            candidates("go", &[], &changed, |_| false).len(),
            MAX_CANDIDATES
        );
    }
}
