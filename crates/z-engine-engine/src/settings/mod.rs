//! Per-session settings: trust-aware loading, instruction docs and
//! glob-scoped rules, the model client, the permission policy, the command
//! sandbox, and model roles.

mod client;
mod effective;
mod instructions;
pub(crate) mod models;
mod policy;
mod rules;
pub(crate) mod sandbox;

pub(crate) use client::session_client;
pub(crate) use effective::{SessionSettings, load_session_settings, trust_gated};
pub(crate) use instructions::nested_docs;
pub(crate) use policy::build_policy;
pub(crate) use rules::scoped_rules;
