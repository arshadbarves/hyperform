//! Language-server results rendered as readable text for the model.

mod diagnostics;
mod locations;
mod rename;
mod symbols;

pub use diagnostics::format_diagnostics;
pub use locations::{format_calls, format_locations, format_workspace_symbols};
pub use rename::format_rename_plan;
pub use symbols::{format_hover, format_symbols};

/// Longest list rendered; the rest is summarized.
const MAX_ITEMS: usize = 200;

fn more(total: usize) -> Option<String> {
    (total > MAX_ITEMS).then(|| format!("... and {} more", total - MAX_ITEMS))
}
