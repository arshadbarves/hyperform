//! Per-run state of deferred MCP tools: which ones the agent loaded, and
//! which catalog version it was last shown.

use std::collections::BTreeSet;
use std::sync::Mutex;

use super::catalog::Catalog;
use crate::sync::lock;

#[derive(Debug, Default)]
pub(crate) struct McpRunState {
    loaded: Mutex<BTreeSet<String>>,
    announced: Mutex<Option<u64>>,
}

impl McpRunState {
    pub(crate) fn loaded(&self) -> BTreeSet<String> {
        lock(&self.loaded).clone()
    }

    pub(crate) fn load(&self, names: impl IntoIterator<Item = String>) {
        lock(&self.loaded).extend(names);
    }

    /// The listing reminder while tools are deferred and this run has not
    /// seen the current catalog yet.
    pub(crate) fn reminder(&self, catalog: &Catalog) -> Option<String> {
        if !catalog.deferred() {
            return None;
        }
        let mut announced = lock(&self.announced);
        if *announced == Some(catalog.version) {
            return None;
        }
        *announced = Some(catalog.version);
        Some(catalog.listing())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_integrations::{McpToolInfo, ToolAnnotations};

    use super::super::catalog::{CatalogTool, DEFER_ABOVE};
    use super::*;

    fn catalog(count: usize, version: u64) -> Catalog {
        let tools = (0..count)
            .map(|index| CatalogTool {
                server: "s".into(),
                name: format!("mcp__s__t{index}"),
                info: McpToolInfo {
                    name: format!("t{index}"),
                    title: None,
                    description: None,
                    input_schema: json!({}),
                    annotations: ToolAnnotations::default(),
                },
            })
            .collect();
        Catalog { tools, version }
    }

    #[test]
    fn each_catalog_version_is_announced_once() {
        let state = McpRunState::default();
        assert!(state.reminder(&catalog(2, 1)).is_none());
        let big = catalog(DEFER_ABOVE + 1, 2);
        assert!(state.reminder(&big).is_some());
        assert!(state.reminder(&big).is_none());
        assert!(state.reminder(&catalog(DEFER_ABOVE + 2, 3)).is_some());
        state.load(["mcp__s__t1".to_string()]);
        assert!(state.loaded().contains("mcp__s__t1"));
    }
}
