//! `[[decisions.rules]]`: questions the user puts to the decision model at
//! `PreToolUse` or `UserPromptSubmit` (`decisions_custom_rules`). A rule's
//! answer can only ask the user, post a notice or remind the agent; it can
//! never allow anything. Rules from a project apply only once it is trusted.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The events a rule may run at, named as hook events.
pub const RULE_EVENTS: [&str; 2] = ["PreToolUse", "UserPromptSubmit"];
/// What a matching answer does.
pub const RULE_ACTIONS: [&str; 3] = ["ask", "notice", "remind"];
/// More options than this and the decision model degrades.
pub const RULE_MAX_OPTIONS: usize = 20;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct DecisionRule {
    /// Shown in approvals and notices; the question when unset.
    pub name: Option<String>,
    /// `PreToolUse` or `UserPromptSubmit`.
    pub event: String,
    /// `PreToolUse` only: a regex over tool names, as in hooks; unset
    /// matches every tool.
    pub matcher: Option<String>,
    /// What to ask about the tool call or the message.
    pub question: String,
    /// The possible answers; empty means yes or no.
    pub options: Vec<String>,
    /// The answers that trigger `action`; empty means `yes`, or the first
    /// option.
    pub when: Vec<String>,
    /// `ask` (approve the tool call first, or have the agent ask you
    /// before it changes anything), `notice`, or `remind` (a note to the
    /// agent).
    pub action: String,
}

impl DecisionRule {
    /// The answers that trigger the action.
    pub fn triggers(&self) -> Vec<String> {
        if !self.when.is_empty() {
            return self.when.clone();
        }
        let first = self.options.first().map_or("yes", String::as_str);
        vec![first.to_string()]
    }

    /// The name shown to people.
    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.question)
    }
}

/// Trims every field and drops rules that cannot run, warning about each.
pub(super) fn normalize_rules(rules: &mut Vec<DecisionRule>, w: &mut Vec<String>) {
    let mut position = 0;
    rules.retain_mut(|rule| {
        position += 1;
        trim(rule);
        match problem(rule) {
            None => true,
            Some(problem) => {
                w.push(format!(
                    "decisions.rules #{position}: {problem}; rule skipped"
                ));
                false
            }
        }
    });
}

fn trim(rule: &mut DecisionRule) {
    for value in [&mut rule.name, &mut rule.matcher] {
        *value = value
            .take()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
    }
    for text in [&mut rule.event, &mut rule.question, &mut rule.action] {
        *text = text.trim().to_string();
    }
    rule.action = rule.action.to_ascii_lowercase();
    for list in [&mut rule.options, &mut rule.when] {
        *list = list
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect();
    }
}

fn problem(rule: &DecisionRule) -> Option<String> {
    if !RULE_EVENTS.contains(&rule.event.as_str()) {
        return Some(format!(
            "event `{}` is not PreToolUse or UserPromptSubmit",
            rule.event
        ));
    }
    if rule.question.is_empty() {
        return Some("the question is empty".into());
    }
    if !RULE_ACTIONS.contains(&rule.action.as_str()) {
        return Some(format!(
            "action `{}` is not ask, notice or remind",
            rule.action
        ));
    }
    let options = &rule.options;
    if options.len() == 1 || options.len() > RULE_MAX_OPTIONS {
        return Some(format!("options need 2 to {RULE_MAX_OPTIONS} answers"));
    }
    let mut unique = options.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != options.len() {
        return Some("an option is repeated".into());
    }
    let answers = if options.is_empty() {
        vec!["yes".to_string(), "no".to_string()]
    } else {
        options.clone()
    };
    if let Some(unknown) = rule.when.iter().find(|answer| !answers.contains(answer)) {
        return Some(format!("`when` names `{unknown}`, which is not an option"));
    }
    let matcher = rule.matcher.as_deref().filter(|pattern| *pattern != "*");
    if let Some(pattern) = matcher {
        if rule.event != "PreToolUse" {
            return Some("only PreToolUse rules take a matcher".into());
        }
        if regex::Regex::new(&format!("^(?:{pattern})$")).is_err() {
            return Some(format!("matcher `{pattern}` is not a valid regex"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(toml: &str) -> DecisionRule {
        toml::from_str(toml).unwrap()
    }

    #[test]
    fn valid_rules_are_trimmed_and_kept() {
        let mut rules = vec![
            rule(
                "event = \"PreToolUse\"\nmatcher = \" Bash \"\nquestion = \" Prod? \"\naction = \"Ask\"\n",
            ),
            rule(
                "event = \"UserPromptSubmit\"\nquestion = \"Area?\"\noptions = [\"ui\", \"db\", \"\"]\n\
                 when = [\"db\"]\naction = \"remind\"\nname = \"area\"\n",
            ),
        ];
        let mut w = Vec::new();
        normalize_rules(&mut rules, &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(rules[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(
            (rules[0].question.as_str(), rules[0].action.as_str()),
            ("Prod?", "ask")
        );
        assert_eq!(rules[0].triggers(), ["yes"]);
        assert_eq!(rules[0].label(), "Prod?");
        assert_eq!(rules[1].options, ["ui", "db"]);
        assert_eq!(
            (rules[1].triggers(), rules[1].label()),
            (vec!["db".to_string()], "area")
        );
    }

    #[test]
    fn rules_that_cannot_run_are_skipped_with_a_warning() {
        let mut rules: Vec<DecisionRule> = [
            ("Stop", "Q?", "notice", ""),
            ("PreToolUse", "Q?", "allow", ""),
            ("PreToolUse", "Q?", "notice", "options = [\"only\"]"),
            ("PreToolUse", "Q?", "notice", "options = [\"a\", \"a\"]"),
            ("PreToolUse", "Q?", "notice", "when = [\"maybe\"]"),
            ("PreToolUse", "Q?", "notice", "matcher = \"(\""),
            ("UserPromptSubmit", "Q?", "notice", "matcher = \"Bash\""),
            ("PreToolUse", " ", "notice", ""),
        ]
        .iter()
        .map(|(event, question, action, extra)| {
            rule(&format!(
                "event = \"{event}\"\nquestion = \"{question}\"\naction = \"{action}\"\n{extra}\n"
            ))
        })
        .collect();
        let mut w = Vec::new();
        normalize_rules(&mut rules, &mut w);
        assert!(rules.is_empty(), "{rules:?}");
        assert_eq!(w.len(), 8, "{w:?}");
        assert!(w[0].starts_with("decisions.rules #1: event `Stop`"));
    }
}
