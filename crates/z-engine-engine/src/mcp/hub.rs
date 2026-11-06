//! [`McpHub`]: one manager per configured server (so a changed server
//! restarts alone), the installed tool catalog, and the channel that
//! carries server changes to the session's watcher task.

use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::mpsc::UnboundedSender;
use z_engine_integrations::{McpChange, McpManager, McpPromptInfo, McpServerState};

use super::catalog::{Catalog, CatalogTool};
use super::specs::ServerPlan;
use crate::sync::{lock, read, write};

#[derive(Debug, Clone)]
pub(super) struct Server {
    pub plan: ServerPlan,
    pub manager: McpManager,
}

/// What reconciling the configured servers changed.
#[derive(Debug, Default)]
pub(super) struct Reconciled {
    /// New or reconfigured servers, still to be started.
    pub started: Vec<Server>,
    /// Removed or reconfigured servers, still to be shut down.
    pub stopped: Vec<McpManager>,
}

/// `(server, prompt)` of every ready server, listed with the tools.
pub(crate) type PromptList = Arc<Vec<(String, McpPromptInfo)>>;

#[derive(Debug, Default)]
pub(crate) struct McpHub {
    servers: Mutex<Vec<Server>>,
    catalog: RwLock<Arc<Catalog>>,
    prompts: RwLock<PromptList>,
    pub(super) changes: Mutex<Option<UnboundedSender<McpChange>>>,
}

impl McpHub {
    pub(crate) fn catalog(&self) -> Arc<Catalog> {
        Arc::clone(&read(&self.catalog))
    }

    /// The prompts of ready servers as of the last refresh.
    pub(crate) fn prompts(&self) -> PromptList {
        Arc::clone(&read(&self.prompts))
    }

    pub(super) fn set_prompts(&self, prompts: Vec<(String, McpPromptInfo)>) {
        *write(&self.prompts) = Arc::new(prompts);
    }

    /// The manager running `server`.
    pub(crate) fn manager(&self, server: &str) -> Option<McpManager> {
        lock(&self.servers)
            .iter()
            .find(|entry| entry.plan.spec.name == server)
            .map(|entry| entry.manager.clone())
    }

    /// Every manager, in configuration order.
    pub(crate) fn managers(&self) -> Vec<McpManager> {
        lock(&self.servers)
            .iter()
            .map(|entry| entry.manager.clone())
            .collect()
    }

    pub(crate) fn any_ready(&self) -> bool {
        self.managers().iter().any(|manager| {
            manager
                .status()
                .iter()
                .any(|status| status.state == McpServerState::Ready)
        })
    }

    pub(super) fn servers(&self) -> Vec<Server> {
        lock(&self.servers).clone()
    }

    /// Keeps servers whose spec is unchanged (taking new tool filters),
    /// and replaces the rest.
    pub(super) fn reconcile(&self, plans: Vec<ServerPlan>) -> Reconciled {
        let mut servers = lock(&self.servers);
        let mut kept = Vec::with_capacity(plans.len());
        let mut reconciled = Reconciled::default();
        for plan in plans {
            let existing = servers
                .iter()
                .position(|entry| entry.plan.spec.name == plan.spec.name)
                .map(|index| servers.remove(index));
            match existing {
                Some(entry) if entry.plan.spec == plan.spec => kept.push(Server {
                    plan,
                    manager: entry.manager,
                }),
                other => {
                    reconciled.stopped.extend(other.map(|entry| entry.manager));
                    let server = Server {
                        plan,
                        manager: McpManager::new(),
                    };
                    reconciled.started.push(server.clone());
                    kept.push(server);
                }
            }
        }
        reconciled
            .stopped
            .extend(servers.drain(..).map(|entry| entry.manager));
        *servers = kept;
        reconciled
    }

    /// Installs `tools`; returns whether they differ from the current ones.
    pub(super) fn install(&self, tools: Vec<CatalogTool>) -> bool {
        let mut catalog = write(&self.catalog);
        if catalog.tools == tools {
            return false;
        }
        *catalog = Arc::new(Catalog {
            tools,
            version: catalog.version + 1,
        });
        true
    }

    /// Stops every server and forgets them (session close).
    pub(crate) async fn shutdown(&self) {
        let managers: Vec<McpManager> = lock(&self.servers)
            .drain(..)
            .map(|entry| entry.manager)
            .collect();
        lock(&self.changes).take();
        futures::future::join_all(managers.iter().map(McpManager::shutdown_all)).await;
    }
}

#[cfg(test)]
mod tests {
    use z_engine_integrations::McpServerSpec;

    use super::*;

    fn plan(name: &str, command: &str) -> ServerPlan {
        ServerPlan {
            spec: McpServerSpec::stdio(name, command, Vec::new()),
            disabled_tools: Vec::new(),
        }
    }

    #[test]
    fn only_changed_servers_restart() {
        let hub = McpHub::default();
        let first = hub.reconcile(vec![plan("a", "x"), plan("b", "y")]);
        assert_eq!(first.started.len(), 2);
        assert!(first.stopped.is_empty());
        let mut filtered = plan("a", "x");
        filtered.disabled_tools = vec!["drop".into()];
        let second = hub.reconcile(vec![filtered, plan("c", "z")]);
        assert_eq!(second.started.len(), 1);
        assert_eq!(second.started[0].plan.spec.name, "c");
        assert_eq!(second.stopped.len(), 1, "b was removed");
        assert_eq!(hub.servers()[0].plan.disabled_tools, ["drop"]);
        let third = hub.reconcile(vec![plan("a", "other"), plan("c", "z")]);
        assert_eq!((third.started.len(), third.stopped.len()), (1, 1));
        assert!(hub.manager("a").is_some() && hub.manager("b").is_none());
        assert!(!hub.any_ready());
    }
}
