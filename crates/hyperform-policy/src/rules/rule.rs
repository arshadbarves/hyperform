//! Rule values and their canonical text form.

use std::fmt;

use super::command::CommandPattern;
use super::domain::DomainPattern;
use super::path_pattern::PathPattern;

/// The list a rule belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleKind {
    Allow,
    Ask,
    Deny,
}

/// A parsed permission rule; see [`Rule::parse`] for the grammar. `Display`
/// prints the canonical form, which parses back to an equal rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub(crate) target: Target,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// `Bash`, `Bash(git status)`, `Bash(npm test:*)`: every shell command,
    /// whichever tool runs it.
    Bash(Option<CommandPattern>),
    /// The `Read(..)` / `Edit(..)` groups or one file tool such as `Grep(..)`.
    Files {
        scope: FileScope,
        pattern: Option<PathPattern>,
    },
    WebFetch(Option<DomainPattern>),
    WebSearch,
    /// `Agent(type)`; `Task(type)` is accepted as an alias.
    Agent(Option<String>),
    Skill(Option<String>),
    /// `mcp__server`, `mcp__server__*`, `mcp__server__tool`.
    Mcp {
        server: String,
        tool: McpTool,
    },
    /// Any other bare tool name, matched exactly.
    Tool(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileScope {
    /// Every read action (`Read`, `Glob`, `Grep`, `LSP`, ...).
    Read,
    /// Every write action (`Write`, `Edit`, `MultiEdit`, `NotebookEdit`,
    /// `ApplyAgentChanges`).
    Edit,
    /// One file tool, by name.
    Tool(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum McpTool {
    /// Every tool of the server; `star` keeps the `__*` spelling.
    All {
        star: bool,
    },
    Named(String),
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.target {
            Target::Bash(pattern) => optional(f, "Bash", pattern.as_ref()),
            Target::Files { scope, pattern } => {
                let name = match scope {
                    FileScope::Read => "Read",
                    FileScope::Edit => "Edit",
                    FileScope::Tool(name) => name,
                };
                optional(f, name, pattern.as_ref())
            }
            Target::WebFetch(domain) => optional(f, "WebFetch", domain.as_ref()),
            Target::WebSearch => f.write_str("WebSearch"),
            Target::Agent(name) => optional(f, "Agent", name.as_ref()),
            Target::Skill(name) => optional(f, "Skill", name.as_ref()),
            Target::Mcp { server, tool } => match tool {
                McpTool::All { star: false } => write!(f, "mcp__{server}"),
                McpTool::All { star: true } => write!(f, "mcp__{server}__*"),
                McpTool::Named(tool) => write!(f, "mcp__{server}__{tool}"),
            },
            Target::Tool(name) => f.write_str(name),
        }
    }
}

fn optional(
    f: &mut fmt::Formatter<'_>,
    tool: &str,
    specifier: Option<&impl fmt::Display>,
) -> fmt::Result {
    match specifier {
        Some(specifier) => write!(f, "{tool}({specifier})"),
        None => f.write_str(tool),
    }
}
