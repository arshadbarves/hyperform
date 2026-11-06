//! Server processes owned by this crate (the documented exception to "only
//! host touches the OS"): spawning and teardown, and the stderr tail.

mod spawn;
mod stderr;

pub(crate) use spawn::{ServerCommand, ServerProcess, find_program, spawn_server};
pub(crate) use stderr::StderrLog;
