//! What the application shell hands the engine: locations, the event sink,
//! an optional model-client factory, and environment overrides.

use std::fmt;
use std::sync::Arc;

use z_engine_config::{EnvOverrides, Paths, Settings};
use z_engine_llm::ModelClient;
use z_engine_protocol::EventEnvelope;

use crate::error::EngineError;

/// Receives every event of every session, already sequenced per session.
/// Called synchronously from engine tasks: it must return quickly.
pub type EventSink = Arc<dyn Fn(EventEnvelope) + Send + Sync>;

/// Builds the model client for a session's settings. Tests inject a
/// scripted model; `None` in [`EngineOptions`] uses the real providers.
pub trait ClientFactory: Send + Sync + fmt::Debug {
    fn build(
        &self,
        settings: &Settings,
        api_key: Option<String>,
    ) -> Result<Arc<dyn ModelClient>, EngineError>;
}

pub struct EngineOptions {
    pub paths: Paths,
    pub event_sink: EventSink,
    /// `None` builds real provider clients from settings and credentials.
    pub client_factory: Option<Arc<dyn ClientFactory>>,
    /// Environment overrides applied on top of every settings layer.
    pub env: EnvOverrides,
}

impl fmt::Debug for EngineOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineOptions")
            .field("paths", &self.paths)
            .field("client_factory", &self.client_factory)
            .field("env", &self.env)
            .finish_non_exhaustive()
    }
}

/// Output of [`crate::Engine::export_session`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// A readable transcript with tool calls summarized.
    Markdown,
    /// Every log record, as a JSON array.
    Json,
}
