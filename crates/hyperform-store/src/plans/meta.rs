//! `meta.json` of a plan: the `PlanSummary` the plan picker lists, cached
//! from the log and rewritten atomically after each revision.

use std::fs;
use std::path::Path;

use hyperform_protocol::work::PlanSummary;
use serde::{Deserialize, Serialize};

use crate::durable::write_atomic;
use crate::error::StoreError;

/// Version of the plan files this crate writes.
pub const PLAN_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanMeta {
    schema: u32,
    #[serde(flatten)]
    summary: PlanSummary,
}

pub(super) fn write(path: &Path, summary: &PlanSummary) -> Result<(), StoreError> {
    let meta = PlanMeta {
        schema: PLAN_SCHEMA,
        summary: summary.clone(),
    };
    let mut bytes =
        serde_json::to_vec_pretty(&meta).map_err(|error| StoreError::json(path, 0, &error))?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

/// A schema newer than this crate writes is `Invalid`, so callers never
/// overwrite metadata they do not understand.
pub(super) fn read(path: &Path) -> Result<PlanSummary, StoreError> {
    let bytes = fs::read(path).map_err(|error| StoreError::open(path, error))?;
    let meta: PlanMeta = serde_json::from_slice(&bytes)
        .map_err(|error| StoreError::json(path, error.line(), &error))?;
    if meta.schema > PLAN_SCHEMA {
        return Err(StoreError::Invalid(format!(
            "{} has schema {}, newer than supported {PLAN_SCHEMA}",
            path.display(),
            meta.schema
        )));
    }
    Ok(meta.summary)
}
