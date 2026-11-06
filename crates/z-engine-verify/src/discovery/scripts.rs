//! Which package scripts (and Deno tasks) are checks, and which script
//! bodies cannot be one: placeholders, watchers, and commands that rewrite
//! files. A check must finish on its own and leave the workspace as it was.

use z_engine_protocol::CheckKind;

/// Script names taken as checks, in the order their checks are listed.
pub(crate) const CANDIDATES: &[(&str, CheckKind)] = &[
    ("test", CheckKind::Test),
    ("build", CheckKind::Build),
    ("typecheck", CheckKind::Typecheck),
    ("type-check", CheckKind::Typecheck),
    ("tsc", CheckKind::Typecheck),
    ("check", CheckKind::Typecheck),
    ("lint", CheckKind::Lint),
    ("format:check", CheckKind::Format),
    ("fmt:check", CheckKind::Format),
    ("prettier:check", CheckKind::Format),
    ("format", CheckKind::Format),
    ("fmt", CheckKind::Format),
    ("prettier", CheckKind::Format),
];

/// Flags that make a formatter report instead of rewrite.
const REPORT_FLAGS: &[&str] = &[
    "--check",
    "-c",
    "--list-different",
    "-l",
    "--dry-run",
    "--diff",
    "check",
];

/// Why `body` of script `name` is not usable as a `kind` check, if it is not.
pub(crate) fn unusable(name: &str, kind: CheckKind, body: &str) -> Option<&'static str> {
    let body = body.trim();
    if body.is_empty() {
        return Some("it is empty");
    }
    if body.contains("no test specified") {
        return Some("it is the npm placeholder");
    }
    let words: Vec<&str> = body.split_whitespace().collect();
    let watches = words.iter().any(|w| {
        *w == "watch" || *w == "-w" || (w.starts_with("--watch") && !w.ends_with("=false"))
    });
    if watches {
        return Some("it watches for changes and never finishes");
    }
    if words
        .iter()
        .any(|w| w.starts_with("--write") || w.starts_with("--fix"))
    {
        return Some("it rewrites files");
    }
    let reports = name.ends_with("check") || words.iter().any(|w| REPORT_FLAGS.contains(w));
    if kind == CheckKind::Format && !reports {
        return Some("it may rewrite files; add a `format:check` script");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usable_scripts() {
        for (name, kind, body) in [
            ("test", CheckKind::Test, "vitest"),
            ("test", CheckKind::Test, "jest --watchAll=false"),
            (
                "check",
                CheckKind::Typecheck,
                "svelte-kit sync && svelte-check",
            ),
            ("lint", CheckKind::Lint, "eslint ."),
            ("format", CheckKind::Format, "prettier --check ."),
            ("format:check", CheckKind::Format, "biome format ."),
            ("fmt", CheckKind::Format, "deno fmt --check"),
        ] {
            assert_eq!(unusable(name, kind, body), None, "{name}: {body}");
        }
    }

    #[test]
    fn unusable_scripts_say_why() {
        for (name, kind, body, reason) in [
            ("test", CheckKind::Test, "  ", "empty"),
            (
                "test",
                CheckKind::Test,
                "echo \"Error: no test specified\" && exit 1",
                "placeholder",
            ),
            ("test", CheckKind::Test, "jest --watch", "watches"),
            ("build", CheckKind::Build, "tsc -w", "watches"),
            ("test", CheckKind::Test, "vitest watch", "watches"),
            ("lint", CheckKind::Lint, "eslint --fix .", "rewrites"),
            (
                "format",
                CheckKind::Format,
                "prettier --write .",
                "rewrites",
            ),
            ("fmt", CheckKind::Format, "deno fmt", "format:check"),
        ] {
            let why = unusable(name, kind, body).unwrap_or_default();
            assert!(why.contains(reason), "{name}: {body} -> {why}");
        }
    }
}
