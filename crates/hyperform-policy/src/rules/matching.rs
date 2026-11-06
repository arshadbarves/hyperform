//! Which actions a rule governs. Shell and file actions are matched per
//! command and per path by the decision code; these helpers expose the
//! relevant part of each rule.

use super::command::CommandPattern;
use super::domain::url_host;
use super::path_pattern::PathPattern;
use super::rule::{FileScope, McpTool, Rule, Target};
use crate::action::Action;

impl Rule {
    /// A bare rule naming exactly this tool, for tools whose action carries
    /// nothing else to match (`TodoWrite`, `Verify`, `Skill`).
    pub(crate) fn names_tool(&self, tool: &str) -> bool {
        match &self.target {
            Target::Tool(name) => name == tool,
            Target::Files {
                scope: FileScope::Tool(name),
                pattern: None,
            } => name == tool,
            Target::Skill(None) => tool == "Skill",
            _ => false,
        }
    }

    /// For a `Read` or `Write` action: `None` when the rule does not govern
    /// it, `Some(None)` for a rule without a path pattern.
    pub(crate) fn path_scope(&self, tool: &str, action: &Action) -> Option<Option<&PathPattern>> {
        let reads = matches!(action, Action::Read { .. });
        let writes = matches!(action, Action::Write { .. });
        match &self.target {
            Target::Files { scope, pattern } => {
                let governs = match scope {
                    FileScope::Read => reads,
                    FileScope::Edit => writes,
                    FileScope::Tool(name) => name == tool && (reads || writes),
                };
                governs.then_some(pattern.as_ref())
            }
            Target::Tool(name) if name == tool && (reads || writes) => Some(None),
            _ => None,
        }
    }

    /// `Some(None)` for a bare `Bash` rule, `Some(Some(..))` for a patterned one.
    pub(crate) fn command_pattern(&self) -> Option<Option<&CommandPattern>> {
        match &self.target {
            Target::Bash(pattern) => Some(pattern.as_ref()),
            _ => None,
        }
    }

    /// The path pattern of a `Read(..)` (or, with `writes`, `Edit(..)`) group
    /// rule, which also applies to files named by shell commands.
    pub(crate) fn operand_pattern(&self, writes: bool) -> Option<&PathPattern> {
        match &self.target {
            Target::Files {
                scope: FileScope::Read,
                pattern: Some(pattern),
            } if !writes => Some(pattern),
            Target::Files {
                scope: FileScope::Edit,
                pattern: Some(pattern),
            } if writes => Some(pattern),
            _ => None,
        }
    }

    /// Match for actions without paths or commands. Names compare
    /// case-insensitively so a deny rule is not dodged by capitalization.
    pub(crate) fn matches_call(&self, tool: &str, action: &Action) -> bool {
        if self.names_tool(tool) {
            return true;
        }
        match (&self.target, action) {
            (Target::WebFetch(None), Action::Fetch { .. })
            | (Target::WebSearch, Action::Search)
            | (Target::Agent(None), Action::Agent { .. })
            | (Target::Skill(None), Action::Skill { .. }) => true,
            (Target::WebFetch(Some(domain)), Action::Fetch { url }) => {
                url_host(url).is_some_and(|host| domain.matches_host(&host))
            }
            (Target::Agent(Some(name)), Action::Agent { agent_type }) => {
                name.eq_ignore_ascii_case(agent_type)
            }
            (Target::Skill(Some(name)), Action::Skill { name: skill }) => {
                name.eq_ignore_ascii_case(skill)
            }
            (
                Target::Mcp {
                    server,
                    tool: pattern,
                },
                _,
            ) => mcp_identity(tool, action).is_some_and(|(called_server, called_tool)| {
                server.eq_ignore_ascii_case(called_server)
                    && match pattern {
                        McpTool::All { .. } => true,
                        McpTool::Named(name) => name.eq_ignore_ascii_case(called_tool),
                    }
            }),
            _ => false,
        }
    }
}

/// `(server, tool)` of an MCP call, from the action or else the
/// `mcp__server__tool` tool name.
fn mcp_identity<'a>(tool: &'a str, action: &'a Action) -> Option<(&'a str, &'a str)> {
    match action {
        Action::Mcp {
            server,
            tool: called,
            ..
        } => Some((server.as_str(), called.as_str())),
        _ => tool.strip_prefix("mcp__")?.split_once("__"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(text: &str) -> Rule {
        Rule::parse(text).unwrap()
    }

    fn mcp(server: &str, tool: &str) -> Action {
        Action::Mcp {
            server: server.into(),
            tool: tool.into(),
            read_only: false,
        }
    }

    #[test]
    fn mcp_rules_match_servers_tools_and_wildcards() {
        let call = mcp("github", "create_issue");
        let name = "mcp__github__create_issue";
        assert!(rule("mcp__github").matches_call(name, &call));
        assert!(rule("mcp__github__*").matches_call(name, &call));
        assert!(rule("mcp__github__create_issue").matches_call(name, &call));
        assert!(rule("mcp__GitHub__Create_Issue").matches_call(name, &call));
        assert!(!rule("mcp__github__get_issue").matches_call(name, &call));
        assert!(!rule("mcp__gitlab").matches_call(name, &call));
        let misfiled = Action::Other { read_only: false };
        assert!(rule("mcp__github").matches_call(name, &misfiled));
    }

    #[test]
    fn web_agent_and_skill_rules() {
        let fetch = Action::Fetch {
            url: "https://docs.example.com/x".into(),
        };
        assert!(rule("WebFetch").matches_call("WebFetch", &fetch));
        assert!(rule("WebFetch(domain:example.com)").matches_call("WebFetch", &fetch));
        assert!(!rule("WebFetch(domain:other.com)").matches_call("WebFetch", &fetch));
        assert!(rule("WebSearch").matches_call("WebSearch", &Action::Search));
        let agent = Action::Agent {
            agent_type: "Explore".into(),
        };
        assert!(rule("Agent").matches_call("Agent", &agent));
        assert!(rule("Task(explore)").matches_call("Agent", &agent));
        assert!(!rule("Agent(plan)").matches_call("Agent", &agent));
        let skill = Action::Skill { name: "pdf".into() };
        assert!(rule("Skill(pdf)").matches_call("Skill", &skill));
        assert!(!rule("Skill(docx)").matches_call("Skill", &skill));
        assert!(rule("Skill").matches_call("Skill", &Action::Other { read_only: true }));
    }

    #[test]
    fn bare_names_match_exactly() {
        let other = Action::Other { read_only: false };
        assert!(rule("JobKill").matches_call("JobKill", &other));
        assert!(!rule("JobKill").matches_call("jobkill", &other));
        assert!(!rule("Job").matches_call("JobKill", &other));
    }

    #[test]
    fn file_rules_scope_by_action_and_tool() {
        let read = Action::Read { paths: vec![] };
        let write = Action::Write { paths: vec![] };
        assert!(rule("Read").path_scope("Grep", &read).is_some());
        assert!(rule("Read").path_scope("LSP", &write).is_none());
        assert!(
            rule("Edit")
                .path_scope("ApplyAgentChanges", &write)
                .is_some()
        );
        assert!(rule("Edit").path_scope("Read", &read).is_none());
        assert!(
            rule("Grep(src/**)")
                .path_scope("Grep", &read)
                .is_some_and(|p| p.is_some())
        );
        assert!(rule("Grep").path_scope("Glob", &read).is_none());
        assert!(
            rule("NotebookRead")
                .path_scope("NotebookRead", &read)
                .is_some()
        );
        assert!(rule("Bash").path_scope("Read", &read).is_none());
        assert!(rule("Read(x)").operand_pattern(false).is_some());
        assert!(rule("Read(x)").operand_pattern(true).is_none());
        assert!(rule("Edit(x)").operand_pattern(true).is_some());
        assert!(rule("Grep(x)").operand_pattern(false).is_none());
    }
}
