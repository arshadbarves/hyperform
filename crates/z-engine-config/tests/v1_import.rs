//! v1 `config.toml` files are import sources only: `Paths::ensure` writes a
//! new `settings.toml` from the user file, loading converts project files in
//! memory, and no v1 file is ever modified.

#[allow(dead_code)]
mod support;

use std::path::Path;

use support::{fixture, read, write};
use z_engine_config::{
    MigrationOutcome, import_v1_file, legacy_project_config_file, project_settings_file,
};

/// The v1 default user config after a few edits from the v1 settings screen.
const V1_USER: &str = r#"# z-engine configuration
# OpenRouter API key lives in auth.json next to this file (Settings → General).
model = "openrouter/free"
base_url = "https://openrouter.ai/api/v1/"
# Bounded task continuations (0 disables; maximum 10). New sessions only.
max_task_continuations = 3
max_context_tokens = 64000
review = true
shell_path = "C:\\Program Files\\Git\\bin\\bash.exe"

[hooks]
session_start = "echo hello"
turn_completed = "cargo fmt"

[permissions]
allow = ["cargo test*", "git status"]

[mcp.servers.fs]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem"]

[cost.overrides]
"my/model" = { usd_per_mtok_input = 1.0, usd_per_mtok_output = 2.0 }
"#;

/// What v1's project writer produced.
const V1_PROJECT: &str = r#"# z-engine project configuration
# bash prefix rules under [permissions.allow] skip approval for this project.
model = "z/a"
compact_at_percent = 85
task_report_view = "compact"

[permissions]
allow = ["npm run*"]
"#;

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn ensure_imports_the_v1_user_config_into_settings_toml() {
    let f = fixture();
    write(&f.paths.user_config_file, V1_USER);
    let outcome = f.paths.ensure().unwrap();
    assert!(outcome.migrated);
    assert_eq!(
        outcome.source.as_deref(),
        Some(f.paths.user_config_file.as_path())
    );
    assert!(
        outcome.notes.iter().any(|note| note.contains("`review`")),
        "{:?}",
        outcome.notes
    );
    assert_eq!(read(&f.paths.user_config_file), V1_USER);
    let imported = read(&f.paths.user_settings_file);
    assert!(
        imported.starts_with("# Imported from ") && imported.contains("schema = 2"),
        "{imported}"
    );
    assert_eq!(
        names_in(&f.paths.config_dir),
        ["config.toml", "settings.toml"]
    );

    let loaded = f.load();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let s = &loaded.settings;
    assert_eq!(s.model.main, "openrouter/free");
    assert_eq!(s.model.context_window, Some(64_000));
    assert_eq!(s.provider.base_url, "https://openrouter.ai/api/v1");
    assert_eq!(s.verification.max_continuations, 3);
    assert_eq!(
        s.shell.path.as_deref(),
        Some("C:\\Program Files\\Git\\bin\\bash.exe")
    );
    assert_eq!(s.hooks["SessionStart"][0].command, "echo hello");
    assert_eq!(s.hooks["Stop"][0].command, "cargo fmt");
    assert_eq!(
        s.permissions.allow,
        ["Bash(cargo test:*)", "Bash(git status)"]
    );
    assert_eq!(s.mcp.servers["fs"].args.len(), 2);
    assert_eq!(
        (s.pricing["my/model"].input, s.pricing["my/model"].output),
        (1.0, 2.0)
    );

    assert_eq!(f.paths.ensure().unwrap(), MigrationOutcome::default());
    assert_eq!(read(&f.paths.user_config_file), V1_USER);
}

#[test]
fn an_unparseable_v1_user_config_is_reported_and_left_alone() {
    let f = fixture();
    write(&f.paths.user_config_file, "model = \n");
    let outcome = f.paths.ensure().unwrap();
    assert!(!outcome.migrated && !outcome.notes.is_empty());
    assert!(!f.paths.user_settings_file.exists());
    assert_eq!(read(&f.paths.user_config_file), "model = \n");
    let user = &f.load().layers[1];
    assert!(user.error.is_some());
    assert_eq!(
        user.path.as_deref(),
        Some(f.paths.user_config_file.as_path())
    );
}

#[test]
fn the_user_v1_file_is_read_in_memory_when_ensure_did_not_run() {
    let f = fixture();
    write(&f.paths.user_config_file, "model = \"legacy\"\n");
    let loaded = f.load();
    assert_eq!(loaded.settings.model.main, "legacy");
    assert!(loaded.layers[1].note.is_some());
    assert!(!f.paths.user_settings_file.exists());
}

#[test]
fn project_v1_files_are_imported_in_memory_only() {
    let f = fixture();
    let legacy = legacy_project_config_file(&f.project);
    write(&legacy, V1_PROJECT);
    let loaded = f.load();
    let s = &loaded.settings;
    assert_eq!(s.model.main, "z/a");
    assert_eq!(s.context.compact_at_percent, 85);
    assert_eq!(s.permissions.allow, ["Bash(npm run:*)"]);
    let project = &loaded.layers[2];
    assert_eq!(project.path.as_deref(), Some(legacy.as_path()));
    assert!(project.exists && project.error.is_none());
    let note = project.note.clone().unwrap_or_default();
    assert!(
        note.contains("in memory") && note.contains("settings.toml"),
        "{note}"
    );
    assert!(loaded.warnings.contains(&note));

    assert_eq!(read(&legacy), V1_PROJECT);
    assert_eq!(names_in(&f.project.join(".z-engine")), ["config.toml"]);
    assert_eq!(f.load().settings, loaded.settings);
}

#[test]
fn a_v2_project_file_takes_the_place_of_the_v1_file() {
    let f = fixture();
    write(&legacy_project_config_file(&f.project), V1_PROJECT);
    let settings = project_settings_file(&f.project);
    write(&settings, "schema = 2\n[model]\nmain = \"v2\"\n");
    let loaded = f.load();
    assert_eq!(loaded.settings.model.main, "v2");
    assert_eq!(loaded.settings.context.compact_at_percent, 92);
    assert_eq!(loaded.layers[2].path.as_deref(), Some(settings.as_path()));
    assert!(loaded.layers[2].note.is_none() && loaded.warnings.is_empty());
}

#[test]
fn import_never_replaces_existing_settings() {
    let f = fixture();
    let legacy = legacy_project_config_file(&f.project);
    let target = project_settings_file(&f.project);
    write(&legacy, V1_PROJECT);
    write(&target, "schema = 2\n");
    assert_eq!(
        import_v1_file(&legacy, &target).unwrap(),
        MigrationOutcome::default()
    );
    assert_eq!(read(&target), "schema = 2\n");
    std::fs::remove_file(&target).unwrap();
    let outcome = import_v1_file(&legacy, &target).unwrap();
    assert!(outcome.migrated && read(&target).contains("[permissions]"));
    assert_eq!(read(&legacy), V1_PROJECT);
    let missing = f.project.join("nothing/config.toml");
    let absent = import_v1_file(&missing, &f.project.join("nothing/settings.toml")).unwrap();
    assert_eq!(absent, MigrationOutcome::default());
}
