//! All LLM prompt text lives here as markdown under `prompts/`. Each module
//! exposes one `pub const` per file via `include_str!`; logic crates never
//! inline prompt prose.

pub mod agents;
pub mod auxiliary;
pub mod commands;
pub mod decisions;
pub mod providers;
pub mod reminders;
pub mod system;
pub mod tools;
pub mod work;
