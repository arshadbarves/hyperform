//! Output styles (`output-styles/*.md`): replacement response-style
//! instructions selected by `ui.output_style`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ExtensionSource;
use super::frontmatter::parse_document;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct OutputStyleDef {
    pub name: String,
    pub description: String,
    pub body: String,
    pub source: ExtensionSource,
}

/// The name defaults to the file stem.
pub(crate) fn parse_output_style(
    markdown: &str,
    source: ExtensionSource,
) -> Result<OutputStyleDef, String> {
    let doc = parse_document(markdown)?;
    let name = match doc.meta.text(&["name"])? {
        Some(name) => name,
        None => source.stem().ok_or("missing `name`")?,
    };
    Ok(OutputStyleDef {
        name,
        description: doc.meta.text(&["description"])?.unwrap_or_default(),
        body: doc.body.to_string(),
        source,
    })
}
