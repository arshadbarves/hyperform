//! Built-in prompt commands (markdown with YAML frontmatter).

/// `/init`: create or improve the project's `AGENTS.md`.
pub const INIT: &str = include_str!("../prompts/commands/init.md");

/// `/review`: review pending changes or a named target.
pub const REVIEW: &str = include_str!("../prompts/commands/review.md");

/// `/security-review`: security-focused review of pending changes.
pub const SECURITY_REVIEW: &str = include_str!("../prompts/commands/security-review.md");

/// `/commit`: commit the current changes; never pushes.
pub const COMMIT: &str = include_str!("../prompts/commands/commit.md");

/// `/replan`: the turn the engine starts after reconciling the attached work
/// plan found problems. Run only by the engine (with `work_plans` on), so
/// it is not part of [`BUILTIN`].
pub const REPLAN: &str = include_str!("../prompts/commands/replan.md");

/// Every built-in command as `(name, definition)`. The name is the slash
/// command and the file stem under `prompts/commands/`.
pub const BUILTIN: &[(&str, &str)] = &[
    ("init", INIT),
    ("review", REVIEW),
    ("security-review", SECURITY_REVIEW),
    ("commit", COMMIT),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_not_empty() {
        for prompt in [INIT, REVIEW, SECURITY_REVIEW, COMMIT, REPLAN] {
            assert!(!prompt.trim().is_empty());
        }
        assert!(REPLAN.starts_with("---\n") && REPLAN.contains("$ARGUMENTS"));
        assert!(BUILTIN.iter().all(|(name, _)| *name != "replan"));
    }

    #[test]
    fn builtin_names_are_unique() {
        for (index, (name, _)) in BUILTIN.iter().enumerate() {
            assert!(
                BUILTIN[index + 1..].iter().all(|(other, _)| other != name),
                "duplicate command `{name}`"
            );
        }
    }
}
