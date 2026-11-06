//! Layered settings, v1 import, credentials, workspace trust, and
//! discovery of user-authored extensions and instruction files.
//!
//! Settings layer as defaults < user `settings.toml` < project
//! `.hyperform/settings.toml` < `.hyperform/settings.local.toml` < environment.
//! Loading never fails: a broken layer is skipped and reported. Public
//! types derive `ts_rs::TS`; `cargo test -p hyperform-config` regenerates the
//! TypeScript mirror under `ui/src/lib/protocol/config/`.

pub mod credentials;
mod document;
mod error;
pub mod extensions;
pub mod features;
mod files;
mod gitignore;
pub mod instructions;
mod loader;
mod merge;
mod migrate;
mod paths;
pub mod settings;
mod trust;
pub mod writer;

pub use credentials::{
    Credentials, KeyStatus, credential_key, resolve_api_key, resolve_api_key_with,
    resolve_search_key, resolve_search_key_with, search_key_bucket,
};
pub use document::SCHEMA_VERSION;
pub use error::ConfigError;
pub use extensions::{
    AgentDef, CommandDef, ExtensionError, ExtensionScope, ExtensionSource, Extensions,
    OutputStyleDef, RuleDef, SkillDef, discover_extensions, load_skill_body, parse_agent,
    parse_command,
};
pub use features::{FEATURES, FeatureGroup, FeatureId, FeatureMode, FeatureSpec, FeatureStage};
pub use instructions::{
    InstructionFile, InstructionScope, discover_instructions, nested_instructions,
};
pub use loader::{EnvOverrides, LayerInfo, LayerScope, LoadedSettings, load, load_with_env};
pub use migrate::{MigrationOutcome, import_v1_file};
pub use paths::{
    APP_DIR, CONFIG_DIR_ENV, DATA_DIR_ENV, LEGACY_CONFIG_FILE, LOCAL_SETTINGS_FILE, PROJECT_DIR,
    Paths, SETTINGS_FILE, legacy_project_config_file, project_dir, project_local_file,
    project_settings_file,
};
pub use settings::{
    AgentSettings, CalibrationEntry, CheckConfig, CompanionLevel, CompatSettings, ContextSettings,
    DecisionRule, DecisionRuntime, DecisionSettings, DecisionSidecarSettings, HOOK_EVENTS,
    HookConfig, LspServerConfig, LspSettings, McpServerConfig, McpSettings, MergePolicy,
    ModelSettings, PermissionSettings, PricingOverride, ProviderKind, ProviderSettings, RuleKind,
    SandboxSettings, SearchBackend, Settings, ShellSettings, TaskReportView, UiSettings,
    VerificationSettings, WebSettings, WorkSettings, is_hook_event,
};
pub use trust::TrustStore;
pub use writer::{
    add_permission_rule, add_to_array, remove_from_array, remove_mcp_server,
    remove_permission_rule, remove_value, set_hooks, set_mcp_server, set_value,
};
