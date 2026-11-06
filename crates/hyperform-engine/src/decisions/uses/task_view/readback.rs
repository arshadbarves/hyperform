//! Readbacks: the agent reading a file a set-aside exchange was saved to
//! means the view dropped something the task needed. Each one is a trace
//! record with outcome `readback`, the measure that keeps
//! `decisions_task_view` honest.

use std::path::Path;

use hyperform_context::compaction::call_paths;
use hyperform_decisions::{AbstainReason, Answer};
use hyperform_tools::names::READ;

use super::ask::NEEDED;
use crate::batch::ToolCall;
use crate::decisions::context::UseContext;

pub(super) fn observe(cx: &UseContext, call: &ToolCall) {
    if call.name != READ {
        return;
    }
    let memory = cx.core.decisions.task_view();
    let paths = call_paths(&call.input);
    let Some(path) = paths.iter().find(|path| memory.is_spilled(path)) else {
        return;
    };
    let observed = Answer {
        abstain: None,
        cached: true,
        ..Answer::abstained(NEEDED, AbstainReason::Rules, "engine")
    };
    let name = Path::new(path)
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    cx.record(cx.record_of(&observed, &name).outcome("readback"));
}
