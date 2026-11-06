//! Applies `Edit` and `MultiEdit` replacements to file content in memory:
//! exact matches first (counted, `replace_all` for several), then the
//! fallback ladder for a single unambiguous region. Files that consistently
//! use CRLF are matched with LF text and written back with CRLF.

use super::ladder::{self, Miss, Region, Rung};
use crate::text::numbered;

/// Lines of context the "most similar text" hint shows at most.
const HINT_MAX_LINES: usize = 20;
const HINT_LINE_CHARS: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditSpec {
    pub(crate) old: String,
    pub(crate) new: String,
    pub(crate) replace_all: bool,
}

/// How one edit found its target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MatchKind {
    Exact,
    /// Lines `first..=last` (1-based) matched on a ladder rung.
    Ladder {
        rung: Rung,
        first: usize,
        last: usize,
    },
    /// An empty `old_string` created (or filled) the file.
    Created,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EditReport {
    pub(crate) replacements: usize,
    pub(crate) kind: MatchKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Applied {
    pub(crate) content: String,
    pub(crate) reports: Vec<EditReport>,
}

/// Why edit number `index` (0-based) could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditFailure {
    pub(crate) index: usize,
    pub(crate) message: String,
}

/// Applies `edits` in order; `content` is `None` when the file does not
/// exist. All-or-nothing: the first failure is returned and nothing else.
pub(crate) fn apply_edits(
    content: Option<&str>,
    edits: &[EditSpec],
) -> Result<Applied, EditFailure> {
    let crlf = content.is_some_and(is_crlf);
    let mut current: Option<String> = content.map(|text| lf(text, crlf));
    let mut reports = Vec::with_capacity(edits.len());
    for (index, edit) in edits.iter().enumerate() {
        let fail = |message: String| EditFailure { index, message };
        let old = lf(&edit.old, crlf);
        let new = lf(&edit.new, crlf);
        if old == new {
            return Err(fail(
                "old_string and new_string are identical, so there is nothing to change".into(),
            ));
        }
        let (next, report) = match current.as_deref() {
            None if old.is_empty() => (new, created()),
            None => {
                return Err(fail(
                    "the file does not exist. To create it, use Write, or pass an empty old_string with the new content".into(),
                ));
            }
            Some(text) if old.is_empty() => {
                if index > 0 || !text.trim().is_empty() {
                    return Err(fail(
                        "old_string is empty, which only creates a new file or fills an empty one. Give the exact text to replace".into(),
                    ));
                }
                (new, created())
            }
            Some(text) => replace(text, &old, &new, edit.replace_all).map_err(fail)?,
        };
        current = Some(next);
        reports.push(report);
    }
    let content = current.unwrap_or_default();
    Ok(Applied {
        content: if crlf {
            content.replace('\n', "\r\n")
        } else {
            content
        },
        reports,
    })
}

fn created() -> EditReport {
    EditReport {
        replacements: 1,
        kind: MatchKind::Created,
    }
}

fn replace(text: &str, old: &str, new: &str, all: bool) -> Result<(String, EditReport), String> {
    let count = text.matches(old).count();
    let exact = |replacements| EditReport {
        replacements,
        kind: MatchKind::Exact,
    };
    match count {
        1 => Ok((text.replacen(old, new, 1), exact(1))),
        n if n > 1 && all => Ok((text.replace(old, new), exact(n))),
        n if n > 1 => Err(format!(
            "old_string occurs {n} times in the file. Set replace_all to replace every occurrence, or include more surrounding lines so it matches exactly once"
        )),
        _ => replace_region(text, old, new),
    }
}

fn replace_region(text: &str, old: &str, new: &str) -> Result<(String, EditReport), String> {
    let lines: Vec<&str> = text.lines().collect();
    let region = ladder::locate(&lines, old).map_err(|miss| miss_message(&lines, miss))?;
    let report = EditReport {
        replacements: 1,
        kind: MatchKind::Ladder {
            rung: region.rung,
            first: region.start + 1,
            last: region.start + region.len,
        },
    };
    Ok((splice(text, &region, new), report))
}

/// `text` with the region's whole lines replaced by `new`.
fn splice(text: &str, region: &Region, new: &str) -> String {
    let pieces: Vec<&str> = text.split_inclusive('\n').collect();
    let end = region.start + region.len;
    let ends_with_newline = pieces[region.start..end]
        .last()
        .is_some_and(|piece| piece.ends_with('\n'));
    let mut middle = ladder::reindent(new, region.indent.as_deref());
    if ends_with_newline && !middle.is_empty() && !middle.ends_with('\n') {
        middle.push('\n');
    }
    let mut out = pieces[..region.start].concat();
    out.push_str(&middle);
    out.push_str(&pieces[end..].concat());
    out
}

fn miss_message(lines: &[&str], miss: Miss) -> String {
    match miss {
        Miss::Ambiguous { count, rung } => format!(
            "old_string was not found exactly, and {count} places match it {}. Include more surrounding lines so it matches exactly once",
            rung_label(rung)
        ),
        Miss::NotFound { closest } => {
            let mut message = "old_string was not found in the file. It must match exactly, including whitespace and indentation, without the line-number prefix Read adds. Read the file again and copy the text exactly".to_string();
            if let Some((start, len, score)) = closest {
                let shown = len.min(HINT_MAX_LINES);
                message.push_str(&format!(
                    ".\nThe most similar text is at lines {}-{} (similarity {score:.2}):\n{}",
                    start + 1,
                    start + len,
                    numbered(
                        lines[start..start + shown].iter().copied(),
                        start + 1,
                        HINT_LINE_CHARS
                    )
                ));
            }
            message
        }
    }
}

/// "after normalizing whitespace" / "approximately (similarity 0.93)".
pub(crate) fn rung_label(rung: Rung) -> String {
    match rung {
        Rung::Whitespace => "after normalizing whitespace".to_string(),
        Rung::Fuzzy(score) => format!("approximately (similarity {score:.2})"),
    }
}

/// Every line break is CRLF.
fn is_crlf(text: &str) -> bool {
    let breaks = text.matches('\n').count();
    breaks > 0 && text.matches("\r\n").count() == breaks
}

fn lf(text: &str, crlf: bool) -> String {
    if crlf {
        text.replace("\r\n", "\n")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(old: &str, new: &str, all: bool) -> EditSpec {
        EditSpec {
            old: old.into(),
            new: new.into(),
            replace_all: all,
        }
    }

    #[test]
    fn exact_single_all_and_ambiguous() {
        let one = apply_edits(Some("a\nb\nc\n"), &[spec("b", "B", false)]).unwrap();
        assert_eq!(one.content, "a\nB\nc\n");
        let all = apply_edits(Some("x x x"), &[spec("x", "y", true)]).unwrap();
        assert_eq!(
            (all.content.as_str(), all.reports[0].replacements),
            ("y y y", 3)
        );
        let err = apply_edits(Some("x x"), &[spec("x", "y", false)]).unwrap_err();
        assert!(err.message.contains("occurs 2 times"), "{}", err.message);
    }

    #[test]
    fn crlf_files_keep_their_line_endings() {
        let out = apply_edits(Some("one\r\ntwo\r\n"), &[spec("one\ntwo", "1\n2", false)]).unwrap();
        assert_eq!(out.content, "1\r\n2\r\n");
    }

    #[test]
    fn ladder_edits_replace_whole_lines_at_the_files_indentation() {
        let text = "fn a() {\n    let x = 1;\n    call( x );\n}\n";
        let spaced = [spec("let x  = 1;\ncall( x );", "let x = 2;", false)];
        let out = apply_edits(Some(text), &spaced).unwrap();
        assert_eq!(out.content, "fn a() {\n    let x = 2;\n}\n");
        assert!(matches!(
            out.reports[0].kind,
            MatchKind::Ladder {
                rung: Rung::Whitespace,
                first: 2,
                last: 3
            }
        ));
        let drifted = [spec("let x = 1;\ncall(x);", "let x = 2;\ncall(x);", false)];
        let out = apply_edits(Some(text), &drifted).unwrap();
        assert_eq!(out.content, "fn a() {\n    let x = 2;\n    call(x);\n}\n");
        assert!(matches!(
            out.reports[0].kind,
            MatchKind::Ladder {
                rung: Rung::Fuzzy(_),
                first: 2,
                last: 3
            }
        ));
        let missing = apply_edits(Some(text), &[spec("return nothing_like_this;", "x", false)]);
        assert!(missing.unwrap_err().message.contains("not found"));
    }

    #[test]
    fn empty_old_string_only_creates() {
        let created =
            apply_edits(None, &[spec("", "new\n", false), spec("new", "NEW", false)]).unwrap();
        assert_eq!(created.content, "NEW\n");
        let err = apply_edits(Some("full"), &[spec("", "x", false)]).unwrap_err();
        assert!(err.message.contains("only creates"), "{}", err.message);
        let err = apply_edits(None, &[spec("a", "b", false)]).unwrap_err();
        assert!(err.message.contains("does not exist"));
    }

    #[test]
    fn failures_name_the_edit_and_apply_nothing() {
        let err = apply_edits(
            Some("a b"),
            &[spec("a", "A", false), spec("zzz", "q", false)],
        )
        .unwrap_err();
        assert_eq!(err.index, 1);
        let same = apply_edits(Some("a"), &[spec("a", "a", false)]).unwrap_err();
        assert!(same.message.contains("identical"));
    }
}
