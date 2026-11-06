//! Prompt text a model provider's adapter adds to requests.

/// Description of the placeholder tool GitHub Copilot needs when a history
/// with tool calls is sent without tools.
pub const COPILOT_NOOP_TOOL: &str = include_str!("../prompts/providers/copilot-noop-tool.md");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_are_not_empty() {
        assert!(!COPILOT_NOOP_TOOL.trim().is_empty());
    }
}
