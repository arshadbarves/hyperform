//! Built-in agent definitions (markdown with YAML frontmatter).

/// Full tool access for self-contained multi-step work.
pub const GENERAL: &str = include_str!("../prompts/agents/general.md");

/// Fast, read-only codebase search and research.
pub const EXPLORE: &str = include_str!("../prompts/agents/explore.md");

/// Read-only architect that returns an implementation plan.
pub const PLAN: &str = include_str!("../prompts/agents/plan.md");

/// Read-only reviewer of pending changes.
pub const REVIEW: &str = include_str!("../prompts/agents/review.md");

/// Runs the project's checks and reports evidence without fixing code.
pub const VERIFY: &str = include_str!("../prompts/agents/verify.md");

/// `work_plans`: read-only reviewer of a work plan; answers with JSON
/// findings in one model call.
pub const AUDIT: &str = include_str!("../prompts/agents/audit.md");

/// Every built-in agent as `(name, definition)`. The name equals the
/// frontmatter `name` and the file stem under `prompts/agents/`.
pub const BUILTIN: &[(&str, &str)] = &[
    ("general", GENERAL),
    ("explore", EXPLORE),
    ("plan", PLAN),
    ("review", REVIEW),
    ("verify", VERIFY),
    ("audit", AUDIT),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_not_empty() {
        for prompt in [GENERAL, EXPLORE, PLAN, REVIEW, VERIFY, AUDIT] {
            assert!(!prompt.trim().is_empty());
        }
    }

    #[test]
    fn builtin_names_are_unique() {
        for (index, (name, _)) in BUILTIN.iter().enumerate() {
            assert!(
                BUILTIN[index + 1..].iter().all(|(other, _)| other != name),
                "duplicate agent `{name}`"
            );
        }
    }

    #[test]
    fn the_audit_agent_answers_with_json_findings() {
        assert!(AUDIT.contains("name: audit"));
        assert!(AUDIT.contains("\"findings\""));
    }

    #[test]
    fn the_audit_agent_works_with_or_without_its_tools() {
        assert!(AUDIT.contains("When you have tools"));
        assert!(AUDIT.contains("when the plan is all you are given"));
    }
}
