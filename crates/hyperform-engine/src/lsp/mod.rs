//! Language servers of one session: configured servers merged over the
//! presets, the `LSP` tool's queries, and error diagnostics reported back
//! right after the model writes a covered file.

mod after_write;
mod hub;
mod query;
mod worker;

pub(crate) use after_write::error_notes;
pub(crate) use hub::LspHub;
pub(crate) use query::answer;
pub(crate) use worker::LspWorker;
