//! Entry point: outline, rank and render the files the caller read.

use tree_sitter::Parser;

use super::language::Lang;
use super::rank::rank;
use super::render::render;
use super::symbol::Symbol;

/// Larger files (usually generated or minified) are neither outlined nor
/// counted as references.
const MAX_SOURCE_BYTES: usize = 1_000_000;

/// A file whose contents the caller has already read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// Path relative to the project root, as shown in the map.
    pub path: String,
    pub text: String,
}

/// A compact definition map of `files` within `budget_chars` characters.
///
/// Files with a [`supported_extension`](super::supported_extension) are
/// outlined with tree-sitter; files that yield no tree or no definitions
/// are left out but still count as references. Files are ranked by how
/// often other files mention the names they define, with `focus` files
/// (paths relative or absolute) first and their reference neighbours next;
/// the lowest-ranked files are dropped first when over budget.
pub fn repo_map(files: &[SourceFile], focus: &[String], budget_chars: usize) -> String {
    repo_map_ranked(files, focus, &[], budget_chars)
}

/// [`repo_map`] with `relevant` files (matched like `focus`) ranked right
/// after the focus files, their neighbours next.
pub fn repo_map_ranked(
    files: &[SourceFile],
    focus: &[String],
    relevant: &[String],
    budget_chars: usize,
) -> String {
    let texts: Vec<&str> = files
        .iter()
        .map(|file| {
            if file.text.len() > MAX_SOURCE_BYTES {
                ""
            } else {
                file.text.as_str()
            }
        })
        .collect();
    let mut parser = Parser::new();
    let outlines: Vec<Vec<Symbol>> = files
        .iter()
        .zip(&texts)
        .map(|(file, text)| {
            Lang::from_path(&file.path)
                .and_then(|lang| lang.outline(&mut parser, text))
                .unwrap_or_default()
        })
        .collect();
    let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    let focused: Vec<bool> = paths.iter().map(|path| is_focus(path, focus)).collect();
    let picked: Vec<bool> = paths.iter().map(|path| is_focus(path, relevant)).collect();
    let order = rank(&texts, &outlines, &focused, &picked, &paths);
    render(&paths, &outlines, &order, budget_chars)
}

/// A focus entry matches a path exactly (ignoring a leading `./`) or ends
/// with `/<path>`, so absolute paths of tracked files match too.
fn is_focus(path: &str, focus: &[String]) -> bool {
    let path = path.strip_prefix("./").unwrap_or(path);
    !path.is_empty()
        && focus.iter().any(|entry| {
            let entry = entry.strip_prefix("./").unwrap_or(entry);
            entry == path
                || entry
                    .strip_suffix(path)
                    .is_some_and(|prefix| prefix.ends_with('/'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_matches_relative_dotted_and_absolute_paths() {
        let focus = vec![
            "./src/lib.rs".to_string(),
            "/work/app/ui/App.tsx".to_string(),
        ];
        assert!(is_focus("src/lib.rs", &focus));
        assert!(is_focus("./src/lib.rs", &focus));
        assert!(is_focus("ui/App.tsx", &focus));
        assert!(!is_focus(
            "App.tsx",
            &["/work/app/ui/MyApp.tsx".to_string()]
        ));
        assert!(!is_focus("lib.rs", &focus[1..]));
        assert!(!is_focus("", &focus));
    }
}
