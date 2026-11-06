//! Plan health for the Plan tab's coverage line: counts and levels the
//! kernel computes from the plan, never a percentage.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HealthLevel {
    Good,
    #[default]
    Fair,
    Poor,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct PlanHealth {
    /// Active requirements, and those a planned task with criteria serves.
    pub requirements: u32,
    pub requirements_covered: u32,
    /// Planned tasks (not proposed, cancelled or superseded).
    pub tasks: u32,
    pub tasks_done: u32,
    /// Open leaf tasks without acceptance criteria.
    pub tasks_without_criteria: u32,
    pub blocked: u32,
    pub failed: u32,
    /// How well finished work is backed by evidence.
    pub evidence: HealthLevel,
    /// Done tasks whose evidence no longer holds (a criterion failed).
    pub evidence_stale: u32,
    /// Audit findings still open, and the latest reconciliation's findings.
    pub open_findings: u32,
    pub reconcile_findings: u32,
    /// Open tasks with risks but no rollback note.
    pub unmitigated_risks: u32,
}
