//! Text shaping shared by the tools: truncation with spill, line
//! numbering, and diffs.

mod diff;
mod numbering;
mod rank;
mod trim;
mod truncate;

pub(crate) use diff::{cap_preview, creation_diff, diff_stats, text_diff, unified_diff};
pub(crate) use numbering::{numbered, push_numbered};
pub(crate) use rank::{fit_entries, grep_entries, rank_before_cut};
pub(crate) use trim::trim_output;
pub(crate) use truncate::{truncate_output, truncate_parts, truncate_with};
