//! `auth.json` stays v1-compatible; API keys resolve env-first per host.

#[allow(dead_code)]
mod support;

use std::collections::HashMap;

use support::{fixture, read, write};
use z_engine_config::credentials::{ANTHROPIC, BRAVE, OPENCODE, OPENROUTER};
use z_engine_config::{
    ConfigError, Credentials, KeyStatus, SearchBackend, credential_key, resolve_api_key_with,
    resolve_search_key_with,
};

/// Exactly what v1's settings screen wrote.
const V1_AUTH: &str = r#"{
  "opencode": {
    "type": "api",
    "key": "zen-secret"
  },
  "openrouter": {
    "type": "api",
    "key": "sk-or-secret-abcd"
  }
}
"#;

fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let vars: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |name| vars.get(name).cloned()
}

#[test]
fn reads_the_v1_file() {
    let f = fixture();
    write(&f.paths.auth_file, V1_AUTH);
    let creds = Credentials::load(&f.paths.auth_file).unwrap();
    assert_eq!(creds.get(OPENROUTER).as_deref(), Some("sk-or-secret-abcd"));
    assert_eq!(creds.get(OPENCODE).as_deref(), Some("zen-secret"));
    let status = creds.status(OPENROUTER);
    assert_eq!(
        status,
        KeyStatus {
            has_key: true,
            hint: Some("abcd".into())
        }
    );
    assert!(!creds.status(ANTHROPIC).has_key);
}

#[test]
fn saving_keeps_the_v1_format_and_unknown_entries() {
    let f = fixture();
    let with_future = V1_AUTH.replacen(
        '{',
        "{\n  \"future\": { \"type\": \"oauth\", \"token\": \"t\" },",
        1,
    );
    write(&f.paths.auth_file, &with_future);
    let mut creds = Credentials::load(&f.paths.auth_file).unwrap();
    creds.set(ANTHROPIC, Some("  sk-ant  ".into()));
    creds.set(OPENCODE, None);
    creds.set(BRAVE, Some(" ".into()));
    creds.save(&f.paths.auth_file).unwrap();

    let saved: serde_json::Value = serde_json::from_str(&read(&f.paths.auth_file)).unwrap();
    assert_eq!(
        saved["anthropic"],
        serde_json::json!({ "type": "api", "key": "sk-ant" })
    );
    assert_eq!(saved["openrouter"]["key"], "sk-or-secret-abcd");
    assert_eq!(saved["future"]["token"], "t");
    assert!(saved.get("opencode").is_none() && saved.get("brave").is_none());
    assert_eq!(Credentials::load(&f.paths.auth_file).unwrap(), creds);
}

#[cfg(unix)]
#[test]
fn saved_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    write(&f.paths.auth_file, V1_AUTH);
    Credentials::load(&f.paths.auth_file)
        .unwrap()
        .save(&f.paths.auth_file)
        .unwrap();
    let mode = std::fs::metadata(&f.paths.auth_file)
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn missing_file_is_empty_and_malformed_file_is_an_error() {
    let f = fixture();
    assert_eq!(
        Credentials::load(&f.paths.auth_file).unwrap(),
        Credentials::default()
    );
    write(&f.paths.auth_file, "{ not json");
    let error = Credentials::load(&f.paths.auth_file);
    assert!(matches!(error, Err(ConfigError::Parse { .. })));
}

#[test]
fn api_keys_resolve_env_first_for_the_matching_host() {
    let mut creds = Credentials::default();
    creds.set(OPENROUTER, Some("stored-or".into()));
    creds.set(ANTHROPIC, Some("stored-ant".into()));
    let openrouter = "https://openrouter.ai/api/v1";
    let anthropic = "https://api.anthropic.com/v1";

    let all = env(&[
        ("ZENGINE_API_KEY", "zengine"),
        ("OPENROUTER_API_KEY", "env-or"),
        ("ANTHROPIC_API_KEY", "env-ant"),
    ]);
    assert_eq!(
        resolve_api_key_with(&creds, anthropic, &all).as_deref(),
        Some("zengine")
    );

    let providers = env(&[
        ("OPENROUTER_API_KEY", "env-or"),
        ("ANTHROPIC_API_KEY", "env-ant"),
    ]);
    assert_eq!(
        resolve_api_key_with(&creds, openrouter, &providers).as_deref(),
        Some("env-or")
    );
    assert_eq!(
        resolve_api_key_with(&creds, anthropic, &providers).as_deref(),
        Some("env-ant")
    );

    let none = env(&[("OPENAI_API_KEY", "env-oai"), ("ZENGINE_API_KEY", "  ")]);
    assert_eq!(
        resolve_api_key_with(&creds, openrouter, &none).as_deref(),
        Some("stored-or")
    );
    assert_eq!(
        resolve_api_key_with(&creds, "https://api.openai.com/v1", &none).as_deref(),
        Some("env-oai")
    );
    let zen = env(&[("OPENCODE_API_KEY", "env-zen")]);
    assert_eq!(
        resolve_api_key_with(&creds, "https://opencode.ai/zen/v1", &zen).as_deref(),
        Some("env-zen")
    );

    let local = "http://localhost:11434/v1";
    assert_eq!(credential_key(local), "custom:localhost:11434");
    assert_eq!(
        resolve_api_key_with(&creds, local, &providers),
        None,
        "keys never cross hosts"
    );
    creds.set(&credential_key(local), Some("local-key".into()));
    assert_eq!(
        resolve_api_key_with(&creds, local, &providers).as_deref(),
        Some("local-key")
    );
}

#[test]
fn search_keys_use_their_own_buckets() {
    let mut creds = Credentials::default();
    creds.set(BRAVE, Some("stored-brave".into()));
    let quiet = env(&[]);
    assert_eq!(
        resolve_search_key_with(&creds, SearchBackend::Brave, &quiet).as_deref(),
        Some("stored-brave")
    );
    let tavily = env(&[("TAVILY_API_KEY", "env-tavily")]);
    assert_eq!(
        resolve_search_key_with(&creds, SearchBackend::Tavily, &tavily).as_deref(),
        Some("env-tavily")
    );
    assert_eq!(
        resolve_search_key_with(&creds, SearchBackend::Exa, &quiet),
        None
    );
    assert_eq!(
        resolve_search_key_with(&creds, SearchBackend::Searxng, &tavily),
        None
    );
}
