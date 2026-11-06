//! Search: file globbing, content grep (ripgrep or builtin), and the fuzzy
//! file index behind `@`-mentions.

mod files;
mod fuzzy;
mod glob;
mod grep;
mod walk;

pub use files::FileIndex;
pub use glob::{GlobResult, glob};
pub use grep::{GrepEngine, GrepMode, GrepQuery, GrepResult, grep, grep_with_engine, rg_available};

pub(crate) use walk::walker;
