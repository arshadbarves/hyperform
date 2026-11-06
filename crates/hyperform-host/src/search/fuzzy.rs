//! Subsequence scoring for `@`-mention completion, after fzy: every query
//! character must appear in order; matches at path-segment and word starts,
//! camel humps, and consecutive runs score higher, gaps cost a little.

const MIN: i64 = i64::MIN / 4;
const GAP_LEADING: i64 = -5;
const GAP_TRAILING: i64 = -5;
const GAP_INNER: i64 = -10;
const CONSECUTIVE: i64 = 1000;
const AFTER_SLASH: i64 = 900;
const AFTER_WORD: i64 = 800;
const CAPITAL: i64 = 700;
const AFTER_DOT: i64 = 600;

/// Score of `needle` (already lowercased) against `haystack`, or `None`
/// when it is not a subsequence.
pub(crate) fn score(needle: &[char], haystack: &str) -> Option<i64> {
    let chars: Vec<char> = haystack.chars().collect();
    let lower: Vec<char> = chars.iter().map(|c| fold(*c)).collect();
    if needle.is_empty() {
        return Some(0);
    }
    if !is_subsequence(needle, &lower) {
        return None;
    }
    let bonus = bonuses(&chars);
    let columns = chars.len();
    // best[j]: best score of the needle so far with its last char at or
    // before j; ending[j]: best score with the last char exactly at j.
    let mut best = vec![MIN; columns];
    let mut ending = vec![MIN; columns];
    for (i, &wanted) in needle.iter().enumerate() {
        let gap = if i + 1 == needle.len() {
            GAP_TRAILING
        } else {
            GAP_INNER
        };
        let mut next_best = vec![MIN; columns];
        let mut next_ending = vec![MIN; columns];
        let mut running = MIN;
        for j in 0..columns {
            if lower[j] == wanted {
                let here = if i == 0 {
                    GAP_LEADING.saturating_mul(j as i64) + bonus[j]
                } else if j > 0 {
                    (best[j - 1].saturating_add(bonus[j]))
                        .max(ending[j - 1].saturating_add(CONSECUTIVE))
                } else {
                    MIN
                };
                next_ending[j] = here;
                running = here.max(running.saturating_add(gap));
            } else {
                running = running.saturating_add(gap);
            }
            next_best[j] = running;
        }
        best = next_best;
        ending = next_ending;
    }
    best.last().copied()
}

pub(crate) fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn is_subsequence(needle: &[char], hay: &[char]) -> bool {
    let mut rest = hay.iter();
    needle.iter().all(|wanted| rest.any(|c| c == wanted))
}

fn bonuses(chars: &[char]) -> Vec<i64> {
    let mut previous = '/';
    chars
        .iter()
        .map(|&c| {
            let bonus = match previous {
                '/' | '\\' => AFTER_SLASH,
                '-' | '_' | ' ' => AFTER_WORD,
                '.' => AFTER_DOT,
                p if p.is_lowercase() && c.is_uppercase() => CAPITAL,
                _ => 0,
            };
            previous = c;
            bonus
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn needle(q: &str) -> Vec<char> {
        q.chars().map(fold).collect()
    }

    #[test]
    fn requires_an_ordered_subsequence() {
        assert!(score(&needle("mn"), "src/main.rs").is_some());
        assert!(score(&needle("nm"), "src/main.rs").is_none());
    }

    #[test]
    fn prefers_segment_starts_and_consecutive_runs() {
        let q = needle("main");
        let tight = score(&q, "src/main.rs").unwrap();
        let loose = score(&q, "src/my_animation.rs").unwrap();
        assert!(tight > loose);
        let q = needle("fb");
        assert!(score(&q, "foo/bar.rs").unwrap() > score(&q, "xfxb.rs").unwrap());
        assert!(
            score(&needle("gs"), "getSomething").unwrap() > score(&needle("gs"), "gasket").unwrap()
        );
    }
}
