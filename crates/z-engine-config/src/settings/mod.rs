//! The v2 settings schema, one file per TOML section. TOML keys and the
//! GUI's JSON both use snake_case field names.

mod agents;
mod compat;
mod context;
mod decisions;
mod decisions_loop_guard;
mod decisions_prefetch;
mod decisions_routing;
mod decisions_rules;
mod decisions_task_view;
mod experimental;
mod hooks;
mod lenient;
mod lsp;
mod mcp;
mod model;
mod normalize;
mod permissions;
mod provider;
mod root;
mod shell;
mod ui;
mod unknown_keys;
mod verification;
mod web;
mod work;

pub use agents::AgentSettings;
pub use compat::CompatSettings;
pub use context::{ContextSettings, MAX_COMPACT_AT_PERCENT, MIN_COMPACT_AT_PERCENT};
pub use decisions::{
    CalibrationEntry, DECISIONS_MAX_BATCH, DECISIONS_MAX_LEN, DECISIONS_TIMEOUT_MS,
    DEFAULT_DECISIONS_CHECKPOINT, DEFAULT_DECISIONS_ENDPOINT, DecisionRuntime, DecisionSettings,
    DecisionSidecarSettings,
};
pub use decisions_loop_guard::{DecisionLoopGuardSettings, LOOP_GUARD_MAX_REMINDERS};
pub use decisions_prefetch::{DecisionPrefetchSettings, PREFETCH_MAX_FILES, PREFETCH_MAX_TOKENS};
pub use decisions_routing::DecisionRoutingSettings;
pub use decisions_rules::{DecisionRule, RULE_ACTIONS, RULE_EVENTS, RULE_MAX_OPTIONS};
pub use decisions_task_view::{DecisionTaskViewSettings, TASK_VIEW_KEEP_RECENT};
pub use hooks::{HOOK_EVENTS, HookConfig, is_hook_event};
pub use lsp::{LspServerConfig, LspSettings};
pub use mcp::{McpServerConfig, McpSettings};
pub use model::{
    DEFAULT_MODEL, MAX_OUTPUT_TOKENS, MIN_OUTPUT_TOKENS, ModelSettings, PricingOverride,
};
pub use permissions::{PermissionSettings, RuleKind};
pub use provider::{DEFAULT_BASE_URL, ProviderKind, ProviderSettings};
pub use root::Settings;
pub use shell::{SandboxSettings, ShellSettings};
pub use ui::{
    CompanionLevel, DEFAULT_PET_NAME, MAX_PET_NAME_CHARS, PetLook, PetSettings, TaskReportView,
    UiSettings,
};
pub use verification::{CheckConfig, MAX_CONTINUATIONS, VerificationSettings};
pub use web::{SearchBackend, WebSettings};
pub use work::{
    DEFAULT_SERIAL_PATHS, MergePolicy, WORK_MAX_ATTEMPTS, WORK_MAX_PARALLEL, WORK_PACKET_TOKENS,
    WorkSettings,
};

use decisions::normalize_decisions;
use experimental::normalize_experimental;
pub(crate) use normalize::normalize;
pub(crate) use unknown_keys::unknown_keys;
use work::normalize_work;
