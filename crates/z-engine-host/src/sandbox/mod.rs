//! Optional OS sandbox for agent shell commands: writes are confined to a
//! [`SandboxProfile`] and the network can be cut off. The sandbox wraps a
//! [`ShellSpec`](crate::ShellSpec), so any run or background shell using
//! the wrapped spec is sandboxed.

mod backend;
mod bubblewrap;
mod profile;
mod seatbelt;
mod wrap;

pub use backend::{SandboxBackend, detect};
pub use profile::{PROTECTED, SandboxProfile, TOOL_CACHES, tool_caches};
pub use wrap::{is_sandbox_denial, sandbox_shell, wrap};
