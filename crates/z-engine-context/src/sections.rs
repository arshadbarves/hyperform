//! System prompt sections handed to the engine.

/// One system prompt segment. The engine maps sections to provider system
/// blocks in order and places the cache breakpoint after the last section
/// whose `cacheable` flag is set (see [`crate::system_breakpoint`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSection {
    pub text: String,
    /// Byte-stable across the requests of a session, so it may sit in the
    /// cached prefix.
    pub cacheable: bool,
}

impl PromptSection {
    pub fn cached(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cacheable: true,
        }
    }

    pub fn uncached(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cacheable: false,
        }
    }
}
