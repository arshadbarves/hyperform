//! Whether two path patterns can name the same file, decided lexically. A
//! pattern is split into `/` segments; a `**` segment stands for any number
//! of segments. Within a segment `*` is any run of characters, `?` and a
//! `[...]` class one character, and `{a,b}` either alternative. When the
//! syntax is unclear the answer leans to yes.

/// `a` and `b` can name the same file, where a literal also covers
/// everything under it (as `scope::matches` does).
pub(crate) fn intersects(a: &str, b: &str) -> bool {
    meets(&covering(a), &covering(b), double_star, |x, y| {
        segment(x, y)
    })
}

/// `named` (a task scope) names a file `pattern` covers. The scope's
/// trailing `**`, and what a literal directory holds, do not count: a
/// scope of `crates/app/**` does not name `crates/app/Cargo.toml`, while
/// `crates/app/Cargo.toml` and `**/*.toml` do.
pub(crate) fn names(named: &str, pattern: &str) -> bool {
    let mut own = segments(named);
    while own.last().is_some_and(|last| *last == "**") {
        own.pop();
    }
    !own.is_empty() && meets(&own, &covering(pattern), double_star, |x, y| segment(x, y))
}

fn double_star(part: &&str) -> bool {
    *part == "**"
}

fn segments(pattern: &str) -> Vec<&str> {
    pattern
        .trim()
        .trim_start_matches("./")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect()
}

fn covering(pattern: &str) -> Vec<&str> {
    let mut parts = segments(pattern);
    if !crate::scope::is_glob(pattern) {
        parts.push("**");
    }
    parts
}

/// The two sequences can match the same sequence, where a `wild` element
/// matches any number of elements and others must `meet` one-to-one.
fn meets<T>(a: &[T], b: &[T], wild: impl Fn(&T) -> bool, meet: impl Fn(&T, &T) -> bool) -> bool {
    let (la, lb) = (a.len(), b.len());
    let mut can = vec![vec![false; lb + 1]; la + 1];
    can[la][lb] = true;
    for i in (0..=la).rev() {
        for j in (0..=lb).rev() {
            if i == la && j == lb {
                continue;
            }
            let (wa, wb) = (i < la && wild(&a[i]), j < lb && wild(&b[j]));
            can[i][j] = if wa {
                can[i + 1][j] || (j < lb && can[i][j + 1])
            } else if wb {
                can[i][j + 1] || (i < la && can[i + 1][j])
            } else {
                i < la && j < lb && meet(&a[i], &b[j]) && can[i + 1][j + 1]
            };
        }
    }
    can[0][0]
}

fn segment(a: &str, b: &str) -> bool {
    let (left, right) = (alternatives(a), alternatives(b));
    left.iter().any(|x| {
        right.iter().any(|y| {
            meets(
                &tokens(x),
                &tokens(y),
                |t| *t == Tok::Star,
                |s, t| *s == Tok::One || *t == Tok::One || s == t,
            )
        })
    })
}

/// `{x,y}` groups expanded; a group holding another group counts as `*`.
fn alternatives(segment: &str) -> Vec<String> {
    let Some(open) = segment.find('{') else {
        return vec![segment.to_string()];
    };
    let mut depth = 0;
    let close = segment[open..].char_indices().find_map(|(at, c)| {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        (depth == 0).then_some(open + at)
    });
    let Some(close) = close else {
        return vec![segment.to_string()];
    };
    let (head, body, tail) = (
        &segment[..open],
        &segment[open + 1..close],
        &segment[close + 1..],
    );
    if body.contains('{') {
        return alternatives(&format!("{head}*{tail}"));
    }
    body.split(',')
        .flat_map(|choice| alternatives(&format!("{head}{choice}{tail}")))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok {
    Star,
    One,
    Lit(char),
}

fn tokens(segment: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = segment.chars();
    while let Some(c) = chars.next() {
        match c {
            '*' if out.last() == Some(&Tok::Star) => {}
            '*' => out.push(Tok::Star),
            '?' => out.push(Tok::One),
            '[' => {
                chars.by_ref().find(|c| *c == ']');
                out.push(Tok::One);
            }
            '\\' => out.push(Tok::Lit(chars.next().unwrap_or('\\'))),
            c => out.push(Tok::Lit(c)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_cover_what_is_under_them() {
        assert!(intersects("src", "src/lib.rs"));
        assert!(intersects("src/lib.rs", "src/lib.rs"));
        assert!(!intersects("src/lib.rs", "src/main.rs"));
        assert!(!intersects("src/auth", "src/authz.rs"));
        assert!(intersects("crates/app", "**/Cargo.toml"));
    }

    #[test]
    fn stars_stay_in_a_segment_and_double_stars_cross() {
        assert!(!intersects("src/*.rs", "src/*.toml"));
        assert!(intersects("src/*.rs", "src/lib.*"));
        assert!(!intersects("src/*.rs", "src/a/b.rs"));
        assert!(intersects("src/**/*.rs", "src/a/b.rs"));
        assert!(intersects("src/**", "**/*.md"));
        assert!(!intersects("src/a/**", "src/b/**"));
        assert!(intersects("**", "anything/at/all"));
        assert!(intersects("a?c.rs", "abc.rs"));
        assert!(!intersects("a?c.rs", "abcd.rs"));
        assert!(intersects("src/[ab].rs", "src/a.rs"));
    }

    #[test]
    fn braces_are_alternatives() {
        assert!(intersects("src/*.{rs,toml}", "src/Cargo.toml"));
        assert!(!intersects("src/*.{rs,toml}", "src/a.md"));
        assert!(intersects("src/{a,{b,c}}.rs", "src/c.rs"));
    }

    #[test]
    fn naming_ignores_what_a_trailing_double_star_reaches() {
        assert!(names("Cargo.toml", "**/Cargo.toml"));
        assert!(names("crates/app/Cargo.toml", "**/Cargo.toml"));
        assert!(names("**/*.toml", "**/Cargo.toml"));
        assert!(!names("crates/app/**", "**/Cargo.toml"));
        assert!(!names("crates/app", "**/Cargo.toml"));
        assert!(names("ui/src/lib/protocol/**", "ui/src/lib/protocol/**"));
        assert!(names(
            "ui/src/lib/protocol/Task.ts",
            "ui/src/lib/protocol/**"
        ));
        assert!(!names("ui/**", "ui/src/lib/protocol/**"));
        assert!(!names("**", "**/Cargo.lock"));
    }
}
