//! The agent types a session offers: the built-in definitions shipped as
//! markdown in `z-engine-prompts`, overridden by discovered custom
//! definitions of the same name (discovery already applied precedence
//! between custom folders).

use z_engine_config::{AgentDef, ExtensionScope, ExtensionSource, parse_agent};
use z_engine_tools::AgentCard;

use crate::work::AUDIT_AGENT;

#[derive(Debug, Clone, Default)]
pub(crate) struct AgentRegistry {
    defs: Vec<AgentDef>,
}

impl AgentRegistry {
    /// Built-ins first (in shipped order), then custom-only types by name.
    /// The built-in `audit` agent comes with `work_plans`.
    pub(crate) fn build(custom: &[AgentDef], work_plans: bool) -> Self {
        let mut defs: Vec<AgentDef> = z_engine_prompts::agents::BUILTIN
            .iter()
            .filter(|(name, _)| offered(name, work_plans))
            .filter_map(|(name, markdown)| {
                let source = ExtensionSource {
                    scope: ExtensionScope::User,
                    path: format!("<built-in>/{name}.md"),
                };
                match parse_agent(markdown, source) {
                    Ok(def) => Some(def),
                    Err(error) => {
                        tracing::error!(agent = name, %error, "built-in agent does not parse");
                        None
                    }
                }
            })
            .collect();
        let mut extra: Vec<&AgentDef> = Vec::new();
        for def in custom {
            match defs.iter_mut().find(|known| known.name == def.name) {
                Some(known) => *known = def.clone(),
                None => extra.push(def),
            }
        }
        extra.sort_by(|a, b| a.name.cmp(&b.name));
        defs.extend(extra.into_iter().cloned());
        Self { defs }
    }

    pub(crate) fn get(&self, name: &str) -> Option<&AgentDef> {
        self.defs.iter().find(|def| def.name == name)
    }

    pub(crate) fn names(&self) -> Vec<&str> {
        self.defs.iter().map(|def| def.name.as_str()).collect()
    }

    /// The definition, or the model-facing error listing the known types.
    pub(crate) fn resolve(&self, name: &str) -> Result<&AgentDef, String> {
        self.get(name).ok_or_else(|| {
            format!(
                "unknown subagent_type {name:?}; available types: {}",
                self.names().join(", ")
            )
        })
    }

    /// One card per type for the `Agent` tool description.
    pub(crate) fn cards(&self) -> Vec<AgentCard> {
        self.defs
            .iter()
            .map(|def| AgentCard {
                name: def.name.clone(),
                description: def.description.clone(),
                tools: tool_summary(def),
            })
            .collect()
    }
}

/// Whether the built-in agent `name` is offered.
pub(crate) fn offered(name: &str, work_plans: bool) -> bool {
    work_plans || name != AUDIT_AGENT
}

fn tool_summary(def: &AgentDef) -> String {
    let allowed = def
        .tools
        .as_ref()
        .map_or_else(|| "*".to_string(), |tools| tools.join(", "));
    if def.disallowed_tools.is_empty() {
        allowed
    } else {
        format!("{allowed}; not {}", def.disallowed_tools.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(name: &str, body: &str) -> AgentDef {
        let markdown = format!("---\nname: {name}\ndescription: Custom {name}.\n---\n{body}\n");
        let source = ExtensionSource {
            scope: ExtensionScope::Project,
            path: format!("/p/.z-engine/agents/{name}.md"),
        };
        parse_agent(&markdown, source).unwrap()
    }

    #[test]
    fn builtins_are_listed_and_custom_types_override_or_extend() {
        let custom = [
            custom("explore", "My explorer."),
            custom("auditor", "Audits."),
        ];
        let registry = AgentRegistry::build(&custom, false);
        assert_eq!(
            registry.names(),
            ["general", "explore", "plan", "review", "verify", "auditor"]
        );
        let names = AgentRegistry::build(&custom, true).names().join(" ");
        assert_eq!(names, "general explore plan review verify audit auditor");
        assert_eq!(
            registry.get("explore").unwrap().prompt.trim(),
            "My explorer."
        );
        let error = registry.resolve("nope").unwrap_err();
        assert!(error.contains("general, explore"), "{error}");
    }

    #[test]
    fn cards_summarize_tools() {
        let registry = AgentRegistry::build(&[], false);
        let cards = registry.cards();
        let general = cards.iter().find(|card| card.name == "general").unwrap();
        assert_eq!(general.tools, "*");
        let explore = cards.iter().find(|card| card.name == "explore").unwrap();
        assert!(
            explore.tools.starts_with("Read, Glob, Grep"),
            "{}",
            explore.tools
        );
    }
}
