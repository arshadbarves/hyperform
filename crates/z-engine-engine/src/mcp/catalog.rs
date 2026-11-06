//! The merged MCP tool catalog of a session: what the registry offers as
//! `mcp__` tools, and the listing sent instead of their schemas when there
//! are too many to offer at once.

use std::sync::Arc;

use z_engine_context::{render_template, wrap_reminder};
use z_engine_integrations::McpToolInfo;
use z_engine_prompts::reminders::MCP_TOOLS_DEFERRED;
use z_engine_tools::builtin::LoadMcpToolsTool;
use z_engine_tools::{ToolRegistry, mcp_tool};

/// Above this many MCP tools, their schemas are loaded on demand.
pub(crate) const DEFER_ABOVE: usize = 40;
/// Longest tool summary in the deferred listing.
const SUMMARY_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CatalogTool {
    pub server: String,
    /// Model-facing `mcp__server__tool` name.
    pub name: String,
    pub info: McpToolInfo,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Catalog {
    /// Server order, then each server's order.
    pub tools: Vec<CatalogTool>,
    /// Bumped whenever the tools change.
    pub version: u64,
}

impl Catalog {
    pub(crate) fn deferred(&self) -> bool {
        self.tools.len() > DEFER_ABOVE
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        self.tools.iter().any(|tool| tool.name == name)
    }

    /// Adds every tool, plus `LoadMcpTools` while the tools are deferred.
    pub(crate) fn register(&self, registry: &mut ToolRegistry) {
        for tool in &self.tools {
            registry.register(mcp_tool(
                &tool.server,
                &tool.info.name,
                tool.info.description.as_deref().unwrap_or_default(),
                tool.info.input_schema.clone(),
                tool.info.annotations.read_only(),
            ));
        }
        if self.deferred() {
            registry.register(Arc::new(LoadMcpToolsTool));
        }
    }

    /// The reminder listing every tool by name and summary.
    pub(crate) fn listing(&self) -> String {
        let lines: Vec<String> = self
            .tools
            .iter()
            .map(|tool| match summary(&tool.info) {
                Some(summary) => format!("- {}: {summary}", tool.name),
                None => format!("- {}", tool.name),
            })
            .collect();
        wrap_reminder(&render_template(
            MCP_TOOLS_DEFERRED,
            &[("tools", &lines.join("\n"))],
        ))
    }
}

/// The first non-blank description line, cut to [`SUMMARY_CHARS`].
fn summary(info: &McpToolInfo) -> Option<String> {
    let line = info
        .description
        .as_deref()?
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    if line.chars().count() <= SUMMARY_CHARS {
        return Some(line.to_string());
    }
    let cut: String = line.chars().take(SUMMARY_CHARS).collect();
    Some(format!("{cut}..."))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_integrations::ToolAnnotations;

    use super::*;

    fn tool(index: usize) -> CatalogTool {
        CatalogTool {
            server: "srv".into(),
            name: format!("mcp__srv__t{index}"),
            info: McpToolInfo {
                name: format!("t{index}"),
                title: None,
                description: Some(format!("\n  Tool {index}.\nMore detail.")),
                input_schema: json!({"type": "object"}),
                annotations: ToolAnnotations::default(),
            },
        }
    }

    #[test]
    fn many_tools_are_deferred_behind_load_mcp_tools() {
        let small = Catalog {
            tools: (0..3).map(tool).collect(),
            version: 1,
        };
        let mut registry = ToolRegistry::new();
        small.register(&mut registry);
        assert_eq!(registry.len(), 3);
        assert!(!small.deferred() && small.contains("mcp__srv__t2"));
        let large = Catalog {
            tools: (0..=DEFER_ABOVE).map(tool).collect(),
            version: 2,
        };
        let mut registry = ToolRegistry::new();
        large.register(&mut registry);
        assert!(large.deferred());
        assert!(registry.get("LoadMcpTools").is_some());
        let listing = large.listing();
        assert!(listing.starts_with("<system-reminder>"));
        assert!(listing.contains("- mcp__srv__t0: Tool 0.\n"));
        assert!(!listing.contains("More detail"));
    }
}
