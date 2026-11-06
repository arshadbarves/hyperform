//! Pure v1 -> v2 key conversion. Keys v1 understood move to their v2
//! location; every other key is kept as written, so v2-shaped keys in a file
//! that lacks `schema` survive, and unknown keys stay as inert as v1 left
//! them.

use toml::{Table, Value};

use crate::document::new_table;
use crate::merge::{merge_layer, nested};

const REVIEW_NOTE: &str = "`review` was removed: v2 reviews changes with a review agent instead of an automatic post-edit pass";

/// The converted document (with `schema = 2`) and notes for the user.
pub(crate) fn convert_v1(v1: Table) -> (Table, Vec<String>) {
    let mut out = new_table();
    let mut notes = Vec::new();
    for (key, value) in v1 {
        match (key.as_str(), value) {
            ("schema", _) => {}
            ("model", Value::String(model)) => {
                put(&mut out, &["model", "main"], Value::String(model))
            }
            ("base_url", value) => put(&mut out, &["provider", "base_url"], value),
            ("max_context_tokens", value) => put(&mut out, &["model", "context_window"], value),
            ("max_output_tokens", value) => put(&mut out, &["model", "max_output_tokens"], value),
            ("compact_at_percent", value) => {
                put(&mut out, &["context", "compact_at_percent"], value)
            }
            ("max_task_continuations", value) => {
                put(&mut out, &["verification", "max_continuations"], value);
            }
            ("task_report_view", value) => put(&mut out, &["ui", "task_report_view"], value),
            ("shell_path", value) => put(&mut out, &["shell", "path"], value),
            ("review", _) => notes.push(REVIEW_NOTE.to_string()),
            ("hooks", Value::Table(hooks)) => convert_hooks(hooks, &mut out, &mut notes),
            ("permissions", Value::Table(rules)) => {
                convert_permissions(rules, &mut out, &mut notes)
            }
            ("cost", Value::Table(cost)) => convert_cost(cost, &mut out, &mut notes),
            (_, value) => put(&mut out, &[key.as_str()], value),
        }
    }
    (out, notes)
}

fn put(out: &mut Table, path: &[&str], value: Value) {
    merge_layer(out, nested(path, value));
}

/// v1 ran `session_start` and `turn_completed` only; other names never ran.
fn convert_hooks(hooks: Table, out: &mut Table, notes: &mut Vec<String>) {
    for (name, value) in hooks {
        let event = match name.as_str() {
            "session_start" => Some("SessionStart"),
            "turn_completed" => Some("Stop"),
            _ => None,
        };
        match (event, value) {
            (Some(event), Value::String(command)) => {
                let hook = Table::from_iter([("command".to_string(), Value::String(command))]);
                put(
                    out,
                    &["hooks", event],
                    Value::Array(vec![Value::Table(hook)]),
                );
                notes.push(format!("hook `{name}` now runs as hooks.{event}"));
            }
            (None, Value::String(_)) => {
                notes.push(format!("hook `{name}` was dropped: v1 never ran it"))
            }
            (_, value) => put(out, &["hooks", name.as_str()], value),
        }
    }
}

fn convert_permissions(rules: Table, out: &mut Table, notes: &mut Vec<String>) {
    for (key, value) in rules {
        let value = match (key.as_str(), value) {
            ("allow", Value::Array(entries)) => {
                let mut changed = 0;
                let converted = entries.into_iter().filter_map(|entry| match entry {
                    Value::String(rule) => {
                        let v2 = bash_rule(&rule);
                        changed += usize::from(v2.as_deref() != Some(rule.as_str()));
                        v2.map(Value::String)
                    }
                    other => Some(other),
                });
                let converted = Value::Array(converted.collect());
                if changed > 0 {
                    notes.push(format!(
                        "converted {changed} v1 shell allow rules to Bash(...) rules"
                    ));
                }
                converted
            }
            (_, value) => value,
        };
        put(out, &["permissions", key.as_str()], value);
    }
}

/// v1 allow entries are shell-command prefixes: a trailing `*` matches any
/// continuation, anything else matches the whole command exactly.
pub(crate) fn bash_rule(rule: &str) -> Option<String> {
    let rule = rule.trim();
    if rule.is_empty() {
        return None;
    }
    if is_tool_rule(rule) {
        return Some(rule.to_string());
    }
    Some(match rule.strip_suffix('*').map(str::trim_end) {
        Some("") => "Bash".to_string(),
        Some(prefix) => format!("Bash({prefix}:*)"),
        None => format!("Bash({rule})"),
    })
}

/// Already in v2 syntax: `Tool` or `Tool(specifier)` with a capitalized name.
fn is_tool_rule(rule: &str) -> bool {
    let (name, rest) = rule.split_at(rule.find('(').unwrap_or(rule.len()));
    name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && (rest.is_empty() || rest.ends_with(')'))
}

fn convert_cost(cost: Table, out: &mut Table, notes: &mut Vec<String>) {
    for (key, value) in cost {
        match (key.as_str(), value) {
            ("overrides", Value::Table(models)) => {
                for (model, pricing) in models {
                    put(out, &["pricing", model.as_str()], rename_pricing(pricing));
                }
            }
            _ => notes.push(format!("`cost.{key}` was dropped: v2 has no equivalent")),
        }
    }
}

fn rename_pricing(pricing: Value) -> Value {
    let Value::Table(fields) = pricing else {
        return pricing;
    };
    let renamed = fields.into_iter().map(|(key, value)| {
        let key = match key.as_str() {
            "usd_per_mtok_input" => "input".to_string(),
            "usd_per_mtok_output" => "output".to_string(),
            _ => key,
        };
        (key, value)
    });
    Value::Table(renamed.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(text: &str) -> (Table, Vec<String>) {
        convert_v1(toml::from_str(text).unwrap())
    }

    #[test]
    fn bash_prefix_rules_become_bash_rules() {
        assert_eq!(
            bash_rule("cargo test*").as_deref(),
            Some("Bash(cargo test:*)")
        );
        assert_eq!(
            bash_rule("cargo test *").as_deref(),
            Some("Bash(cargo test:*)")
        );
        assert_eq!(bash_rule("git status").as_deref(), Some("Bash(git status)"));
        assert_eq!(bash_rule("*").as_deref(), Some("Bash"));
        assert_eq!(bash_rule("Bash(ls:*)").as_deref(), Some("Bash(ls:*)"));
        assert_eq!(bash_rule("  ").as_deref(), None);
    }

    #[test]
    fn scalars_move_to_their_sections() {
        let (t, notes) = convert(
            "model = \"qwen/qwen3-coder\"\nbase_url = \"https://opencode.ai/zen/v1\"\n\
             max_context_tokens = 64000\nmax_output_tokens = 8000\ncompact_at_percent = 85\n\
             review = false\nmax_task_continuations = 4\ntask_report_view = \"compact\"\n\
             shell_path = \"C:/Git/bin/bash.exe\"\n",
        );
        assert_eq!(t["schema"].as_integer(), Some(2));
        assert_eq!(t["model"]["main"].as_str(), Some("qwen/qwen3-coder"));
        assert_eq!(t["model"]["context_window"].as_integer(), Some(64000));
        assert_eq!(t["model"]["max_output_tokens"].as_integer(), Some(8000));
        assert_eq!(
            t["provider"]["base_url"].as_str(),
            Some("https://opencode.ai/zen/v1")
        );
        assert_eq!(t["context"]["compact_at_percent"].as_integer(), Some(85));
        assert_eq!(t["verification"]["max_continuations"].as_integer(), Some(4));
        assert_eq!(t["ui"]["task_report_view"].as_str(), Some("compact"));
        assert_eq!(t["shell"]["path"].as_str(), Some("C:/Git/bin/bash.exe"));
        assert!(t.get("review").is_none());
        assert_eq!(notes, [REVIEW_NOTE]);
    }

    #[test]
    fn sections_convert_and_unknown_keys_pass_through() {
        let (t, notes) = convert(
            "[hooks]\nsession_start = \"./start.sh\"\nturn_completed = \"cargo fmt\"\nlater = \"x\"\n\
             [permissions]\nallow = [\"cargo test*\", \"git status\"]\n\
             [mcp.servers.fs]\ncommand = \"npx\"\nargs = [\"-y\", \"fs\"]\n\
             [cost.overrides]\n\"my/model\" = { usd_per_mtok_input = 1.0, usd_per_mtok_output = 2.0 }\n\
             [web]\nfetch_extract = false\n",
        );
        assert_eq!(
            t["hooks"]["SessionStart"][0]["command"].as_str(),
            Some("./start.sh")
        );
        assert_eq!(t["hooks"]["Stop"][0]["command"].as_str(), Some("cargo fmt"));
        assert!(t["hooks"].get("later").is_none());
        let allow = t["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow[0].as_str(), Some("Bash(cargo test:*)"));
        assert_eq!(allow[1].as_str(), Some("Bash(git status)"));
        assert_eq!(t["mcp"]["servers"]["fs"]["args"][1].as_str(), Some("fs"));
        assert_eq!(t["pricing"]["my/model"]["input"].as_float(), Some(1.0));
        assert_eq!(t["pricing"]["my/model"]["output"].as_float(), Some(2.0));
        assert_eq!(t["web"]["fetch_extract"].as_bool(), Some(false));
        assert!(t.get("cost").is_none());
        assert_eq!(notes.len(), 4, "{notes:?}");
    }

    #[test]
    fn v2_shaped_sections_without_schema_are_kept() {
        let (t, notes) = convert("max_output_tokens = 1000\n[model]\nmain = \"m\"\n");
        assert_eq!(t["model"]["main"].as_str(), Some("m"));
        assert_eq!(t["model"]["max_output_tokens"].as_integer(), Some(1000));
        assert!(notes.is_empty());
    }
}
