//! One file per decision use, named after its feature id without the
//! `decisions_` prefix. A use implements `DecisionUse` and is listed in
//! `USES` (`decisions/registry.rs`).

pub(super) mod check_select;
pub(super) mod compaction;
pub(super) mod completion_check;
pub(super) mod custom_rules;
mod digest;
mod guidance;
pub(super) mod hints;
pub(super) mod inbox_priority;
pub(super) mod loop_guard;
pub(super) mod memory_suggest;
pub(super) mod output_trim;
pub(super) mod pet_mood;
pub(super) mod plan_suggest;
pub(super) mod prefetch;
pub(super) mod question_check;
mod relevance;
pub(super) mod review_suggest;
pub(super) mod risk;
pub(super) mod routing;
#[cfg(test)]
mod scripted;
pub(super) mod search_rank;
pub(super) mod secret_screen;
pub(super) mod session_context;
pub(super) mod task_view;
pub(super) mod user_signal;
