//! Model transport: the [`ModelClient`] seam, provider adapters, retries,
//! fallback models, the model catalog and cost accounting.

pub mod accumulate;
pub mod catalog;
pub mod client;
pub mod cost;
pub mod error;
pub mod fallback;
pub mod provider;
pub mod types;

mod anthropic;
mod copilot;
mod openai;
mod opencode;
mod responses;
mod retry;
mod sse;
mod transport;
mod turns;
mod wire;
mod zen;

pub use accumulate::{AssistantTurn, MalformedToolUse, ResponseAccumulator};
pub use catalog::{ModelCatalog, ModelInfo, Pricing, builtin_pricing, fetch_models_dev};
pub use client::{ModelClient, collect};
pub use copilot::{
    DeviceCode, LoginPoll, build_copilot_client, copilot_login_url, is_copilot_url, poll_login,
    start_login,
};
pub use cost::{cost_usd, price_for};
pub use error::LlmError;
pub use fallback::FallbackClient;
pub use provider::{
    ANTHROPIC_BASE_URL, ProviderConfig, ProviderKind, ReasoningStyle, build_client,
    catalog_provider, detect_kind, provider_label,
};
pub use types::{
    ModelEvent, ModelRequest, ModelStream, StopReason, SystemBlock, ThinkingConfig, ToolChoice,
    ToolSpec,
};
