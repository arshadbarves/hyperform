//! Skills (`skills/<dir>/SKILL.md`): instructions and bundled files the
//! model loads on demand. Discovery reads only the frontmatter's metadata;
//! the body is loaded when the skill is used.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::frontmatter::{ListStyle, parse_document};
use super::{EXTENSION_FILE_LIMIT, ExtensionSource};
use crate::error::ConfigError;
use crate::files::{lossy_text, read_capped};

pub const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct SkillDef {
    pub name: String,
    /// When to use the skill; shown to the model.
    pub description: String,
    /// Tool rules granted while the skill is active.
    pub allowed_tools: Vec<String>,
    /// The skill directory, which holds any bundled files.
    pub dir: String,
    /// Path of `SKILL.md`.
    pub file: String,
    pub source: ExtensionSource,
}

/// The name defaults to the directory name; a description is required.
pub(crate) fn parse_skill(
    dir: &Path,
    markdown: &str,
    source: ExtensionSource,
) -> Result<SkillDef, String> {
    let doc = parse_document(markdown)?;
    let meta = &doc.meta;
    let dir_name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let name = meta.text(&["name"])?.or(dir_name).ok_or("missing `name`")?;
    Ok(SkillDef {
        name,
        description: meta
            .text(&["description"])?
            .ok_or("missing `description`")?,
        allowed_tools: meta
            .list(
                &["allowed-tools", "allowed_tools", "allowedTools"],
                ListStyle::Tools,
            )?
            .unwrap_or_default(),
        dir: dir.to_string_lossy().into_owned(),
        file: source.path.clone(),
        source,
    })
}

/// The instructions of `SKILL.md`, without frontmatter.
pub fn load_skill_body(skill: &SkillDef) -> Result<String, ConfigError> {
    let path = Path::new(&skill.file);
    let (bytes, truncated) =
        read_capped(path, EXTENSION_FILE_LIMIT).map_err(|error| ConfigError::io(path, error))?;
    if truncated {
        return Err(ConfigError::TooLarge {
            path: path.to_path_buf(),
            limit: EXTENSION_FILE_LIMIT,
        });
    }
    let text = lossy_text(bytes);
    let doc = parse_document(&text).map_err(|message| ConfigError::parse(path, message))?;
    Ok(doc.body.to_string())
}
