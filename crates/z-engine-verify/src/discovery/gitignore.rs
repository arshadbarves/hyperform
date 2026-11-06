//! The gitignore subset the discovery walk honors: `*`, `?`, `**`,
//! character classes, `\` escapes, `!` negation, trailing-`/` directory
//! patterns, and anchoring (a `/` before the end anchors a pattern to its
//! file's directory). Deeper files and later lines win. Children of an
//! ignored directory are never visited, so they cannot be re-included —
//! as in git.

use std::sync::Arc;

use regex::Regex;

/// The rules of one ignore file, whose directory is `base`.
#[derive(Debug)]
pub(crate) struct Rules {
    base: String,
    rules: Vec<Rule>,
}

#[derive(Debug)]
struct Rule {
    pattern: Regex,
    negated: bool,
    dir_only: bool,
}

impl Rules {
    pub(crate) fn parse(base: &str, text: &str) -> Self {
        Self {
            base: base.to_string(),
            rules: text.lines().filter_map(rule).collect(),
        }
    }

    /// `Some(true)` ignored, `Some(false)` re-included, `None` no rule
    /// matched `rel` (root-relative).
    fn decide(&self, rel: &str, is_dir: bool) -> Option<bool> {
        let local = if self.base.is_empty() {
            rel
        } else {
            rel.strip_prefix(self.base.as_str())?.strip_prefix('/')?
        };
        self.rules
            .iter()
            .rev()
            .find(|r| (is_dir || !r.dir_only) && r.pattern.is_match(local))
            .map(|r| !r.negated)
    }
}

/// Whether `rel` is ignored by the ignore files on its path (outermost
/// first).
pub(crate) fn ignored(stack: &[Arc<Rules>], rel: &str, is_dir: bool) -> bool {
    stack
        .iter()
        .rev()
        .find_map(|rules| rules.decide(rel, is_dir))
        .unwrap_or(false)
}

fn rule(line: &str) -> Option<Rule> {
    let line = trim_unescaped_trailing_spaces(line);
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (negated, body) = match line.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    let (dir_only, body) = match body.strip_suffix('/') {
        Some(rest) => (true, rest),
        None => (false, body),
    };
    if body.is_empty() {
        return None;
    }
    let anchored = body.contains('/');
    let body = body.strip_prefix('/').unwrap_or(body);
    let mut pattern = String::from("^");
    if !anchored {
        pattern.push_str("(?:.*/)?");
    }
    push_glob(body, &mut pattern);
    pattern.push('$');
    let pattern = Regex::new(&pattern).ok()?;
    Some(Rule {
        pattern,
        negated,
        dir_only,
    })
}

fn trim_unescaped_trailing_spaces(line: &str) -> &str {
    let trimmed = line.trim_end_matches([' ', '\t', '\r']);
    if trimmed.ends_with('\\') && trimmed.len() < line.len() {
        // `foo\ ` keeps its escaped space.
        &line[..trimmed.len() + 1]
    } else {
        trimmed
    }
}

fn push_glob(glob: &str, out: &mut String) {
    let chars: Vec<char> = glob.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            let segment_start = i == 0 || chars[i - 1] == '/';
            match chars.get(i + 2) {
                Some('/') if segment_start => {
                    out.push_str("(?:.*/)?");
                    i += 3;
                }
                None if segment_start => {
                    out.push_str(".*");
                    i += 2;
                }
                _ => {
                    out.push_str("[^/]*");
                    i += 2;
                }
            }
            continue;
        }
        match c {
            '*' => out.push_str("[^/]*"),
            '?' => out.push_str("[^/]"),
            '[' => match class(&chars, i) {
                Some((class, end)) => {
                    out.push_str(&class);
                    i = end;
                }
                None => out.push_str(r"\["),
            },
            '\\' if i + 1 < chars.len() => {
                i += 1;
                out.push_str(&regex::escape(&chars[i].to_string()));
            }
            c => out.push_str(&regex::escape(&c.to_string())),
        }
        i += 1;
    }
}

/// The regex class for the glob class starting at `start`, and the index
/// of its closing `]`; `None` when unclosed.
fn class(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut class = String::from("[");
    let mut i = start + 1;
    if matches!(chars.get(i), Some('!' | '^')) {
        // A negated class still never matches a separator.
        class.push_str("^/");
        i += 1;
    }
    let first = i;
    while let Some(&c) = chars.get(i) {
        match c {
            ']' if i > first => {
                class.push(']');
                return Some((class, i));
            }
            '/' => return None,
            '-' => class.push('-'),
            c if c.is_alphanumeric() => class.push(c),
            c => {
                class.push('\\');
                class.push(c);
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(files: &[(&str, &str)]) -> Vec<Arc<Rules>> {
        files
            .iter()
            .map(|(base, text)| Arc::new(Rules::parse(base, text)))
            .collect()
    }

    #[test]
    fn basenames_anchors_and_directories() {
        let rules = stack(&[("", "# comment\n*.log\n/dist\nbuild/\ndocs/*.md\n")]);
        assert!(ignored(&rules, "a/b/x.log", false));
        assert!(ignored(&rules, "dist", true));
        assert!(!ignored(&rules, "web/dist", true));
        assert!(ignored(&rules, "web/build", true));
        assert!(!ignored(&rules, "web/build", false));
        assert!(ignored(&rules, "docs/a.md", false));
        assert!(!ignored(&rules, "docs/sub/a.md", false));
        assert!(!ignored(&rules, "README.md", false));
    }

    #[test]
    fn double_stars_classes_and_escapes() {
        let rules = stack(&[("", "**/gen\nlogs/**\na/**/z\n[!a-c]x\n\\#hash\nsp\\ \n")]);
        assert!(ignored(&rules, "gen", true) && ignored(&rules, "x/y/gen", true));
        assert!(ignored(&rules, "logs/a/b", false));
        assert!(!ignored(&rules, "logs", true));
        assert!(ignored(&rules, "a/z", true) && ignored(&rules, "a/q/r/z", true));
        assert!(ignored(&rules, "dx", false));
        assert!(!ignored(&rules, "bx", false));
        let separator = stack(&[("", "foo[!a]bar\n")]);
        assert!(ignored(&separator, "fooxbar", false));
        assert!(!ignored(&separator, "foo/bar", false));
        assert!(ignored(&rules, "#hash", false));
        assert!(ignored(&rules, "sp ", false));
    }

    #[test]
    fn negation_and_deeper_files_win() {
        let rules = stack(&[
            ("", "vendor/*\n!vendor/keep\n"),
            ("web", "legacy\n!/legacy/app\n"),
        ]);
        assert!(ignored(&rules, "vendor/drop", true));
        assert!(!ignored(&rules, "vendor/keep", true));
        assert!(ignored(&rules, "web/legacy", true));
        assert!(!ignored(&rules, "legacy", true));
        let nested = stack(&[("", "*.json\n"), ("web", "!package.json\n")]);
        assert!(!ignored(&nested, "web/package.json", false));
        assert!(ignored(&nested, "package.json", false));
    }
}
