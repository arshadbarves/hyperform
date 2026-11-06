//! Actions without paths or commands: fetches, searches, subagents, MCP
//! calls, skills, and other tools.

use crate::action::Action;
use crate::engine::{Decision, Policy};
use crate::rules::{Rule, url_host};

pub(super) fn decide(policy: &Policy, tool: &str, action: &Action) -> Decision {
    if let Some(rule) = policy
        .allow_rules()
        .find(|rule| rule.matches_call(tool, action))
    {
        return Decision::allow(format!("allowed by rule {rule}"));
    }
    match action {
        Action::Fetch { url } => match url_host(url) {
            Some(host) => Decision::ask(
                format!("fetches {host}"),
                valid(format!("WebFetch(domain:{host})")),
                true,
            ),
            None => Decision::ask("fetches a URL without a host", None, true),
        },
        Action::Search => Decision::ask("searches the web", Some("WebSearch".into()), true),
        Action::Agent { .. } => Decision::allow("subagents are gated per tool call"),
        Action::Skill { .. } => Decision::allow("skills only load instructions"),
        Action::Mcp { server, tool, .. } => Decision::ask(
            format!("calls MCP tool {tool} on {server}"),
            valid(format!("mcp__{server}__{tool}")),
            true,
        ),
        Action::Other { read_only: true } => Decision::allow("read-only tool"),
        Action::Other { read_only: false } => Decision::ask(
            format!("{tool} changes state"),
            valid(tool.to_string()),
            true,
        ),
        Action::Read { .. } | Action::Write { .. } | Action::Execute { .. } => {
            Decision::ask(format!("{tool} needs approval"), None, false)
        }
    }
}

/// The rule in canonical form when it parses.
fn valid(text: String) -> Option<String> {
    Rule::parse(&text).ok().map(|rule| rule.to_string())
}
