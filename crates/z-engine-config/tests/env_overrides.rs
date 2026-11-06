//! Environment overrides form the highest layer and are passed explicitly,
//! so these tests never touch the process environment.

#[allow(dead_code)]
mod support;

use support::{fixture, write};
use z_engine_config::{EnvOverrides, LayerScope, ProviderKind, load_with_env, project_local_file};

#[test]
fn env_overrides_beat_every_file() {
    let f = fixture();
    let local = "schema = 2\n[model]\nmain = \"local\"\n[provider]\nkind = \"openai_chat\"\n";
    write(&project_local_file(&f.project), local);
    let env = EnvOverrides {
        model: Some("env".into()),
        base_url: Some("http://env/v1/".into()),
        provider: Some("anthropic".into()),
        shell: Some("/bin/zsh".into()),
    };
    let loaded = load_with_env(&f.paths, Some(&f.project), &env);
    let settings = &loaded.settings;
    assert_eq!(settings.model.main, "env");
    assert_eq!(settings.provider.base_url, "http://env/v1");
    assert_eq!(settings.provider.kind, ProviderKind::Anthropic);
    assert_eq!(settings.shell.path.as_deref(), Some("/bin/zsh"));
    let env_layer = loaded.layers.last().unwrap();
    assert_eq!(env_layer.scope, LayerScope::Env);
    assert!(env_layer.exists && env_layer.error.is_none());
}

#[test]
fn an_invalid_env_override_is_reported_and_files_still_apply() {
    let f = fixture();
    write(
        &f.paths.user_settings_file,
        "schema = 2\n[model]\nmain = \"user\"\n",
    );
    let env = EnvOverrides {
        provider: Some("carrier-pigeon".into()),
        ..EnvOverrides::default()
    };
    let loaded = load_with_env(&f.paths, Some(&f.project), &env);
    assert_eq!(loaded.settings.model.main, "user");
    let error = loaded.layers.last().and_then(|layer| layer.error.clone());
    assert!(error.is_some_and(|e| e.contains("carrier-pigeon")));
    let skipped = loaded
        .warnings
        .iter()
        .any(|w| w.contains("environment overrides were skipped"));
    assert!(skipped, "{:?}", loaded.warnings);
}

#[test]
fn without_a_project_only_user_and_env_layers_load() {
    let f = fixture();
    write(
        &project_local_file(&f.project),
        "schema = 2\n[model]\nmain = \"local\"\n",
    );
    let env = EnvOverrides::default();
    let loaded = load_with_env(&f.paths, None, &env);
    assert_eq!(
        loaded.settings.model.main,
        z_engine_config::settings::DEFAULT_MODEL
    );
    let scopes: Vec<_> = loaded.layers.iter().map(|layer| layer.scope).collect();
    assert_eq!(
        scopes,
        [LayerScope::Default, LayerScope::User, LayerScope::Env]
    );
}
