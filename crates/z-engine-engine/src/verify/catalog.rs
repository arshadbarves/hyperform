//! The session's checks: discovered from manifests and merged with the
//! configured ones. Rebuilt at open and on settings reload; project-level
//! checks are already dropped from the settings of an untrusted project.

use std::path::Path;
use std::sync::{Arc, RwLock};

use z_engine_config::CheckConfig;
use z_engine_verify::{
    CheckSpec, ConfiguredCheck, DiscoveryOptions, ProjectProfile, discover, merge_configured,
};

use crate::sync::{read, write};

#[derive(Debug, Default)]
pub(crate) struct CheckHub {
    profile: RwLock<Arc<ProjectProfile>>,
}

impl CheckHub {
    pub(crate) fn profile(&self) -> Arc<ProjectProfile> {
        Arc::clone(&read(&self.profile))
    }

    pub(crate) fn set(&self, profile: ProjectProfile) {
        *write(&self.profile) = Arc::new(profile);
    }

    pub(crate) fn find(&self, id: &str) -> Option<CheckSpec> {
        let id = id.trim();
        self.profile()
            .checks
            .iter()
            .find(|check| check.id == id)
            .cloned()
    }
}

/// Discovery (bounded, read-only, off the async threads) plus the
/// configured checks; a failed discovery task yields only the configured.
pub(crate) async fn discover_checks(root: &Path, configured: &[CheckConfig]) -> ProjectProfile {
    let root = root.to_path_buf();
    let configured: Vec<ConfiguredCheck> = configured.iter().map(configured_check).collect();
    let fallback = (root.clone(), configured.clone());
    let discovered = tokio::task::spawn_blocking(move || {
        let mut profile = discover(&root, &DiscoveryOptions::default());
        merge_configured(&mut profile, &configured, &root);
        profile
    })
    .await;
    match discovered {
        Ok(profile) => profile,
        Err(error) => {
            tracing::warn!(%error, "check discovery failed");
            let (root, configured) = fallback;
            let mut profile = ProjectProfile::default();
            merge_configured(&mut profile, &configured, &root);
            profile
        }
    }
}

fn configured_check(check: &CheckConfig) -> ConfiguredCheck {
    ConfiguredCheck {
        id: check.id.clone(),
        label: check.label.clone(),
        command: check.command.clone(),
        kind: check.kind,
        cwd: check.cwd.clone(),
        timeout_secs: check.timeout_secs,
    }
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::CheckKind;

    use super::*;

    #[tokio::test]
    async fn configured_checks_join_discovered_ones() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let configured = [CheckConfig {
            id: "e2e".into(),
            command: "make e2e".into(),
            kind: CheckKind::Test,
            ..CheckConfig::default()
        }];
        let hub = CheckHub::default();
        hub.set(discover_checks(dir.path(), &configured).await);
        let profile = hub.profile();
        assert!(profile.checks.iter().any(|check| check.id == "cargo:test"));
        let custom = hub.find(" custom:e2e ").unwrap();
        assert_eq!(custom.command, "make e2e");
        assert_eq!(custom.label, "make e2e");
        assert!(hub.find("nope").is_none());
    }
}
