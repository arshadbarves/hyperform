//! Background shells: long-running commands the model polls and kills.

mod job;
mod ring;
mod shells;
mod types;

pub use shells::BackgroundShells;
pub use types::{BackgroundSpec, JobEvent, JobEventSink, JobRead, JobSnapshot};
