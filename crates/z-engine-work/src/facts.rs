//! What the engine knows that the pure kernel cannot find out by itself.

use std::collections::BTreeSet;

use z_engine_protocol::work::EvidenceRef;
use z_engine_protocol::{CheckKind, TaskId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// Epoch milliseconds stamped on accepted records.
    pub now: u64,
    /// Fingerprint of the acting agent's current tree (its worktree, or the
    /// project). `None` when unknown: no check evidence counts as fresh.
    pub fingerprint: Option<String>,
    /// Check record ids the engine judged stale.
    pub stale_records: BTreeSet<String>,
    /// Root-relative files changed since the plan last reconciled.
    pub changed_files: BTreeSet<String>,
    /// Root-relative files deleted since the plan last reconciled.
    pub deleted_files: BTreeSet<String>,
    /// Epoch ms the changed and deleted files are counted from. A task
    /// done at or after it may have made those changes itself, so they do
    /// not make its evidence stale.
    pub changed_since: Option<u64>,
    /// The task the acting agent was dispatched for, if any.
    pub bound_task: Option<TaskId>,
    /// The proposal derives from web content (untrusted research).
    pub untrusted_input: bool,
    /// The session is in plan mode: nothing is dispatched in parallel.
    pub planning: bool,
    pub limits: Limits,
    /// What the engine observed of the acting helper's run.
    pub run: RunFacts,
    /// Set: every accepted revision ends with the deterministic audit,
    /// recorded only when its findings change.
    pub audit: Option<AuditScope>,
}

/// What the deterministic audit needs from the engine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditScope {
    /// Check kinds the project can run; `None` skips the method check.
    pub check_kinds: Option<Vec<CheckKind>>,
    /// `RISKY_PATHS` plus `[work] sensitive_paths`.
    pub risky_paths: Vec<String>,
}

/// The `[work]` settings the kernel enforces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Parallel helpers at once (`Settings::max_parallel_tasks`).
    pub max_parallel: u32,
    /// Starts per task before only the user can retry it; `None` leaves
    /// the budget unchecked.
    pub max_attempts: Option<u32>,
    /// `[work] serial_paths`.
    pub serial_paths: Vec<String>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_parallel: 1,
            max_attempts: None,
            serial_paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunFacts {
    /// Files the helper's worktree diff changed; `None` when unknown.
    pub files_changed: Option<Vec<String>>,
    /// Check records of the helper's run (`EvidenceRef::Check`).
    pub checks: Vec<EvidenceRef>,
}

impl Facts {
    pub fn at(now: u64) -> Self {
        Self {
            now,
            ..Self::default()
        }
    }

    pub fn with_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.fingerprint = Some(fingerprint.into());
        self
    }
}
