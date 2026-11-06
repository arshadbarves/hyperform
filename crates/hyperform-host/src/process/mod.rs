//! Processes: shell selection, environment policy, one-shot runs with a
//! persistent working directory, background jobs, and tree kills.

mod background;
mod capture;
mod env;
mod kill;
mod lines;
mod run;
mod script;
mod shell;
#[cfg(windows)]
mod shell_windows;
mod spawn;
mod tree;

pub use background::{
    BackgroundShells, BackgroundSpec, JobEvent, JobEventSink, JobRead, JobSnapshot,
};
pub use env::EnvPolicy;
pub use kill::kill_tree;
pub use run::{DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_RUN_TIMEOUT, OutputSink, RunOutput, RunSpec, run};
pub use shell::{ShellKind, ShellSpec, resolve_shell};

#[cfg(windows)]
pub(crate) use kill::CREATE_NO_WINDOW;
