//! Fallback matching for an `old_string` that does not occur verbatim.
//!
//! 1. **whitespace**: whole lines equal after collapsing whitespace, so
//!    indentation and spacing drift is tolerated;
//! 2. **fuzzy**: one block of lines at least 90% similar (normalized
//!    Levenshtein over whitespace-normalized text).
//!
//! Both rungs select whole lines and only ever accept a single region: when
//! several places qualify the edit is refused as ambiguous.

use strsim::normalized_levenshtein;

pub(crate) const FUZZY_THRESHOLD: f64 = 0.9;
/// Normalized old strings shorter than this are never matched fuzzily.
const FUZZY_MIN_CHARS: usize = 12;
/// Levenshtein cells the fuzzy rung may compute before giving up.
const FUZZY_BUDGET: u64 = 150_000_000;
/// Near misses below this similarity are not worth pointing at.
const HINT_THRESHOLD: f64 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Rung {
    Whitespace,
    Fuzzy(f64),
}

/// A matched block of whole lines (0-based `start`, `len` lines).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Region {
    pub(crate) start: usize,
    pub(crate) len: usize,
    pub(crate) rung: Rung,
    /// Indentation the file has beyond `old_string`'s, re-applied to
    /// `new_string` so its lines land at the file's indentation.
    pub(crate) indent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Miss {
    /// `count` separate regions qualified on `rung`.
    Ambiguous { count: usize, rung: Rung },
    /// Nothing qualified; `closest` is the most similar block, if any.
    NotFound {
        closest: Option<(usize, usize, f64)>,
    },
}

/// Finds `old` in `lines` (file lines without terminators).
pub(crate) fn locate(lines: &[&str], old: &str) -> Result<Region, Miss> {
    let old_lines: Vec<&str> = old.strip_suffix('\n').unwrap_or(old).split('\n').collect();
    let k = old_lines.len();
    if lines.len() < k {
        return Err(Miss::NotFound { closest: None });
    }
    let norm_old: Vec<String> = old_lines.iter().map(|line| normalize(line)).collect();
    let norm_file: Vec<String> = lines.iter().map(|line| normalize(line)).collect();

    if norm_old.iter().any(|line| !line.is_empty()) {
        let hits: Vec<usize> = (0..=lines.len() - k)
            .filter(|&i| norm_file[i..i + k] == norm_old[..])
            .collect();
        match hits.as_slice() {
            [start] => {
                return Ok(Region {
                    start: *start,
                    len: k,
                    rung: Rung::Whitespace,
                    indent: indent_delta(&lines[*start..*start + k], &old_lines),
                });
            }
            [] => {}
            many => {
                return Err(Miss::Ambiguous {
                    count: many.len(),
                    rung: Rung::Whitespace,
                });
            }
        }
    }
    fuzzy(&norm_file, &norm_old, lines, &old_lines)
}

fn fuzzy(
    norm_file: &[String],
    norm_old: &[String],
    lines: &[&str],
    old_lines: &[&str],
) -> Result<Region, Miss> {
    let k = norm_old.len();
    let target = norm_old.join("\n");
    let target_len = target.chars().count();
    let char_lens: Vec<usize> = norm_file.iter().map(|line| line.chars().count()).collect();
    let mut candidates: Vec<(usize, f64)> = Vec::new();
    let mut closest: Option<(usize, f64)> = None;
    let mut spent: u64 = 0;
    for start in 0..=norm_file.len() - k {
        let window_len = char_lens[start..start + k].iter().sum::<usize>() + (k - 1);
        let longest = window_len.max(target_len).max(1);
        // Levenshtein is at least the length difference.
        let ceiling = 1.0 - window_len.abs_diff(target_len) as f64 / longest as f64;
        if ceiling < HINT_THRESHOLD {
            continue;
        }
        spent += (window_len as u64) * (target_len as u64);
        if spent > FUZZY_BUDGET {
            return Err(Miss::NotFound { closest: None });
        }
        let score = normalized_levenshtein(&norm_file[start..start + k].join("\n"), &target);
        if closest.is_none_or(|(_, best)| score > best) {
            closest = Some((start, score));
        }
        if score >= FUZZY_THRESHOLD {
            candidates.push((start, score));
        }
    }
    let hint = closest
        .filter(|(_, score)| *score >= HINT_THRESHOLD)
        .map(|(start, score)| (start, k, score));
    if target_len < FUZZY_MIN_CHARS {
        return Err(Miss::NotFound { closest: hint });
    }
    let Some(&(best, score)) = candidates
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    else {
        return Err(Miss::NotFound { closest: hint });
    };
    // Windows overlapping the best one are the same place; count the other
    // places as clusters of overlapping windows.
    let mut separate = 1;
    let mut cluster: Option<usize> = None;
    for &(start, _) in candidates.iter().filter(|(s, _)| s.abs_diff(best) >= k) {
        if cluster.is_none_or(|first| start - first >= k) {
            separate += 1;
            cluster = Some(start);
        }
    }
    if separate > 1 {
        return Err(Miss::Ambiguous {
            count: separate,
            rung: Rung::Fuzzy(score),
        });
    }
    Ok(Region {
        start: best,
        len: k,
        rung: Rung::Fuzzy(score),
        indent: indent_delta(&lines[best..best + k], old_lines),
    })
}

/// Collapses runs of whitespace and trims the ends.
fn normalize(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn leading_ws(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// The indentation every non-blank file line has in addition to the
/// matching `old` line, when that extra prefix is the same for all of them.
fn indent_delta(file: &[&str], old: &[&str]) -> Option<String> {
    let mut delta: Option<&str> = None;
    for (file_line, old_line) in file.iter().zip(old) {
        if file_line.trim().is_empty() || old_line.trim().is_empty() {
            continue;
        }
        let extra = leading_ws(file_line).strip_suffix(leading_ws(old_line))?;
        match delta {
            Some(seen) if seen != extra => return None,
            _ => delta = Some(extra),
        }
    }
    delta.filter(|extra| !extra.is_empty()).map(str::to_string)
}

/// `new` with `indent` added before every non-blank line.
pub(crate) fn reindent(new: &str, indent: Option<&str>) -> String {
    let Some(indent) = indent else {
        return new.to_string();
    };
    new.split_inclusive('\n')
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<&str> {
        text.lines().collect()
    }

    #[test]
    fn whitespace_rung_restores_the_files_indentation() {
        let file = lines("fn a() {\n    if x {\n        y();\n    }\n}");
        let region = locate(&file, "if x {\n    y();\n}").unwrap();
        assert_eq!((region.start, region.len), (1, 3));
        assert_eq!(region.rung, Rung::Whitespace);
        assert_eq!(region.indent.as_deref(), Some("    "));
        assert_eq!(reindent("if z {\n}\n", Some("  ")), "  if z {\n  }\n");
    }

    #[test]
    fn repeated_blocks_are_ambiguous() {
        let file = lines("  foo( a )\nbar\n  foo( a )");
        let miss = locate(&file, "foo(  a  )").unwrap_err();
        assert!(matches!(miss, Miss::Ambiguous { count: 2, .. }), "{miss:?}");
    }

    #[test]
    fn fuzzy_rung_accepts_one_near_identical_block() {
        let file = lines("header\npub fn calc(a: u32, b: u32) -> u32 {\n    a - b\n}\ntail");
        let region = locate(&file, "pub fn calc(a: u32, b: u32) -> u32 {\n    a + b\n}").unwrap();
        assert_eq!((region.start, region.len), (1, 3));
        assert!(matches!(region.rung, Rung::Fuzzy(score) if score >= FUZZY_THRESHOLD));
    }

    #[test]
    fn fuzzy_duplicates_and_distant_text_are_refused() {
        let file = lines("alpha beta gamma delta\nsep\nalpha beta gamma delta");
        let miss = locate(&file, "alpha bets gamma delta").unwrap_err();
        assert!(matches!(
            miss,
            Miss::Ambiguous {
                count: 2,
                rung: Rung::Fuzzy(_)
            }
        ));
        let miss = locate(&lines("one two three"), "zzz qqq www eee").unwrap_err();
        assert_eq!(miss, Miss::NotFound { closest: None });
    }

    #[test]
    fn near_misses_are_reported_as_hints() {
        let file = lines("let total = compute_total(items, tax_rate);\nother");
        let miss = locate(&file, "let total = compute_sum(items, rate);").unwrap_err();
        assert!(
            matches!(
                miss,
                Miss::NotFound {
                    closest: Some((0, 1, _))
                }
            ),
            "{miss:?}"
        );
    }
}
