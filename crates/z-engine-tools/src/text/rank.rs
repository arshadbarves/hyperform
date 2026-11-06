//! Search ranking before the result cap (`decisions_search_rank`): when
//! Grep or Glob found more than they can show, the decision model scores
//! the results for the task and the useful ones are shown first, so they
//! survive the cut. Below the cap, without a ranker or without an answer,
//! results keep today's order.

use crate::context::ToolCtx;
use crate::ports::{RankRequest, RankTarget};

/// Characters of one result the model reads.
const EXCERPT_CHARS: usize = 400;

/// Result indices in the order to show them: results the model called
/// relevant, then unknown ones, then the rest, each group in result order.
/// `None` keeps today's order.
pub(crate) async fn rank_before_cut(
    ctx: &ToolCtx,
    tool: &str,
    subject: &str,
    entries: &[String],
) -> Option<Vec<usize>> {
    let ranker = ctx.ports.ranker(RankTarget::SearchHits)?;
    let request = RankRequest {
        target: RankTarget::SearchHits,
        tool: tool.to_string(),
        subject: subject.to_string(),
        items: entries
            .iter()
            .map(|entry| entry.chars().take(EXCERPT_CHARS).collect())
            .collect(),
        chars: entries.iter().map(|entry| entry.chars().count()).sum(),
    };
    let answers = ranker.rank(ctx, request).await?;
    relevance_order(&answers, entries.len())
}

/// `None` when the answers do not fit or change nothing.
fn relevance_order(answers: &[Option<bool>], count: usize) -> Option<Vec<usize>> {
    if answers.len() != count || answers.iter().all(Option::is_none) {
        return None;
    }
    let group = |wanted: Option<bool>| (0..count).filter(move |&i| answers[i] == wanted);
    let order: Vec<usize> = group(Some(true))
        .chain(group(None))
        .chain(group(Some(false)))
        .collect();
    order
        .iter()
        .enumerate()
        .any(|(at, &i)| at != i)
        .then_some(order)
}

/// ripgrep output split into results: one per line, or in content mode
/// one per file (its lines, without `--` separators).
pub(crate) fn grep_entries(text: &str, content: bool) -> Vec<String> {
    if !content {
        return text.lines().map(str::to_string).collect();
    }
    let mut entries: Vec<(String, String)> = Vec::new();
    for line in text.lines().filter(|line| *line != "--") {
        let path = file_of(line);
        match entries.last_mut() {
            Some((last, lines)) if *last == path => {
                lines.push('\n');
                lines.push_str(line);
            }
            _ => entries.push((path.to_string(), line.to_string())),
        }
    }
    entries.into_iter().map(|(_, lines)| lines).collect()
}

/// The path of a `path:line:text` or `path-line-text` line: everything
/// before the first `:N:` or `-N-` separator.
fn file_of(line: &str) -> &str {
    let bytes = line.as_bytes();
    for (at, &byte) in bytes.iter().enumerate() {
        if byte != b':' && byte != b'-' {
            continue;
        }
        let digits = bytes[at + 1..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits > 0 && bytes.get(at + 1 + digits) == Some(&byte) {
            return &line[..at];
        }
    }
    line
}

/// Results in `order` while they fit in `budget` characters, joined by
/// newlines; with the count shown.
pub(crate) fn fit_entries(entries: &[String], order: &[usize], budget: usize) -> (String, usize) {
    let mut out = String::new();
    let mut shown = 0;
    for &index in order {
        let entry = &entries[index];
        if out.len() + entry.len() + 1 > budget {
            break;
        }
        out.push_str(entry);
        out.push('\n');
        shown += 1;
    }
    (out, shown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevant_results_come_first_in_result_order() {
        let order = relevance_order(&[Some(false), None, Some(true), Some(true)], 4);
        assert_eq!(order, Some(vec![2, 3, 1, 0]));
        assert_eq!(relevance_order(&[None, None], 2), None, "no answer");
        assert_eq!(relevance_order(&[Some(true), None], 2), None, "no change");
        assert_eq!(relevance_order(&[Some(true)], 2), None, "wrong length");
    }

    #[test]
    fn content_results_group_by_file() {
        let text = "src/a.rs:3:fn a()\nsrc/a.rs-4-  body\n--\nsrc/b-c.rs:10:fn b()\nREADME:1:x";
        let entries = grep_entries(text, true);
        assert_eq!(
            entries,
            [
                "src/a.rs:3:fn a()\nsrc/a.rs-4-  body",
                "src/b-c.rs:10:fn b()",
                "README:1:x"
            ]
        );
        assert_eq!(grep_entries("a.rs\nb.rs", false), ["a.rs", "b.rs"]);
    }

    #[test]
    fn fitting_stops_at_the_budget() {
        let entries: Vec<String> = ["aaaa", "bb", "cccccc"].map(String::from).into();
        assert_eq!(
            fit_entries(&entries, &[2, 1, 0], 10),
            ("cccccc\nbb\n".into(), 2)
        );
        assert_eq!(fit_entries(&entries, &[0], 3), (String::new(), 0));
    }
}
