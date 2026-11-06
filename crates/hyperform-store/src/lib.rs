//! Session persistence v2: per-session logs, subagent transcripts,
//! artifacts, metadata, import of v1 transcripts, and the opt-in decision
//! dataset.
//!
//! Under the sessions directory: `<id>/log.jsonl` (records),
//! `<id>/meta.json` (listing cache), `<id>/agents/<agent>.jsonl` (subagent
//! transcripts), `<id>/artifacts/`, and untouched v1 `<ULID>.jsonl` files.
//! Work plans live apart, under the plans directory (`PlanStore`).

mod append;
mod artifacts;
mod dataset;
mod durable;
mod error;
mod heal;
mod layout;
mod legacy;
mod listing;
mod log;
mod meta;
mod plans;
mod read;
mod record;
mod replay;
mod store;

pub use artifacts::Artifacts;
pub use dataset::DecisionDataset;
pub use error::StoreError;
pub use legacy::import_v1;
pub use log::{AgentLog, SessionLog};
pub use meta::SessionMeta;
pub use plans::{LoadedPlan, PLAN_SCHEMA, PlanLog, PlanStore};
pub use read::{ReadOutcome, read_records};
pub use record::{LogRecord, SESSION_SCHEMA};
pub use replay::{ReplayState, replay};
pub use store::{LoadedSession, NewSession, SessionStore};
