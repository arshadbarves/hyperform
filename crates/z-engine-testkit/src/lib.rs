//! Dev-only test support: a scripted model client, fixture repositories,
//! and an event recorder. Not a product surface.

pub mod events;
pub mod model;
pub mod repo;

pub use events::{DEFAULT_WAIT, EventRecorder};
pub use model::{Script, ScriptedModel, usage};
pub use repo::FixtureRepo;
