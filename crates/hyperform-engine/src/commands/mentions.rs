//! Tokens a command body or prompt refers to: `@agent-<name>` mentions,
//! `@path` inclusions, and `` !`cmd` `` pre-executions.

/// `@` tokens that start a word, with trailing punctuation removed.
fn at_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(char::is_whitespace).filter_map(|word| {
        let token = word.strip_prefix('@')?;
        let token = token.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '"', '\'']);
        (!token.is_empty()).then_some(token)
    })
}

/// Known agent names mentioned as `@agent-<name>`, in order, once each.
pub(crate) fn agent_mentions(text: &str, known: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for token in at_tokens(text) {
        let Some(name) = token.strip_prefix("agent-") else {
            continue;
        };
        if known.contains(&name) && !found.iter().any(|seen| seen == name) {
            found.push(name.to_string());
        }
    }
    found
}

/// `@path` tokens (agent mentions excluded), once each.
pub(crate) fn file_mentions(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for token in at_tokens(text) {
        if token.starts_with("agent-") || found.iter().any(|seen| seen == token) {
            continue;
        }
        found.push(token.to_string());
    }
    found
}

/// `` !`cmd` `` spans as `(whole span, command)`.
pub(crate) fn inline_commands(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("!`") {
        let after = &rest[start + 2..];
        let Some(end) = after.find('`') else { break };
        let command = &after[..end];
        if !command.trim().is_empty() {
            found.push((format!("!`{command}`"), command.trim().to_string()));
        }
        rest = &after[end + 1..];
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_mentions_need_a_known_name() {
        let text = "ask @agent-explore and (@agent-explore) or @agent-nobody, mail a@agent-plan";
        assert_eq!(agent_mentions(text, &["explore", "plan"]), ["explore"]);
    }

    #[test]
    fn file_mentions_skip_agents_and_trim_punctuation() {
        let text = "See @src/lib.rs, then @README.md. Ask @agent-plan; @src/lib.rs again";
        assert_eq!(file_mentions(text), ["src/lib.rs", "README.md"]);
    }

    #[test]
    fn inline_commands_are_backtick_spans() {
        let text = "Status: !`git status --short` and !`` nothing, then !`echo hi`";
        let found = inline_commands(text);
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0],
            ("!`git status --short`".into(), "git status --short".into())
        );
        assert_eq!(found[1].1, "echo hi");
    }
}
