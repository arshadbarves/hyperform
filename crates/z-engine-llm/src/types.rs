//! Provider-neutral request and streaming event types.

use serde_json::Value;
use z_engine_protocol::{Effort, Message, Usage};

use crate::error::LlmError;

/// One system prompt segment. `cache` places a cache breakpoint after it.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemBlock {
    pub text: String,
    pub cache: bool,
}

impl SystemBlock {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cache: false,
        }
    }

    pub fn cached(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cache: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool input (an object schema).
    pub input_schema: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ToolChoice {
    #[default]
    Auto,
    None,
    /// The model must call some tool.
    Any,
    /// The model must call this tool.
    Tool(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThinkingConfig {
    pub effort: Effort,
    /// Explicit budget; adapters derive one from `effort` when absent.
    pub budget_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelRequest {
    pub model: String,
    pub system: Vec<SystemBlock>,
    pub messages: Vec<Message>,
    /// Indices into `messages` whose last block carries a cache breakpoint.
    pub cache_breakpoints: Vec<usize>,
    pub tools: Vec<ToolSpec>,
    /// Place a cache breakpoint after the last tool definition.
    pub cache_tools: bool,
    pub tool_choice: ToolChoice,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub thinking: Option<ThinkingConfig>,
    pub stop_sequences: Vec<String>,
    /// Stable conversation key for providers that bind sessions (e.g. Zen).
    pub session_key: Option<String>,
}

impl ModelRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            system: Vec::new(),
            messages,
            cache_breakpoints: Vec::new(),
            tools: Vec::new(),
            cache_tools: false,
            tool_choice: ToolChoice::Auto,
            max_tokens: 4_096,
            temperature: None,
            thinking: None,
            stop_sequences: Vec::new(),
            session_key: None,
        }
    }

    pub fn with_system(mut self, system: Vec<SystemBlock>) -> Self {
        self.system = system;
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Concatenated system text (for adapters with a single system slot).
    pub fn system_text(&self) -> String {
        self.system
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
    Refusal,
    Other(String),
}

/// Streaming events, normalized across providers.
///
/// Tool calls arrive as `ToolUseStart` (id and name), zero or more
/// `ToolUseDelta` fragments of JSON input, then `ToolUseEnd`. `Usage`
/// carries the *cumulative* usage of the response so far; keep the last.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ThinkingSignature(String),
    RedactedThinking(String),
    ToolUseStart {
        index: usize,
        id: String,
        name: String,
    },
    ToolUseDelta {
        index: usize,
        partial_json: String,
    },
    ToolUseEnd {
        index: usize,
    },
    Usage(Usage),
    Stop(StopReason),
    /// Informational: the client is backing off before retrying.
    Retrying {
        attempt: u32,
        delay_ms: u64,
        reason: String,
    },
}

/// Receiver of streaming events. The stream ends after `Stop` or a single
/// terminal error; dropping it cancels the underlying request.
pub type ModelStream = tokio::sync::mpsc::Receiver<Result<ModelEvent, LlmError>>;
