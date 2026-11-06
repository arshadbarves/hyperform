//! Rules (`rules/**/*.md`): instructions attached to requests always or
//! when matching files are involved.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ExtensionSource;
use super::frontmatter::{ListStyle, parse_document};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct RuleDef {
    pub name: String,
    pub description: String,
    /// Validated gitignore-style globs for the paths the rule applies to.
    pub globs: Vec<String>,
    /// Attach to every request regardless of paths.
    pub always_apply: bool,
    pub body: String,
    pub source: ExtensionSource,
}

/// `default_name` is the path under `rules/` in command form; frontmatter
/// `name` overrides it.
pub(crate) fn parse_rule(
    default_name: &str,
    markdown: &str,
    source: ExtensionSource,
) -> Result<RuleDef, String> {
    let doc = parse_document(markdown)?;
    let meta = &doc.meta;
    let globs = meta.list(&["globs"], ListStyle::Globs)?.unwrap_or_default();
    for glob in &globs {
        globset::Glob::new(glob).map_err(|error| format!("invalid glob `{glob}`: {error}"))?;
    }
    Ok(RuleDef {
        name: meta
            .text(&["name"])?
            .unwrap_or_else(|| default_name.to_string()),
        description: meta.text(&["description"])?.unwrap_or_default(),
        globs,
        always_apply: meta
            .flag(&["alwaysApply", "always_apply"])?
            .unwrap_or(false),
        body: doc.body.to_string(),
        source,
    })
}
