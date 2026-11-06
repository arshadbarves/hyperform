//! The editing core behind `Edit` and `MultiEdit`: in-memory replacement
//! with a fallback ladder, change snippets, and guarded file access.

mod access;
mod engine;
mod flow;
mod ladder;
mod report;
mod snippet;

pub(crate) use access::{ensure_fresh, load_text, save};
pub(crate) use engine::EditSpec;
pub(crate) use flow::{edit_file, preview_edits};
pub(crate) use report::describe;
