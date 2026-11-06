//! The rule grammar.

use super::command::CommandPattern;
use super::domain::DomainPattern;
use super::path_pattern::{PathPattern, PatternError};
use super::rule::{FileScope, McpTool, Rule, Target};
use crate::error::PolicyError;

/// File tools that take path specifiers besides the `Read` and `Edit` groups.
const FILE_TOOLS: &[&str] = &[
    "Write",
    "MultiEdit",
    "NotebookEdit",
    "ApplyAgentChanges",
    "Glob",
    "Grep",
    "LSP",
    "NotebookRead",
];

impl Rule {
    /// Parses `Tool` or `Tool(specifier)`:
    ///
    /// - `Bash` (every command), `Bash(git status)` (exact), `Bash(npm test:*)`
    ///   (word prefix); v1's `Bash(cargo test*)` is read as a prefix.
    /// - `Read(src/**)`, `Edit(docs/**)`, `Grep(..)`, ...: gitignore-style
    ///   paths relative to the project root, `~/..` for home, `//abs/..` (or
    ///   `/abs/..`) for absolute paths. `Read` covers every read action and
    ///   `Edit` every write action.
    /// - `WebFetch(domain:example.com)` (the host and its subdomains;
    ///   `*.example.com` is accepted), `WebSearch`.
    /// - `Agent(explore)` (alias `Task(explore)`), `Skill(name)`.
    /// - `mcp__server`, `mcp__server__*`, `mcp__server__tool`.
    /// - Any other bare tool name (`TodoWrite`) matches that tool exactly.
    pub fn parse(text: &str) -> Result<Self, PolicyError> {
        let text = text.trim();
        let (tool, specifier) = split(text)?;
        let target = match tool.strip_prefix("mcp__") {
            Some(_) if specifier.is_some() => {
                return Err(PolicyError::invalid(
                    text,
                    "MCP rules do not take a specifier",
                ));
            }
            Some(rest) => mcp(text, rest)?,
            None => target(text, tool, specifier)?,
        };
        Ok(Self { target })
    }
}

fn split(text: &str) -> Result<(&str, Option<&str>), PolicyError> {
    let (tool, specifier) = match text.split_once('(') {
        Some((tool, rest)) => {
            let inner = rest
                .strip_suffix(')')
                .ok_or_else(|| PolicyError::invalid(text, "missing closing parenthesis"))?
                .trim();
            if inner.is_empty() {
                return Err(PolicyError::invalid(text, "empty specifier"));
            }
            (tool.trim_end(), Some(inner))
        }
        None => (text, None),
    };
    if tool.is_empty() {
        return Err(PolicyError::invalid(text, "missing tool name"));
    }
    let valid = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '*');
    if !tool.chars().all(valid) {
        return Err(PolicyError::invalid(
            text,
            "tool names use letters, digits, `_`, `-` and `.`",
        ));
    }
    Ok((tool, specifier))
}

fn target(text: &str, tool: &str, specifier: Option<&str>) -> Result<Target, PolicyError> {
    let invalid = |message: String| PolicyError::invalid(text, message);
    if tool.contains('*') {
        return Err(invalid(
            "wildcards are only supported as `mcp__server__*`".into(),
        ));
    }
    Ok(match (tool, specifier) {
        ("Bash", None) => Target::Bash(None),
        ("Bash", Some(spec)) => Target::Bash(CommandPattern::parse(spec).map_err(invalid)?),
        ("Read", spec) => files(text, FileScope::Read, spec)?,
        ("Edit", spec) => files(text, FileScope::Edit, spec)?,
        (name, spec) if FILE_TOOLS.contains(&name) => {
            files(text, FileScope::Tool(name.to_string()), spec)?
        }
        ("WebFetch", spec) => Target::WebFetch(
            spec.map(DomainPattern::parse)
                .transpose()
                .map_err(invalid)?,
        ),
        ("WebSearch", None) => Target::WebSearch,
        ("Agent" | "Task", spec) => Target::Agent(spec.map(str::to_string)),
        ("Skill", spec) => Target::Skill(spec.map(str::to_string)),
        (name, None) => Target::Tool(name.to_string()),
        (name, Some(_)) => return Err(invalid(format!("{name} rules do not take a specifier"))),
    })
}

fn files(text: &str, scope: FileScope, specifier: Option<&str>) -> Result<Target, PolicyError> {
    let pattern = specifier
        .map(PathPattern::parse)
        .transpose()
        .map_err(|error| match error {
            PatternError::Invalid(message) => PolicyError::invalid(text, message),
            PatternError::Glob(source) => PolicyError::InvalidPattern {
                rule: text.to_string(),
                source,
            },
        })?;
    Ok(Target::Files { scope, pattern })
}

fn mcp(text: &str, rest: &str) -> Result<Target, PolicyError> {
    let invalid = |message: &str| PolicyError::invalid(text, message);
    let (server, tool) = match rest.split_once("__") {
        None => (rest, McpTool::All { star: false }),
        Some((server, "*")) => (server, McpTool::All { star: true }),
        Some((_, "")) => return Err(invalid("missing MCP tool name")),
        Some((_, tool)) if tool.contains('*') => {
            return Err(invalid("only `mcp__server__*` wildcards are supported"));
        }
        Some((server, tool)) => (server, McpTool::Named(tool.to_string())),
    };
    if server.is_empty() || server.contains('*') {
        return Err(invalid("missing MCP server name"));
    }
    Ok(Target::Mcp {
        server: server.to_string(),
        tool,
    })
}

#[cfg(test)]
#[path = "grammar_tests.rs"]
mod tests;
