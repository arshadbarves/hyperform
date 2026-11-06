//! Deep merge of raw TOML layers, applied before deserialization so list
//! values can union, concatenate, or merge by identity.
//!
//! Tables merge recursively and scalars and other arrays override, except:
//! rule lists, `shell.env_passthrough` and `shell.sandbox.extra_writable`
//! union (deduplicated, in order);
//! `hooks.<event>` and `decisions.rules` lists concatenate;
//! `verification.checks` merge by `id`,
//! a later check replacing the earlier one with that id in place; and a
//! later `mcp.servers.<name>` or `lsp.servers.<name>` replaces the earlier
//! server entirely.

use toml::{Table, Value};

const UNION_ARRAYS: [&[&str]; 6] = [
    &["permissions", "allow"],
    &["permissions", "ask"],
    &["permissions", "deny"],
    &["permissions", "additional_directories"],
    &["shell", "env_passthrough"],
    &["shell", "sandbox", "extra_writable"],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    Deep,
    Union,
    Concat,
    ById,
    Replace,
}

fn rule_for(path: &[String]) -> Rule {
    match path {
        _ if UNION_ARRAYS.iter().any(|union| union.iter().eq(path)) => Rule::Union,
        [section, _] if section == "hooks" => Rule::Concat,
        [section, key] if section == "decisions" && key == "rules" => Rule::Concat,
        [section, key] if section == "verification" && key == "checks" => Rule::ById,
        [section, servers, _] if (section == "mcp" || section == "lsp") && servers == "servers" => {
            Rule::Replace
        }
        _ => Rule::Deep,
    }
}

/// Merges `overlay` (a higher-precedence layer) into `base`.
pub(crate) fn merge_layer(base: &mut Table, overlay: Table) {
    merge_tables(base, overlay, &mut Vec::new());
}

/// `{a = {b = value}}` for the path `["a", "b"]`.
pub(crate) fn nested(path: &[&str], value: Value) -> Table {
    let Some((first, rest)) = path.split_first() else {
        return Table::new();
    };
    let inner = rest.iter().rev().fold(value, |inner, key| {
        Value::Table(Table::from_iter([((*key).to_string(), inner)]))
    });
    Table::from_iter([((*first).to_string(), inner)])
}

fn merge_tables(base: &mut Table, overlay: Table, path: &mut Vec<String>) {
    for (key, value) in overlay {
        path.push(key.clone());
        match base.get_mut(&key) {
            Some(slot) => combine(slot, value, path),
            None => {
                let value = match value {
                    Value::Array(_) | Value::Table(_) => {
                        let mut slot = empty_like(&value);
                        combine(&mut slot, value, path);
                        slot
                    }
                    scalar => scalar,
                };
                base.insert(key, value);
            }
        }
        path.pop();
    }
}

fn combine(slot: &mut Value, value: Value, path: &mut Vec<String>) {
    match (rule_for(path), slot, value) {
        (Rule::Union, Value::Array(items), Value::Array(incoming)) => {
            for item in incoming {
                if !items.contains(&item) {
                    items.push(item);
                }
            }
        }
        (Rule::Concat, Value::Array(items), Value::Array(incoming)) => items.extend(incoming),
        (Rule::ById, Value::Array(items), Value::Array(incoming)) => merge_by_id(items, incoming),
        (Rule::Deep, Value::Table(table), Value::Table(incoming)) => {
            merge_tables(table, incoming, path);
        }
        (_, slot, value) => *slot = value,
    }
}

fn merge_by_id(items: &mut Vec<Value>, incoming: Vec<Value>) {
    for item in incoming {
        let id = item.get("id").and_then(Value::as_str).map(str::to_string);
        let existing = id.and_then(|id| {
            items
                .iter()
                .position(|old| old.get("id").and_then(Value::as_str) == Some(id.as_str()))
        });
        match existing {
            Some(index) => items[index] = item,
            None => items.push(item),
        }
    }
}

fn empty_like(value: &Value) -> Value {
    match value {
        Value::Array(_) => Value::Array(Vec::new()),
        _ => Value::Table(Table::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merged(layers: &[&str]) -> Table {
        let mut base = Table::new();
        for layer in layers {
            merge_layer(&mut base, toml::from_str(layer).unwrap());
        }
        base
    }

    fn strings(table: &Table, section: &str, key: &str) -> Vec<String> {
        table[section][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn scalars_override_and_tables_merge() {
        let t = merged(&[
            "[model]\nmain = \"a\"\nfast = \"f\"",
            "[model]\nmain = \"b\"",
        ]);
        assert_eq!(t["model"]["main"].as_str(), Some("b"));
        assert_eq!(t["model"]["fast"].as_str(), Some("f"));
    }

    #[test]
    fn rule_lists_union_and_other_arrays_override() {
        let t = merged(&[
            "[permissions]\nallow = [\"Read\", \"Read\"]\n[model]\nfallbacks = [\"x\"]",
            "[permissions]\nallow = [\"Bash(ls)\", \"Read\"]\n[model]\nfallbacks = [\"y\"]",
        ]);
        assert_eq!(strings(&t, "permissions", "allow"), ["Read", "Bash(ls)"]);
        assert_eq!(strings(&t, "model", "fallbacks"), ["y"]);
    }

    #[test]
    fn sandbox_extra_writable_unions() {
        let t = merged(&[
            "[shell.sandbox]\nextra_writable = [\"a\"]\nenabled = true",
            "[shell.sandbox]\nextra_writable = [\"b\", \"a\"]",
        ]);
        let sandbox = &t["shell"]["sandbox"];
        let items: Vec<_> = sandbox["extra_writable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(items, ["a", "b"]);
        assert_eq!(sandbox["enabled"].as_bool(), Some(true));
    }

    #[test]
    fn hooks_concatenate() {
        let a = "[[hooks.Stop]]\ncommand = \"one\"";
        let t = merged(&[a, "[[hooks.Stop]]\ncommand = \"two\"", a]);
        let commands: Vec<_> = t["hooks"]["Stop"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["command"].as_str().unwrap())
            .collect();
        assert_eq!(commands, ["one", "two", "one"]);
    }

    #[test]
    fn decision_rules_concatenate() {
        let user = "[[decisions.rules]]\nquestion = \"user\"";
        let t = merged(&[user, "[[decisions.rules]]\nquestion = \"project\""]);
        let questions: Vec<_> = t["decisions"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|rule| rule["question"].as_str().unwrap())
            .collect();
        assert_eq!(questions, ["user", "project"]);
    }

    #[test]
    fn checks_merge_by_id_in_place() {
        let t = merged(&[
            "[[verification.checks]]\nid = \"a\"\ncommand = \"1\"\n[[verification.checks]]\nid = \"b\"\ncommand = \"2\"",
            "[[verification.checks]]\nid = \"a\"\ncommand = \"3\"\n[[verification.checks]]\nid = \"c\"\ncommand = \"4\"",
        ]);
        let checks = t["verification"]["checks"].as_array().unwrap();
        let pairs: Vec<_> = checks
            .iter()
            .map(|c| (c["id"].as_str().unwrap(), c["command"].as_str().unwrap()))
            .collect();
        assert_eq!(pairs, [("a", "3"), ("b", "2"), ("c", "4")]);
    }

    #[test]
    fn servers_are_replaced_whole() {
        let t = merged(&[
            "[mcp.servers.fs]\ncommand = \"npx\"\nargs = [\"-y\"]\n[mcp.servers.git]\ncommand = \"uvx\"",
            "[mcp.servers.fs]\nurl = \"http://localhost:1\"",
        ]);
        let fs = t["mcp"]["servers"]["fs"].as_table().unwrap();
        assert_eq!(fs.len(), 1);
        assert_eq!(fs["url"].as_str(), Some("http://localhost:1"));
        assert!(t["mcp"]["servers"].get("git").is_some());
    }

    #[test]
    fn nested_builds_a_path() {
        let t = nested(&["provider", "base_url"], Value::String("u".into()));
        assert_eq!(t["provider"]["base_url"].as_str(), Some("u"));
        assert!(nested(&[], Value::Integer(1)).is_empty());
    }
}
