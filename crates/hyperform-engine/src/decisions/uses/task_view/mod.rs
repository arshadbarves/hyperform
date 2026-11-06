//! Task-scoped history (`decisions_task_view`): at a task boundary, earlier
//! exchanges the new task does not need are set aside (saved to artifacts,
//! listed in an index) until the next boundary, so each request carries
//! less. Hard keeps, the cost check and every failure leave history whole.

mod apply;
mod ask;
mod history;
mod memory;
mod plan;
mod readback;
#[cfg(test)]
mod tests;

pub(crate) use apply::{apply_task_view_for, include_full_history};
pub(crate) use memory::TaskViewMemory;
pub(crate) use plan::TASK_VIEW;
