//! Lexical matching of task scopes: root-relative paths and globs (`*`
//! stays within one directory, `**` crosses them). A literal path also
//! covers everything under it. Nothing touches the filesystem.

use globset::GlobBuilder;
use hyperform_protocol::work::Task;

use crate::glob;

const META: [char; 4] = ['*', '?', '[', '{'];

pub fn is_glob(pattern: &str) -> bool {
    pattern.contains(META)
}

/// `path` falls under `pattern`.
pub fn matches(pattern: &str, path: &str) -> bool {
    let (pattern, path) = (normalize(pattern), normalize(path));
    if !is_glob(pattern) {
        let dir = pattern.trim_end_matches('/');
        return path == dir
            || path
                .strip_prefix(dir)
                .is_some_and(|rest| rest.starts_with('/'));
    }
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .is_ok_and(|glob| glob.compile_matcher().is_match(path))
}

pub fn matches_any(patterns: &[String], path: &str) -> bool {
    patterns.iter().any(|pattern| matches(pattern, path))
}

/// A write to `path` falls outside `task`'s declared files. A task that
/// declares no files has no scope to drift from, so nothing is outside.
pub fn outside(task: &Task, path: &str) -> bool {
    !task.scope.files.is_empty() && !matches_any(&task.scope.files, path)
}

/// The `patterns` that `scope` names (see `glob::names`): a scope of
/// `crates/app/**` does not name `**/Cargo.toml`, `crates/app/Cargo.toml`
/// does.
pub fn named(scope: &[String], patterns: &[String]) -> Vec<String> {
    patterns
        .iter()
        .filter(|pattern| scope.iter().any(|file| glob::names(file, pattern)))
        .cloned()
        .collect()
}

/// Patterns of `a` and `b` that can name the same file (the narrower of
/// each pair).
pub fn overlap(a: &[String], b: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for left in a {
        for right in b {
            if let Some(shared) = pair_overlap(left, right) {
                if !out.contains(&shared) {
                    out.push(shared);
                }
            }
        }
    }
    out
}

/// The directory part of a pattern's literal prefix (`src/auth` for
/// `src/auth/*.rs`), or the pattern itself when it is a literal.
pub fn base_dir(pattern: &str) -> &str {
    literal_prefix(normalize(pattern))
        .rsplit_once('/')
        .map_or("", |(dir, _)| dir)
}

fn pair_overlap(a: &str, b: &str) -> Option<String> {
    let (a, b) = (normalize(a), normalize(b));
    if !glob::intersects(a, b) {
        return None;
    }
    let narrower = match (is_glob(a), is_glob(b)) {
        (false, false) | (true, false) if matches(a, b) => b,
        (false, false) | (false, true) if matches(b, a) => a,
        (true, false) | (false, false) => a,
        (false, true) => b,
        (true, true) if literal_prefix(a).len() >= literal_prefix(b).len() => a,
        (true, true) => b,
    };
    Some(narrower.to_string())
}

fn literal_prefix(pattern: &str) -> &str {
    pattern
        .find(META)
        .map_or(pattern, |index| &pattern[..index])
}

fn normalize(path: &str) -> &str {
    path.trim().trim_start_matches("./")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn literals_cover_themselves_and_what_is_under_them() {
        assert!(matches("src/lib.rs", "src/lib.rs"));
        assert!(matches("./src/auth", "src/auth/login.rs"));
        assert!(!matches("src/auth", "src/authz.rs"));
        assert!(!matches("src/lib.rs", "src/lib.rs.bak"));
    }

    #[test]
    fn single_stars_stay_in_one_directory() {
        assert!(matches("src/*.rs", "src/lib.rs"));
        assert!(!matches("src/*.rs", "src/a/b.rs"));
        assert!(matches("src/**/*.rs", "src/a/b.rs"));
        assert!(matches("**/Cargo.toml", "crates/x/Cargo.toml"));
        assert!(!matches("[", "["), "an invalid glob matches nothing");
    }

    #[test]
    fn overlap_names_the_narrower_pattern_of_each_pair() {
        assert_eq!(
            overlap(&s(&["src/**"]), &s(&["src/lib.rs"])),
            s(&["src/lib.rs"])
        );
        assert_eq!(
            overlap(&s(&["src/a/*.rs"]), &s(&["src/b/*.rs"])),
            Vec::<String>::new()
        );
        assert!(overlap(&s(&["src/*.rs"]), &s(&["src/*.toml"])).is_empty());
        assert_eq!(
            overlap(&s(&["docs/a.md"]), &s(&["docs"])),
            s(&["docs/a.md"])
        );
        assert_eq!(
            overlap(&s(&["crates/app"]), &s(&["**/Cargo.toml"])),
            s(&["**/Cargo.toml"])
        );
        assert_eq!(
            overlap(&s(&["src/**/*.rs"]), &s(&["src/auth"])),
            s(&["src/**/*.rs"])
        );
        assert!(overlap(&s(&["a.rs"]), &s(&["b.rs"])).is_empty());
    }

    #[test]
    fn writes_outside_a_declared_scope_are_found() {
        let mut task = crate::testkit::created("a");
        assert!(!outside(&task, "anything.rs"), "no scope, no drift");
        task.scope.files = s(&["src/auth/**", "docs/auth.md"]);
        assert!(!outside(&task, "src/auth/login.rs"));
        assert!(!outside(&task, "./docs/auth.md"));
        assert!(outside(&task, "src/billing.rs"));
    }

    #[test]
    fn named_serial_paths_ignore_broad_directories() {
        let serial = s(&["**/Cargo.lock", "**/Cargo.toml", "ui/src/lib/protocol/**"]);
        assert!(named(&s(&["crates/app/**", "ui/**"]), &serial).is_empty());
        assert_eq!(
            named(&s(&["crates/app/Cargo.toml", "src/lib.rs"]), &serial),
            s(&["**/Cargo.toml"])
        );
        assert_eq!(
            named(&s(&["ui/src/lib/protocol/Task.ts"]), &serial),
            s(&["ui/src/lib/protocol/**"])
        );
    }

    #[test]
    fn base_dirs_come_from_the_literal_prefix() {
        assert_eq!(base_dir("src/auth/*.rs"), "src/auth");
        assert_eq!(base_dir("src/lib.rs"), "src");
        assert_eq!(base_dir("*.md"), "");
        assert_eq!(base_dir("Cargo.toml"), "");
    }
}
