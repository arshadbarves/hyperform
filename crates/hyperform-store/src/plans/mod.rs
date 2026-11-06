//! Work plans of each project: `<plans dir>/<project key>/<plan id>/`
//! with `events.jsonl` and `meta.json`.

mod meta;
mod store;

pub use meta::PLAN_SCHEMA;
pub use store::{LoadedPlan, PlanLog, PlanStore};

#[cfg(test)]
mod tests;
