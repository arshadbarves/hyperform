//! The complete settings tree deserialized from the merged layers.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    AgentSettings, CompatSettings, ContextSettings, DecisionSettings, HookConfig, LspSettings,
    McpSettings, ModelSettings, PermissionSettings, PricingOverride, ProviderSettings,
    ShellSettings, UiSettings, VerificationSettings, WebSettings, WorkSettings,
};
use crate::features::FeatureMode;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct Settings {
    pub model: ModelSettings,
    pub provider: ProviderSettings,
    pub permissions: PermissionSettings,
    pub context: ContextSettings,
    pub agents: AgentSettings,
    pub verification: VerificationSettings,
    /// Hook event (see `HOOK_EVENTS`) to hooks run in order; lists
    /// concatenate across layers.
    pub hooks: BTreeMap<String, Vec<HookConfig>>,
    pub mcp: McpSettings,
    pub lsp: LspSettings,
    pub web: WebSettings,
    pub shell: ShellSettings,
    pub ui: UiSettings,
    pub compat: CompatSettings,
    /// Model id to pricing, overriding the catalog.
    pub pricing: BTreeMap<String, PricingOverride>,
    /// Feature id (see `FEATURES`) to its mode; read it with
    /// `Settings::feature`.
    pub experimental: BTreeMap<String, FeatureMode>,
    pub decisions: DecisionSettings,
    pub work: WorkSettings,
}
