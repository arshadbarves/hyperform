//! `SkillPort`: skills discovered with the session's extensions; bodies
//! are loaded on demand.

use std::sync::Arc;

use z_engine_config::load_skill_body;
use z_engine_tools::{SkillContent, SkillPort};

use crate::session::SessionCore;

#[derive(Debug)]
pub(crate) struct Skills {
    core: Arc<SessionCore>,
}

impl Skills {
    pub(crate) fn new(core: Arc<SessionCore>) -> Self {
        Self { core }
    }
}

impl SkillPort for Skills {
    fn list(&self) -> Vec<(String, String)> {
        self.core
            .settings()
            .extensions
            .skills
            .iter()
            .map(|skill| (skill.name.clone(), skill.description.clone()))
            .collect()
    }

    fn load(&self, name: &str) -> Result<SkillContent, String> {
        let settings = self.core.settings();
        let skill = settings
            .extensions
            .skills
            .iter()
            .find(|skill| skill.name == name.trim())
            .ok_or_else(|| format!("unknown skill `{name}`"))?;
        let body = load_skill_body(skill).map_err(|error| error.to_string())?;
        Ok(SkillContent {
            body,
            dir: skill.dir.clone(),
        })
    }
}
