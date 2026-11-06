//! Tree-sitter repository map: per-file definition outlines, ranked by
//! cross-file references and rendered within a character budget.

mod ecmascript;
mod go;
mod language;
mod map;
mod python;
mod rank;
mod render;
mod rust;
mod symbol;

pub use language::supported_extension;
pub use map::{SourceFile, repo_map, repo_map_ranked};
