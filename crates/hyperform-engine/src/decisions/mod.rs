//! The decision layer in the engine: a per-session `DecisionService`
//! (rebuilt on reload), the engine-wide sidecar and native model (with its
//! download from Settings), the seams where uses run,
//! and one file per use. The model proposes, the engine decides: features
//! that are off, a model that fails and low confidence all keep today's
//! behavior.

mod build;
mod context;
mod hub;
mod memo;
pub(crate) mod native;
mod registry;
pub(crate) mod seams;
mod service;
mod sidecar;
mod suggest;
mod uses;

pub(crate) use build::{build_service, connect};
pub(crate) use hub::DecisionHub;
pub(crate) use sidecar::Sidecars;
pub(crate) use suggest::resolve_suggestion;
pub(crate) use uses::routing::{route_new_task, route_subagent, routed_effort, routed_model};
pub(crate) use uses::session_context::preload_tools;
pub(crate) use uses::task_view::{apply_task_view_for, include_full_history};
