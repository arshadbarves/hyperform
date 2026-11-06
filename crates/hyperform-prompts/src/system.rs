//! Main system prompt layers.

/// Main agent: identity, communication, task method, tool policy, safety,
/// and verification rules.
pub const MAIN: &str = include_str!("../prompts/system/main.md");

/// Appended to every subagent's own prompt: reporting to the parent, no
/// contact with the user, scope, safety, and worktree isolation.
pub const SUBAGENT: &str = include_str!("../prompts/system/subagent.md");

/// Environment note of an agent isolated in a git worktree. Template:
/// `{{path}}`, `{{branch}}`, `{{base}}` (short sha) and `{{project}}`.
pub const WORKTREE: &str = include_str!("../prompts/system/worktree.md");

/// The repository map section. Template: `{{map}}` holds the outline.
pub const REPO_MAP: &str = include_str!("../prompts/system/repo-map.md");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_are_not_empty() {
        for prompt in [MAIN, SUBAGENT, WORKTREE, REPO_MAP] {
            assert!(!prompt.trim().is_empty());
        }
    }

    #[test]
    fn templates_name_their_placeholders() {
        for key in ["{{path}}", "{{branch}}", "{{base}}", "{{project}}"] {
            assert!(WORKTREE.contains(key), "worktree note lacks {key}");
        }
        assert_eq!(REPO_MAP.matches("{{map}}").count(), 1);
    }
}
