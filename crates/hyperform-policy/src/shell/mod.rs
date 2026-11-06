//! Static analysis of shell command lines: quote-aware lexing into simple
//! commands, read-only classification, acceptEdits filesystem commands, and
//! rule suggestions. Nothing is executed and nothing is read from disk.

mod analysis;
mod fields;
mod fs_command;
mod git;
mod lexer;
mod location;
mod operands;
mod read_only;
mod sed;
mod sed_script;
mod suggest;
mod syntax;
mod wrappers;

pub use analysis::{ShellAnalysis, analyze};
pub use fs_command::is_common_fs_command;
pub use read_only::is_read_only;
pub use suggest::suggest_prefix;

pub(crate) use analysis::{Parsed, parse};
pub(crate) use location::{Location, locate};
pub(crate) use operands::operands;
pub(crate) use read_only::{command_is_read_only, segment_is_read_only};
pub(crate) use suggest::prefix_for;
pub(crate) use syntax::{Segment, Word};
pub(crate) use wrappers::executed_commands;
