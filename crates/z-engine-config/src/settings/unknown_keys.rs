//! Detects misspelled keys. The known keys come from serializing the
//! default [`Settings`], so the check follows the schema without a list.

use serde_json::Value as Json;
use toml::Table;

use super::Settings;
use crate::features::FeatureId;

/// Sections whose keys are user-chosen names rather than schema fields.
const NAMED_SECTIONS: [&str; 2] = ["hooks", "pricing"];

/// Top-level and section keys of `table` that the schema does not know.
pub(crate) fn unknown_keys(table: &Table) -> Vec<String> {
    let Ok(Json::Object(schema)) = serde_json::to_value(Settings::default()) else {
        return Vec::new();
    };
    let mut unknown = Vec::new();
    for (key, value) in table {
        if key == "schema" || NAMED_SECTIONS.contains(&key.as_str()) {
            continue;
        }
        if let ("experimental", toml::Value::Table(features)) = (key.as_str(), value) {
            let ids = features.keys().filter(|id| FeatureId::parse(id).is_none());
            unknown.extend(ids.map(|id| format!("experimental.{id}")));
            continue;
        }
        match (schema.get(key), value) {
            (None, _) => unknown.push(key.clone()),
            (Some(Json::Object(fields)), toml::Value::Table(section)) => {
                section_keys(key, fields, section, &mut unknown);
            }
            _ => {}
        }
    }
    unknown.sort();
    unknown
}

/// Keys of `section` missing from `fields`, descending into sub-sections
/// with a fixed schema (`shell.sandbox`). Empty schema maps (`shell.env`,
/// `mcp.servers`) hold user-chosen names and are not checked.
fn section_keys(
    prefix: &str,
    fields: &serde_json::Map<String, Json>,
    section: &Table,
    unknown: &mut Vec<String>,
) {
    for (field, value) in section {
        let path = format!("{prefix}.{field}");
        match (fields.get(field), value) {
            (None, _) => unknown.push(path),
            (Some(Json::Object(nested)), toml::Value::Table(inner)) if !nested.is_empty() => {
                section_keys(&path, nested, inner, unknown);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_typos_but_not_named_entries() {
        let table: Table = toml::from_str(
            r#"
schema = 2
modle = "x"
[model]
main = "m"
effrot = "high"
[hooks.MyEvent]
[pricing."a/b"]
input = 1.0
output = 2.0
[mcp.servers.fs]
command = "npx"
[shell.env]
MY_VAR = "1"
[shell.sandbox]
enabled = true
alow_network = true
[experimental]
decisions_compaction = "shadow"
decisions_compacton = "on"
[decisions]
timeout_ms = 300
time_out = 1
[decisions.calibration.compaction_relevant]
temperature = 1.5
[decisions.sidecar]
comand = "laya-serve"
"#,
        )
        .unwrap();
        assert_eq!(
            unknown_keys(&table),
            vec![
                "decisions.sidecar.comand",
                "decisions.time_out",
                "experimental.decisions_compacton",
                "model.effrot",
                "modle",
                "shell.sandbox.alow_network"
            ]
        );
    }
}
