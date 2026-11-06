//! File layer precedence, list merge rules, broken layers, and clamping,
//! through the public loader.

#[allow(dead_code)]
mod support;

use support::{fixture, write};
use z_engine_config::{LayerScope, Settings, project_local_file, project_settings_file};
use z_engine_protocol::{CheckKind, Effort, PermissionMode, VerificationMode};

#[test]
fn defaults_apply_when_no_file_exists() {
    let f = fixture();
    let loaded = f.load();
    assert_eq!(loaded.settings, Settings::default());
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let scopes: Vec<_> = loaded
        .layers
        .iter()
        .map(|layer| (layer.scope, layer.exists))
        .collect();
    assert_eq!(
        scopes,
        [
            (LayerScope::Default, true),
            (LayerScope::User, false),
            (LayerScope::Project, false),
            (LayerScope::ProjectLocal, false),
            (LayerScope::Env, false),
        ]
    );
}

#[test]
fn later_layers_override_scalars() {
    let f = fixture();
    let user = "schema = 2\n[model]\nmain = \"user\"\nfast = \"cheap\"\n[provider]\nbase_url = \"http://user/v1\"\n";
    write(&f.paths.user_settings_file, user);
    let project = "schema = 2\n[model]\nmain = \"project\"\n";
    write(&project_settings_file(&f.project), project);
    let local = "schema = 2\n[model]\nmain = \"local\"\n";
    write(&project_local_file(&f.project), local);
    let settings = f.load().settings;
    assert_eq!(settings.model.main, "local");
    assert_eq!(settings.model.fast_model(), "cheap");
    assert_eq!(settings.model.review_model(), "local");
    assert_eq!(settings.provider.base_url, "http://user/v1");
}

#[test]
fn rule_lists_union_and_hooks_concatenate() {
    let f = fixture();
    let user = r#"schema = 2
[permissions]
allow = ["Read", "Bash(ls)"]
[shell]
env_passthrough = ["PATH"]
[[hooks.Stop]]
command = "user-stop"
"#;
    let project = r#"schema = 2
[permissions]
allow = ["Bash(ls)", "Edit"]
deny = ["Bash(rm:*)"]
[shell]
env_passthrough = ["PATH", "HOME"]
[[hooks.Stop]]
command = "project-stop"
[[hooks.PreToolUse]]
matcher = "Bash"
command = "guard"
"#;
    write(&f.paths.user_settings_file, user);
    write(&project_settings_file(&f.project), project);
    write(
        &project_local_file(&f.project),
        "schema = 2\n[permissions]\nallow = [\"Edit\", \"Write\"]\n",
    );
    let settings = f.load().settings;
    assert_eq!(
        settings.permissions.allow,
        ["Read", "Bash(ls)", "Edit", "Write"]
    );
    assert_eq!(settings.permissions.deny, ["Bash(rm:*)"]);
    assert_eq!(settings.shell.env_passthrough, ["PATH", "HOME"]);
    let stops: Vec<_> = settings.hooks["Stop"]
        .iter()
        .map(|hook| hook.command.as_str())
        .collect();
    assert_eq!(stops, ["user-stop", "project-stop"]);
    let guard = &settings.hooks["PreToolUse"][0];
    assert_eq!(
        (guard.matcher.as_deref(), guard.timeout_secs),
        (Some("Bash"), 60)
    );
}

#[test]
fn checks_merge_by_id_and_servers_replace_by_name() {
    let f = fixture();
    let user = r#"schema = 2
[[verification.checks]]
id = "test"
command = "cargo test"
kind = "tests"
[[verification.checks]]
id = "lint"
command = "cargo clippy"
kind = "lint"
[mcp.servers.fs]
command = "npx"
args = ["-y", "fs"]
[mcp.servers.git]
command = "uvx"
[lsp.servers.rust]
command = "rust-analyzer"
extensions = ["rs"]
"#;
    let project = r#"schema = 2
[[verification.checks]]
id = "test"
command = "cargo nextest run"
timeout_secs = 1200
[mcp.servers.fs]
url = "http://localhost:3000/mcp"
[lsp.servers.rust]
command = "ra-multiplex"
"#;
    write(&f.paths.user_settings_file, user);
    write(&project_settings_file(&f.project), project);
    let settings = f.load().settings;
    let checks: Vec<_> = settings
        .verification
        .checks
        .iter()
        .map(|c| (c.id.as_str(), c.command.as_str()))
        .collect();
    assert_eq!(
        checks,
        [("test", "cargo nextest run"), ("lint", "cargo clippy")]
    );
    let test = &settings.verification.checks[0];
    assert_eq!(
        (test.kind, test.timeout_secs, test.label.as_str()),
        (CheckKind::Custom, 1200, "test")
    );
    assert_eq!(settings.verification.checks[1].kind, CheckKind::Lint);
    let fs = &settings.mcp.servers["fs"];
    assert_eq!(
        (fs.command.as_deref(), fs.url.as_deref()),
        (None, Some("http://localhost:3000/mcp"))
    );
    assert!(fs.args.is_empty());
    assert!(settings.mcp.servers.contains_key("git"));
    assert!(settings.lsp.servers["rust"].extensions.is_empty());
}

#[test]
fn broken_layers_are_skipped_and_reported() {
    let f = fixture();
    write(
        &f.paths.user_settings_file,
        "schema = 2\n[model]\nmain = \"user\"\n",
    );
    let project = project_settings_file(&f.project);
    write(&project, "schema = 2\n[model\nmain = ");
    let local = project_local_file(&f.project);
    write(
        &local,
        "schema = 2\n[model]\nmain = \"local\"\neffort = \"extreme\"\n",
    );
    let loaded = f.load();
    assert_eq!(loaded.settings.model.main, "user");
    for (index, path) in [(2, &project), (3, &local)] {
        let layer = &loaded.layers[index];
        assert!(layer.exists && layer.error.is_some(), "{layer:?}");
        let shown = path.display().to_string();
        assert!(
            loaded
                .warnings
                .iter()
                .any(|w| w.starts_with(&shown) && w.contains("skipped"))
        );
    }
    let local_error = loaded.layers[3].error.as_deref().unwrap_or_default();
    assert!(
        local_error.contains("extreme") && local_error.contains("model.effort"),
        "{local_error}"
    );
}

#[test]
fn values_are_clamped_and_synonyms_accepted() {
    let f = fixture();
    let user = r#"schema = 2
[model]
max_output_tokens = 1000000
effort = "xhigh"
[context]
compact_at_percent = 20
[verification]
mode = "strict"
max_continuations = 50
[permissions]
mode = "bypassPermissions"
"#;
    write(&f.paths.user_settings_file, user);
    let loaded = f.load();
    let settings = &loaded.settings;
    assert_eq!(settings.model.max_output_tokens, 200_000);
    assert_eq!(settings.context.compact_at_percent, 50);
    assert_eq!(settings.verification.max_continuations, 10);
    assert_eq!(settings.model.effort, Some(Effort::Max));
    assert_eq!(settings.verification.mode, VerificationMode::Strict);
    assert_eq!(settings.permissions.mode, PermissionMode::Bypass);
    assert!(!settings.permissions.planning);
    assert_eq!(loaded.warnings.len(), 3, "{:?}", loaded.warnings);
}

#[test]
fn a_layer_with_the_old_plan_mode_starts_chats_planning_at_ask() {
    let f = fixture();
    write(
        &f.paths.user_settings_file,
        "schema = 2\n[permissions]\nmode = \"acceptEdits\"\n",
    );
    write(
        &project_settings_file(&f.project),
        "schema = 2\n[permissions]\nmode = \"plan\"\n",
    );
    let loaded = f.load();
    let permissions = &loaded.settings.permissions;
    assert!(permissions.planning);
    assert_eq!(permissions.mode, PermissionMode::Default);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
}

#[test]
fn questionable_entries_produce_warnings() {
    let f = fixture();
    let user = r#"schema = 2
colour = "blue"
[model]
mian = "typo"
[[hooks.BeforeCoffee]]
command = "brew"
[mcp.servers.both]
command = "npx"
url = "http://localhost:1"
[mcp.servers.neither]
args = ["x"]
[web]
search_backend = "searxng"
"#;
    write(&f.paths.user_settings_file, user);
    let loaded = f.load();
    assert!(loaded.settings.mcp.servers.is_empty());
    for expected in [
        "`colour`",
        "`model.mian`",
        "hooks.BeforeCoffee",
        "mcp.servers.both",
        "mcp.servers.neither",
        "search_url",
    ] {
        assert!(
            loaded.warnings.iter().any(|w| w.contains(expected)),
            "{expected}: {:?}",
            loaded.warnings
        );
    }
}

#[test]
fn newer_schema_loads_known_keys_with_a_warning() {
    let f = fixture();
    write(
        &f.paths.user_settings_file,
        "schema = 3\n[model]\nmain = \"future\"\n",
    );
    let loaded = f.load();
    assert_eq!(loaded.settings.model.main, "future");
    assert!(loaded.warnings.iter().any(|w| w.contains("schema 3")));
}
