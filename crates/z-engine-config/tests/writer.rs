//! GUI edits of one layer file: creation, round trips, preserved keys,
//! refused invalid writes, seeding from untouched v1 files, and the local
//! `.gitignore`.

#[allow(dead_code)]
mod support;

use support::{fixture, read, write};
use toml::Value;
use z_engine_config::{
    ConfigError, HookConfig, McpServerConfig, RuleKind, add_permission_rule, add_to_array,
    legacy_project_config_file, project_local_file, project_settings_file, remove_from_array,
    remove_mcp_server, remove_permission_rule, remove_value, set_hooks, set_mcp_server, set_value,
};

#[test]
fn a_missing_file_is_created_with_the_schema() {
    let f = fixture();
    let file = project_local_file(&f.project);
    set_value(&file, &["model", "main"], Value::String("m".into())).unwrap();
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    assert_eq!(doc["schema"].as_integer(), Some(2));
    assert_eq!(f.load().settings.model.main, "m");
}

#[test]
fn edits_keep_unrelated_keys() {
    let f = fixture();
    let file = f.paths.user_settings_file.clone();
    let original = "schema = 2\nexperimental = { flag = true }\n[model]\nmain = \"m\"\n[mcp.servers.fs]\ncommand = \"npx\"\n[[hooks.Stop]]\ncommand = \"done\"\n";
    write(&file, original);
    set_value(&file, &["context", "repo_map"], Value::Boolean(false)).unwrap();
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    assert_eq!(doc["experimental"]["flag"].as_bool(), Some(true));
    assert_eq!(doc["model"]["main"].as_str(), Some("m"));
    assert_eq!(doc["mcp"]["servers"]["fs"]["command"].as_str(), Some("npx"));
    assert_eq!(doc["hooks"]["Stop"][0]["command"].as_str(), Some("done"));
    assert_eq!(doc["context"]["repo_map"].as_bool(), Some(false));

    remove_value(&file, &["context", "repo_map"]).unwrap();
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    assert!(doc.get("context").is_none(), "empty tables are pruned");
}

#[test]
fn arrays_deduplicate_and_keep_emptied_lists() {
    let f = fixture();
    let file = f.paths.user_settings_file.clone();
    add_to_array(&file, &["model", "fallbacks"], "a").unwrap();
    add_to_array(&file, &["model", "fallbacks"], "b").unwrap();
    add_to_array(&file, &["model", "fallbacks"], "a").unwrap();
    assert_eq!(f.load().settings.model.fallbacks, ["a", "b"]);
    remove_from_array(&file, &["model", "fallbacks"], "a").unwrap();
    remove_from_array(&file, &["model", "fallbacks"], "b").unwrap();
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    assert_eq!(doc["model"]["fallbacks"].as_array().map(Vec::len), Some(0));
}

#[test]
fn permission_rules_round_trip() {
    let f = fixture();
    let file = project_local_file(&f.project);
    add_permission_rule(&file, RuleKind::Allow, " Bash(cargo test:*) ").unwrap();
    add_permission_rule(&file, RuleKind::Allow, "Bash(cargo test:*)").unwrap();
    add_permission_rule(&file, RuleKind::Deny, "Read(./.env)").unwrap();
    let permissions = f.load().settings.permissions;
    assert_eq!(permissions.allow, ["Bash(cargo test:*)"]);
    assert_eq!(permissions.deny, ["Read(./.env)"]);
    remove_permission_rule(&file, RuleKind::Allow, "Bash(cargo test:*)").unwrap();
    assert!(f.load().settings.permissions.allow.is_empty());
    assert!(add_permission_rule(&file, RuleKind::Ask, "  ").is_err());
}

#[test]
fn mcp_servers_round_trip_and_invalid_servers_are_refused() {
    let f = fixture();
    let file = f.paths.user_settings_file.clone();
    let stdio = McpServerConfig {
        command: Some("npx".into()),
        args: vec!["-y".into(), "fs".into()],
        ..McpServerConfig::default()
    };
    set_mcp_server(&file, "fs", &stdio).unwrap();
    assert_eq!(f.load().settings.mcp.servers["fs"], stdio);
    let doc: toml::Table = toml::from_str(&read(&file)).unwrap();
    let written = doc["mcp"]["servers"]["fs"].as_table().unwrap();
    let mut keys: Vec<_> = written.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["args", "command", "enabled", "timeout_secs"],
        "empty lists and maps are omitted"
    );

    let http = McpServerConfig {
        url: Some("http://localhost:3000/mcp".into()),
        ..McpServerConfig::default()
    };
    set_mcp_server(&file, "fs", &http).unwrap();
    assert_eq!(f.load().settings.mcp.servers["fs"], http);

    let before = read(&file);
    let both = McpServerConfig {
        command: Some("npx".into()),
        ..http
    };
    assert!(matches!(
        set_mcp_server(&file, "fs", &both),
        Err(ConfigError::Invalid { .. })
    ));
    assert_eq!(read(&file), before);

    remove_mcp_server(&file, "fs").unwrap();
    remove_mcp_server(&file, "missing").unwrap();
    assert!(f.load().settings.mcp.servers.is_empty());
}

#[test]
fn hooks_round_trip() {
    let f = fixture();
    let file = project_local_file(&f.project);
    let hook = HookConfig {
        matcher: Some("Edit|Write".into()),
        command: "./fmt.sh".into(),
        timeout_secs: 30,
    };
    set_hooks(&file, "PostToolUse", std::slice::from_ref(&hook)).unwrap();
    assert_eq!(f.load().settings.hooks["PostToolUse"], [hook]);
    set_hooks(&file, "PostToolUse", &[]).unwrap();
    assert!(f.load().settings.hooks.is_empty());
    let unknown = set_hooks(&file, "AfterLunch", &[HookConfig::default()]);
    assert!(matches!(unknown, Err(ConfigError::KeyPath { .. })));
}

#[test]
fn invalid_edits_are_refused_without_touching_the_file() {
    let f = fixture();
    let file = f.paths.user_settings_file.clone();
    write(&file, "schema = 2\n[model]\nmain = \"m\"\n");
    let before = read(&file);
    let bad_value = set_value(&file, &["model", "effort"], Value::String("extreme".into()));
    assert!(
        matches!(bad_value, Err(ConfigError::Invalid { .. })),
        "{bad_value:?}"
    );
    let through_scalar = set_value(&file, &["model", "main", "x"], Value::Integer(1));
    assert!(matches!(through_scalar, Err(ConfigError::KeyPath { .. })));
    assert!(matches!(
        set_value(&file, &["schema"], Value::Integer(9)),
        Err(ConfigError::KeyPath { .. })
    ));
    assert!(matches!(
        set_value(&file, &[], Value::Integer(9)),
        Err(ConfigError::KeyPath { .. })
    ));
    assert_eq!(read(&file), before);

    write(&file, "schema = 2\n[model\n");
    let unparseable = set_value(&file, &["model", "main"], Value::String("x".into()));
    assert!(matches!(unparseable, Err(ConfigError::Parse { .. })));
    assert_eq!(read(&file), "schema = 2\n[model\n");
}

#[test]
fn removals_from_a_missing_file_do_not_create_it() {
    let f = fixture();
    let file = project_local_file(&f.project);
    remove_value(&file, &["model", "main"]).unwrap();
    remove_from_array(&file, &["permissions", "allow"], "Read").unwrap();
    remove_mcp_server(&file, "fs").unwrap();
    assert!(!file.exists());
}

const V1_PROJECT: &str = "model = \"old\"\n[permissions]\nallow = [\"ls*\"]\n";

#[test]
fn the_first_project_edit_seeds_from_the_untouched_v1_file() {
    let f = fixture();
    let legacy = legacy_project_config_file(&f.project);
    write(&legacy, V1_PROJECT);
    let file = project_settings_file(&f.project);
    add_permission_rule(&file, RuleKind::Allow, "Read").unwrap();
    assert_eq!(read(&legacy), V1_PROJECT);
    assert!(read(&file).contains("schema = 2"));
    let loaded = f.load();
    assert_eq!(loaded.settings.model.main, "old");
    assert_eq!(loaded.settings.permissions.allow, ["Bash(ls:*)", "Read"]);
    assert!(loaded.layers[2].note.is_none(), "the v2 file is used now");
    assert_eq!(loaded.layers[2].path.as_deref(), Some(file.as_path()));
}

#[test]
fn removing_an_imported_value_writes_the_seeded_file() {
    let f = fixture();
    let legacy = legacy_project_config_file(&f.project);
    write(&legacy, V1_PROJECT);
    let file = project_settings_file(&f.project);
    remove_value(&file, &["model", "not_there"]).unwrap();
    assert!(!file.exists(), "nothing changed, nothing written");
    remove_permission_rule(&file, RuleKind::Allow, "Bash(ls:*)").unwrap();
    assert!(f.load().settings.permissions.allow.is_empty());
    assert_eq!(read(&legacy), V1_PROJECT);
}

#[test]
fn v1_config_files_are_read_only() {
    let f = fixture();
    let legacy = legacy_project_config_file(&f.project);
    write(&legacy, V1_PROJECT);
    let edit = set_value(&legacy, &["model", "main"], Value::String("x".into()));
    assert!(matches!(edit, Err(ConfigError::Invalid { .. })));
    assert_eq!(read(&legacy), V1_PROJECT);
}

#[test]
fn local_writes_keep_the_gitignore_current() {
    let f = fixture();
    let ignore = f.project.join(".z-engine/.gitignore");
    write(&ignore, "# mine\nnotes/");
    add_permission_rule(&project_local_file(&f.project), RuleKind::Allow, "Read").unwrap();
    assert_eq!(
        read(&ignore),
        "# mine\nnotes/\nsettings.local.toml\nworktrees/\n"
    );
    add_permission_rule(&project_local_file(&f.project), RuleKind::Deny, "Write").unwrap();
    assert_eq!(
        read(&ignore),
        "# mine\nnotes/\nsettings.local.toml\nworktrees/\n"
    );

    let fresh = fixture();
    let fresh_ignore = fresh.project.join(".z-engine/.gitignore");
    let shared = project_settings_file(&fresh.project);
    set_value(&shared, &["model", "main"], Value::String("m".into())).unwrap();
    assert!(!fresh_ignore.exists(), "shared settings are committed");
    let local = project_local_file(&fresh.project);
    set_value(
        &local,
        &["ui", "output_style"],
        Value::String("terse".into()),
    )
    .unwrap();
    assert_eq!(read(&fresh_ignore), "settings.local.toml\nworktrees/\n");
}
