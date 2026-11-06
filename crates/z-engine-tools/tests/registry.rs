//! `ToolRegistry`: built-in order, registration, filtering, and specs.

use serde_json::json;
use z_engine_tools::{ToolRegistry, mcp_tool, names};

fn registry_with_mcp() -> ToolRegistry {
    let mut registry = ToolRegistry::builtin(Vec::new());
    registry.register(mcp_tool(
        "github",
        "create_issue",
        "Create",
        json!({}),
        false,
    ));
    registry.register(mcp_tool("github", "get_issue", "Get", json!({}), true));
    registry.register(mcp_tool("docs", "search", "Search", json!({}), true));
    registry
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

#[test]
fn builtin_registers_every_tool_in_order() {
    let registry = ToolRegistry::builtin(Vec::new());
    assert_eq!(registry.names(), names::ALL.to_vec());
    assert_eq!(registry.len(), 27);
    assert!(registry.get("Read").is_some() && registry.get("read").is_none());
}

#[test]
fn register_replaces_a_tool_of_the_same_name_in_place() {
    let mut registry = registry_with_mcp();
    let before = registry
        .names()
        .iter()
        .position(|name| *name == "mcp__github__get_issue");
    registry.register(mcp_tool("github", "get_issue", "Newer", json!({}), true));
    let after = registry
        .names()
        .iter()
        .position(|name| *name == "mcp__github__get_issue");
    assert_eq!(before, after);
    assert_eq!(registry.len(), 30);
    assert_eq!(
        registry
            .get("mcp__github__get_issue")
            .unwrap()
            .description(),
        "Newer"
    );
}

#[test]
fn allow_lists_match_names_prefix_globs_and_rule_spellings() {
    let registry = registry_with_mcp();
    let allow = owned(&["Read", "Bash(git status:*)", "mcp__github__*"]);
    let filtered = registry.filtered(Some(allow.as_slice()), &[]);
    assert_eq!(
        filtered.names(),
        vec![
            "Read",
            "Bash",
            "mcp__github__create_issue",
            "mcp__github__get_issue"
        ]
    );
    let everything = owned(&["*"]);
    assert_eq!(
        registry.filtered(Some(everything.as_slice()), &[]).len(),
        registry.len()
    );
    assert!(
        registry
            .filtered(Some(owned(&[]).as_slice()), &[])
            .is_empty()
    );
}

#[test]
fn deny_lists_remove_tools_but_ignore_scoped_rules() {
    let registry = registry_with_mcp();
    let deny = owned(&["WebFetch", "mcp__docs__*", "Bash(rm:*)"]);
    let filtered = registry.filtered(None, &deny);
    let names = filtered.names();
    assert!(!names.contains(&"WebFetch") && !names.contains(&"mcp__docs__search"));
    assert!(
        names.contains(&"Bash"),
        "scoped deny rules gate calls, not tools"
    );
    assert_eq!(filtered.len(), registry.len() - 2);
}

#[test]
fn without_removes_named_tools_and_specs_follow_the_order() {
    let registry = ToolRegistry::builtin(Vec::new())
        .without([names::ASK_USER_QUESTION, names::EXIT_PLAN_MODE]);
    assert_eq!(registry.len(), 25);
    assert!(registry.get("AskUserQuestion").is_none());
    let specs = registry.specs();
    assert_eq!(specs.len(), 25);
    assert_eq!(specs[0].0, "Read");
    assert!(!specs[0].1.is_empty());
    assert_eq!(specs[0].2["type"], "object");
    let dropped = registry.without(vec!["Read".to_string()]);
    assert_eq!(dropped.names()[0], "Write");
    assert!(format!("{registry:?}").contains("Glob"));
}
