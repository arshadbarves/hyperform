//! The places (`Seam`) the engine consults decision uses. Each dispatcher
//! returns at once when no registered use of its seam runs, and applies
//! only advice from `on` uses.

mod after_call;
mod ask_user;
mod attention;
mod check_select;
mod completion;
mod dispatch;
mod pressure;
mod relevance;
mod request;
mod route;
mod stop;
#[cfg(test)]
mod tests;
mod tool_gate;
mod turn_end;
mod turn_start;

pub(crate) use after_call::annotate_result;
pub(crate) use ask_user::review_questions;
#[cfg(test)]
pub(crate) use attention::score_attention_with;
pub(crate) use attention::{
    AttentionItem, AttentionKind, score_attention, watch_notices, worth_notifying,
};
pub(crate) use check_select::select_needed;
pub(crate) use completion::review_completion;
pub(crate) use pressure::{ClearAdvice, review_clears};
pub(crate) use relevance::rank_items;
pub(crate) use request::{Finding, ResultText, screen_request};
pub(crate) use route::{RouteAdvice, RouteTask, route_task};
pub(crate) use stop::review_stop;
pub(crate) use tool_gate::{escalate, review_call};
pub(crate) use turn_end::at_turn_end;
#[cfg(test)]
pub(crate) use turn_end::turn_end_work;
pub(crate) use turn_start::at_turn_start;
