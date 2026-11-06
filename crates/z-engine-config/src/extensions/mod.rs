//! User-authored extensions: markdown files with YAML frontmatter defining
//! agents, commands, skills, rules, and output styles.

mod agent;
mod catalog;
mod command;
mod discover;
mod frontmatter;
mod output_style;
mod rule;
mod skill;

pub use agent::{AgentDef, parse_agent};
pub use catalog::{ExtensionError, ExtensionScope, ExtensionSource, Extensions};
pub use command::{COMMAND_DESCRIPTION_LIMIT, CommandDef, parse_command};
pub use discover::{EXTENSION_FILE_LIMIT, discover_extensions};
pub use output_style::OutputStyleDef;
pub use rule::RuleDef;
pub use skill::{SKILL_FILE, SkillDef, load_skill_body};
