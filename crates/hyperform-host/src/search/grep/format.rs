//! The hit model both engines produce and the one renderer that turns it
//! into ripgrep-style text, so their output is identical by construction.

use std::path::Path;

use super::query::{GrepEngine, GrepMode, GrepQuery, GrepResult};

/// ripgrep `--max-columns 500 --max-columns-preview`.
const MAX_COLUMNS: usize = 500;
const OMITTED: &str = " [... omitted end of long line]";

#[derive(Debug)]
pub(super) struct FileMatches {
    /// Display path (relative to the root when inside it).
    pub(super) path: String,
    /// Printed lines in order (content mode only).
    pub(super) lines: Vec<HitLine>,
    /// Matching lines, or merged match regions in multiline mode.
    pub(super) count: usize,
}

#[derive(Debug)]
pub(super) struct HitLine {
    pub(super) number: u64,
    pub(super) text: String,
    pub(super) is_match: bool,
}

#[derive(Debug, Default)]
pub(super) struct Collected {
    pub(super) files: Vec<FileMatches>,
    /// A safety cap on collected output was hit.
    pub(super) overflow: bool,
}

/// ripgrep's long-line rule: a line over 500 bytes (terminator included)
/// keeps its first 500 characters plus a marker.
pub(super) fn preview(line: &str, terminated: bool) -> String {
    if line.len() + usize::from(terminated) <= MAX_COLUMNS {
        return line.to_string();
    }
    let head: String = line.chars().take(MAX_COLUMNS).collect();
    format!("{head}{OMITTED}")
}

pub(super) fn render(collected: Collected, query: &GrepQuery, engine: GrepEngine) -> GrepResult {
    let mut files = collected.files;
    files.sort_by(|a, b| Path::new(&a.path).cmp(Path::new(&b.path)));
    let lines = match query.mode {
        GrepMode::FilesWithMatches => files.iter().map(|f| f.path.clone()).collect(),
        GrepMode::Count => files
            .iter()
            .map(|f| format!("{}:{}", f.path, f.count))
            .collect(),
        GrepMode::Content => content_lines(&files, query),
    };
    let matches = match query.mode {
        GrepMode::FilesWithMatches => files.len(),
        GrepMode::Count => files.iter().map(|f| f.count).sum(),
        GrepMode::Content => files
            .iter()
            .map(|f| f.lines.iter().filter(|l| l.is_match).count())
            .sum(),
    };
    let total = lines.len();
    let window: Vec<String> = lines
        .into_iter()
        .skip(query.offset)
        .take(query.head_limit.unwrap_or(usize::MAX))
        .collect();
    let truncated = collected.overflow || query.offset.saturating_add(window.len()) < total;
    GrepResult {
        text: window.join("\n"),
        matches,
        files: files.len(),
        truncated,
        engine,
    }
}

/// `path:line:text` / `path-line-text`, with `--` between non-adjacent
/// groups when context was requested (ripgrep's layout).
fn content_lines(files: &[FileMatches], query: &GrepQuery) -> Vec<String> {
    let context = query.before > 0 || query.after > 0;
    let mut out = Vec::new();
    let mut previous: Option<(usize, u64)> = None;
    for (index, file) in files.iter().enumerate() {
        for line in &file.lines {
            if let (true, Some((last_file, last_number))) = (context, previous) {
                if last_file != index || line.number > last_number + 1 {
                    out.push("--".to_string());
                }
            }
            previous = Some((index, line.number));
            let sep = if line.is_match { ':' } else { '-' };
            out.push(if query.line_numbers {
                format!("{}{sep}{}{sep}{}", file.path, line.number, line.text)
            } else {
                format!("{}{sep}{}", file.path, line.text)
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_matches_ripgrep_thresholds() {
        let exact = "a".repeat(499);
        assert_eq!(preview(&exact, true), exact);
        let long = "a".repeat(500);
        assert_eq!(preview(&long, true), format!("{long}{OMITTED}"));
        assert_eq!(preview(&long, false), long);
        let wide = format!("{}tail", "é".repeat(300));
        assert_eq!(preview(&wide, true), format!("{wide}{OMITTED}"));
    }
}
