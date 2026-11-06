//! [`LspHub`]: the session's language servers while `lsp.enabled`, as an
//! [`LspWorker`] replaced when the server specs change on reload. Servers
//! start lazily, on the first file asked about.

use std::path::Path;
use std::sync::{Arc, RwLock};

use z_engine_config::{LspServerConfig, LspSettings};
use z_engine_integrations::{LspServerSpec, merge_specs, presets};

use super::worker::LspWorker;
use crate::sync::{read, write};

#[derive(Debug, Clone)]
struct Active {
    specs: Vec<LspServerSpec>,
    worker: Arc<LspWorker>,
}

#[derive(Debug, Default)]
pub(crate) struct LspHub {
    active: RwLock<Option<Active>>,
}

impl LspHub {
    /// Applies the settings; returns a replaced worker for the caller to
    /// shut down.
    pub(crate) fn configure(&self, root: &Path, settings: &LspSettings) -> Option<Arc<LspWorker>> {
        let specs = settings.enabled.then(|| server_specs(settings));
        let mut active = write(&self.active);
        match (specs, active.as_ref()) {
            (Some(specs), Some(current)) if current.specs == specs => None,
            (Some(specs), _) => {
                let worker = Arc::new(LspWorker::new(root.to_path_buf(), specs.clone()));
                active
                    .replace(Active { specs, worker })
                    .map(|old| old.worker)
            }
            (None, _) => active.take().map(|old| old.worker),
        }
    }

    pub(crate) fn worker(&self) -> Option<Arc<LspWorker>> {
        read(&self.active)
            .as_ref()
            .map(|active| Arc::clone(&active.worker))
    }

    /// Names of the servers the settings enable (empty when LSP is off).
    pub(crate) fn server_names(&self) -> Vec<String> {
        read(&self.active).as_ref().map_or_else(Vec::new, |active| {
            active.specs.iter().map(|spec| spec.name.clone()).collect()
        })
    }

    /// Whether some server claims `path`'s extension.
    pub(crate) fn covers(&self, path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            return false;
        };
        read(&self.active)
            .as_ref()
            .is_some_and(|active| active.specs.iter().any(|spec| spec.handles(extension)))
    }

    pub(crate) async fn shutdown(&self) {
        let active = write(&self.active).take();
        if let Some(active) = active {
            active.worker.shutdown().await;
        }
    }
}

/// Enabled configured servers merged over the presets; a disabled entry
/// also removes the preset of the same name.
fn server_specs(settings: &LspSettings) -> Vec<LspServerSpec> {
    let configured = settings
        .servers
        .iter()
        .filter(|(_, server)| server.enabled && !server.command.trim().is_empty())
        .map(|(name, server)| spec(name, server))
        .collect();
    let disabled: Vec<&String> = settings
        .servers
        .iter()
        .filter(|(_, server)| !server.enabled)
        .map(|(name, _)| name)
        .collect();
    merge_specs(presets(), configured)
        .into_iter()
        .filter(|spec| !disabled.contains(&&spec.name))
        .collect()
}

fn spec(name: &str, server: &LspServerConfig) -> LspServerSpec {
    let owned = |items: &[String]| -> Vec<String> {
        items
            .iter()
            .map(|item| item.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|item| !item.is_empty())
            .collect()
    };
    let mut spec = LspServerSpec::new(name, server.command.trim(), &[], &[], &[]);
    spec.args = server.args.clone();
    spec.extensions = owned(&server.extensions);
    spec.root_markers = server.root_markers.clone();
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_servers_win_and_disabled_ones_vanish() {
        let mut settings = LspSettings::default();
        settings.servers.insert(
            "fake".into(),
            LspServerConfig {
                command: "/bin/fake".into(),
                extensions: vec![".RS".into()],
                ..LspServerConfig::default()
            },
        );
        settings.servers.insert(
            "clangd".into(),
            LspServerConfig {
                enabled: false,
                ..LspServerConfig::default()
            },
        );
        let specs = server_specs(&settings);
        assert_eq!(specs[0].name, "fake");
        assert_eq!(specs[0].extensions, ["rs"]);
        assert!(specs.iter().all(|spec| spec.name != "clangd"));
        let hub = LspHub::default();
        let dir = tempfile::tempdir().unwrap();
        assert!(hub.configure(dir.path(), &settings).is_none());
        assert!(hub.covers(Path::new("src/lib.rs")) && !hub.covers(Path::new("x.txt")));
        assert!(hub.configure(dir.path(), &settings).is_none(), "unchanged");
        settings.enabled = false;
        assert!(hub.configure(dir.path(), &settings).is_some());
        assert!(hub.worker().is_none() && !hub.covers(Path::new("a.rs")));
    }
}
