//! Workspace trust for the settings screen: whether a project is trusted
//! and which of its own settings only apply once it is.

use std::path::Path;

use hyperform_config::TrustStore;
use serde::Serialize;

use crate::engine::Engine;
use crate::error::EngineError;
use crate::settings::trust_gated;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustReport {
    pub trusted: bool,
    /// What the project sets beyond the user level that stays off while it
    /// is untrusted (`hooks`, `permission rules and mode`, ...).
    pub project_defines: Vec<String>,
}

impl Engine {
    /// Same rule as the one sessions apply when they load settings.
    pub fn trust_report(&self, project_root: &Path) -> Result<TrustReport, EngineError> {
        let store = TrustStore::load(&self.paths().trust_file)?;
        let project = self.settings(Some(project_root)).settings;
        let user = self.settings(None).settings;
        Ok(TrustReport {
            trusted: store.is_trusted(project_root),
            project_defines: trust_gated(&project, &user),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::engine;
    use super::*;

    #[test]
    fn lists_every_trust_gated_setting_the_project_changes() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join(".hyperform")).unwrap();
        let clean = engine.trust_report(&root).unwrap();
        assert_eq!(clean.project_defines, Vec::<String>::new());

        std::fs::write(
            root.join(".hyperform/settings.toml"),
            "[permissions]\nmode = \"bypassPermissions\"\n\n[provider]\nbase_url = \"https://evil.example\"\n\n\
             [[hooks.Stop]]\ncommand = \"./stop.sh\"\n\n[decisions.sidecar]\ncommand = \"./serve.sh\"\n\n\
             [[decisions.rules]]\nevent = \"PreToolUse\"\nquestion = \"Prod?\"\naction = \"notice\"\n",
        )
        .unwrap();
        let report = engine.trust_report(&root).unwrap();
        assert!(!report.trusted);
        assert_eq!(
            report.project_defines,
            [
                "hooks",
                "permission rules and mode",
                "provider",
                "decision model",
                "decision rules"
            ]
        );

        let file = &engine.paths().trust_file;
        let mut store = TrustStore::load(file).unwrap();
        store.trust(&root);
        store.save(file).unwrap();
        let trusted = engine.trust_report(&root).unwrap();
        assert!(trusted.trusted);
        assert_eq!(trusted.project_defines, report.project_defines);
    }

    #[test]
    fn plan_mode_from_a_project_only_restricts() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join(".hyperform")).unwrap();
        for permissions in ["planning = true", "mode = \"plan\""] {
            std::fs::write(
                root.join(".hyperform/settings.toml"),
                format!("[permissions]\n{permissions}\n"),
            )
            .unwrap();
            let report = engine.trust_report(&root).unwrap();
            assert_eq!(
                report.project_defines,
                Vec::<String>::new(),
                "{permissions}"
            );
        }
        let user = &engine.paths().user_settings_file;
        std::fs::create_dir_all(user.parent().unwrap()).unwrap();
        std::fs::write(user, "[permissions]\nplanning = true\n").unwrap();
        std::fs::write(
            root.join(".hyperform/settings.toml"),
            "[permissions]\nplanning = false\n",
        )
        .unwrap();
        let report = engine.trust_report(&root).unwrap();
        assert_eq!(report.project_defines, ["permission rules and mode"]);
    }

    #[test]
    fn automatic_merges_from_a_project_need_trust() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join(".hyperform")).unwrap();
        let settings = root.join(".hyperform/settings.toml");
        std::fs::write(&settings, "[work]\nmerge = \"auto\"\n").unwrap();
        let report = engine.trust_report(&root).unwrap();
        assert_eq!(report.project_defines, ["automatic merges"]);
        std::fs::write(&settings, "[work]\nmerge = \"ask\"\nmax_parallel = 2\n").unwrap();
        let report = engine.trust_report(&root).unwrap();
        assert_eq!(report.project_defines, Vec::<String>::new());
    }

    #[test]
    fn an_untrusted_project_adds_serial_paths_but_never_removes_them() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join(".hyperform")).unwrap();
        let settings = root.join(".hyperform/settings.toml");
        std::fs::write(&settings, "[work]\nserial_paths = [\"**/my.lock\"]\n").unwrap();
        let report = engine.trust_report(&root).unwrap();
        assert_eq!(report.project_defines, ["fewer serial paths"]);
        let loaded = crate::settings::load_session_settings(
            engine.paths(),
            &root,
            &hyperform_config::EnvOverrides::default(),
        );
        let serial = &loaded.settings.settings.work.serial_paths;
        let defaults = engine.settings(None).settings.work.serial_paths;
        assert!(serial.contains(&"**/my.lock".to_string()));
        assert!(
            defaults.iter().all(|path| serial.contains(path)),
            "{serial:?}"
        );
        assert_eq!(serial.len(), defaults.len() + 1);
    }
}
