//! Permission rules: `Tool` or `Tool(specifier)`.

mod command;
mod domain;
mod grammar;
mod matching;
mod path_pattern;
mod rule;

pub use rule::{Rule, RuleKind};

pub(crate) use domain::url_host;
