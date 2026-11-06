//! Content search with ripgrep-compatible output from two engines.

mod builtin;
mod engine;
mod format;
mod plan;
mod query;
mod ripgrep;

pub use engine::{grep, grep_with_engine, rg_available};
pub use query::{GrepEngine, GrepMode, GrepQuery, GrepResult};
