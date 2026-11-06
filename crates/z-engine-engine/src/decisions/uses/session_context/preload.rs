//! Deferred MCP tools picked for the first request, held from the
//! turn-start seam until the run that serves the request exists.

use std::sync::Mutex;

use crate::run::RunContext;
use crate::sync::lock;

#[derive(Debug, Default)]
pub(crate) struct ToolPreload {
    names: Mutex<Vec<String>>,
}

impl ToolPreload {
    pub(super) fn set(&self, names: Vec<String>) {
        *lock(&self.names) = names;
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *lock(&self.names))
    }
}

/// Loads the picked tools that are still in the catalog into `ctx`'s run,
/// as `LoadMcpTools` would.
pub(crate) fn preload_tools(ctx: &RunContext) {
    let names = ctx.core.decisions.preload().take();
    if names.is_empty() {
        return;
    }
    let catalog = ctx.core.mcp.catalog();
    let known = names.into_iter().filter(|name| catalog.contains(name));
    ctx.mcp.load(known);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_are_taken_once() {
        let preload = ToolPreload::default();
        preload.set(vec!["mcp__docs__search".into()]);
        assert_eq!(preload.take(), vec!["mcp__docs__search".to_string()]);
        assert!(preload.take().is_empty());
    }
}
