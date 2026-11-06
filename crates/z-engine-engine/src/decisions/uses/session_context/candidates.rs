//! Candidates for the first request: the project files and the deferred
//! MCP tools sharing the most content words with it, a few of each, so the
//! model is asked about likely ones only (one yes/no request each).

use std::collections::BTreeSet;

use super::super::guidance::{content_words, overlap};

/// Most files and most tools asked about.
pub(super) const MAX_FILES: usize = 12;
pub(super) const MAX_TOOLS: usize = 12;
/// Outline lines shown per file.
const OUTLINE_LINES: usize = 8;

/// Paths (map order) sharing a content word with the request, best first.
pub(super) fn files(request: &BTreeSet<String>, paths: &[String], map: &str) -> Vec<String> {
    let scored = paths.iter().map(|path| {
        let words = content_words(&format!("{path}\n{}", outline(map, path)));
        (overlap(request, &words), path)
    });
    best(scored, MAX_FILES).into_iter().cloned().collect()
}

/// Deferred tools (name, description) sharing a content word with the
/// request, best first.
pub(super) fn tools(
    request: &BTreeSet<String>,
    tools: Vec<(String, String)>,
) -> Vec<(String, String)> {
    let scored = tools.into_iter().map(|(name, description)| {
        let words = content_words(&format!("{name} {description}"));
        (overlap(request, &words), (name, description))
    });
    best(scored, MAX_TOOLS)
}

/// The outline lines the repository map lists under `path`.
pub(super) fn outline(map: &str, path: &str) -> String {
    let header = format!("{path}:");
    let mut lines = map.lines().skip_while(|line| line.trim_end() != header);
    if lines.next().is_none() {
        return String::new();
    }
    let body = lines.take_while(|line| line.starts_with(char::is_whitespace));
    body.take(OUTLINE_LINES)
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Highest scores first (ties keep their order), zero scores dropped.
fn best<T>(scored: impl Iterator<Item = (usize, T)>, limit: usize) -> Vec<T> {
    let mut kept: Vec<(usize, T)> = scored.filter(|(score, _)| *score > 0).collect();
    kept.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    kept.into_iter().take(limit).map(|(_, item)| item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "src/login/form.rs:\n  fn validate_login (L3)\n  struct LoginForm (L9)\nsrc/db.rs:\n  fn connect (L1)\n";

    #[test]
    fn files_sharing_words_with_the_request_come_first() {
        let request = content_words("fix the login form validation");
        let paths = vec!["src/db.rs".to_string(), "src/login/form.rs".to_string()];
        assert_eq!(files(&request, &paths, MAP), vec!["src/login/form.rs"]);
    }

    #[test]
    fn outlines_are_read_from_the_map() {
        assert_eq!(
            outline(MAP, "src/login/form.rs"),
            "fn validate_login (L3)\nstruct LoginForm (L9)"
        );
        assert_eq!(outline(MAP, "src/db.rs"), "fn connect (L1)");
        assert_eq!(outline(MAP, "src/none.rs"), "");
    }
}
