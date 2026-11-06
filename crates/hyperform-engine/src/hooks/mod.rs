//! User-configured shell hooks on agent lifecycle events.

mod event;
mod notify;
mod outcome;
mod runner;

pub(crate) use event::{HookEnv, HookEvent, HookInput};
pub(crate) use notify::notify;
pub(crate) use outcome::PermissionOverride;
pub(crate) use runner::run_hooks;
