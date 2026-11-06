//! The ordered set of tools an agent may call. Order is kept stable so the
//! tool list (and the provider's prompt cache) does not churn.

use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::builtin::builtin_tools;
use crate::ports::AgentCard;
use crate::tool::Tool;

#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every built-in tool; the `Agent` description lists `agent_catalog`.
    pub fn builtin(agent_catalog: Vec<AgentCard>) -> Self {
        Self {
            tools: builtin_tools(agent_catalog),
        }
    }

    /// Adds `tool`, replacing a tool of the same name in place.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        match self
            .tools
            .iter_mut()
            .find(|existing| existing.name() == tool.name())
        {
            Some(existing) => *existing = tool,
            None => self.tools.push(tool),
        }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|tool| tool.name() == name).cloned()
    }

    pub fn names(&self) -> Vec<&str> {
        self.tools.iter().map(|tool| tool.name()).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Tool>> {
        self.tools.iter()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// The tools `allow` names (all when `None`) minus those `deny` names.
    ///
    /// Entries match names exactly or, ending in `*`, by prefix
    /// (`mcp__github__*`, `*`). An allow entry written as a permission rule
    /// (`Bash(git add:*)`) counts as its tool; a deny entry with a rule
    /// scope removes nothing, because it restricts calls rather than tools.
    pub fn filtered(&self, allow: Option<&[String]>, deny: &[String]) -> Self {
        let allowed = |name: &str| {
            allow.is_none_or(|entries| {
                entries.iter().any(|entry| {
                    matches(
                        entry
                            .split_once('(')
                            .map_or(entry.as_str(), |(tool, _)| tool),
                        name,
                    )
                })
            })
        };
        let denied = |name: &str| {
            deny.iter()
                .filter(|entry| !entry.contains('('))
                .any(|entry| matches(entry, name))
        };
        Self {
            tools: self
                .tools
                .iter()
                .filter(|tool| allowed(tool.name()) && !denied(tool.name()))
                .cloned()
                .collect(),
        }
    }

    /// This registry without the named tools.
    pub fn without<I, S>(&self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names: Vec<S> = names.into_iter().collect();
        Self {
            tools: self
                .tools
                .iter()
                .filter(|tool| !names.iter().any(|name| name.as_ref() == tool.name()))
                .cloned()
                .collect(),
        }
    }

    /// `(name, description, input_schema)` for every tool, in order.
    pub fn specs(&self) -> Vec<(String, String, Value)> {
        self.tools
            .iter()
            .map(|tool| {
                (
                    tool.name().to_string(),
                    tool.description(),
                    tool.input_schema(),
                )
            })
            .collect()
    }
}

fn matches(entry: &str, name: &str) -> bool {
    let entry = entry.trim();
    match entry.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => entry == name,
    }
}

impl fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.names())
            .finish()
    }
}
