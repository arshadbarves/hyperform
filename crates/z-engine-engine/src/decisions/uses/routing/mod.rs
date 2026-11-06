//! Per-task routing (`decisions_routing`): when a task starts, a complexity
//! answer picks the reasoning effort (only while the user leaves effort on
//! auto) and, with `[decisions.routing] allow_model_switch`, the main or
//! the fast model. The choice holds for the whole task so the prompt cache
//! survives follow-ups; a subagent whose model is `inherit` is routed once
//! at launch. Each applied route is announced (`RouteChosen`).

mod decide;
mod memory;
mod signals;
mod start;
#[cfg(test)]
mod tests;

pub(crate) use decide::ROUTING;
pub(crate) use memory::{RouteMemory, routed_effort, routed_model};
pub(crate) use start::{route_new_task, route_subagent};
