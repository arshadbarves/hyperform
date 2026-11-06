//! Bounded, read-only project discovery: which ecosystems a workspace
//! uses, where their roots are, and which checks their manifests suggest;
//! plus configured checks layered on top.

mod collector;
mod discover;
mod ecosystems;
mod gitignore;
mod jsonc;
mod merge;
mod read;
mod rel;
mod scripts;
mod types;
mod walk;

pub use discover::discover;
pub use merge::merge_configured;
pub use types::{DiscoveryOptions, ProjectProfile, ProjectRoot};
