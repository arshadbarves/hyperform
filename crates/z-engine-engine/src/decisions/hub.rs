//! `DecisionHub`: a session's decision state. The service is swapped on
//! reload; the trace and the shadow-task budget live as long as the session.

use std::sync::{Arc, RwLock};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use z_engine_decisions::DecisionTrace;

use super::memo::UseMemo;
use super::service::DecisionService;
use super::uses::compaction::CompactionMemory;
use super::uses::loop_guard::LoopWatch;
use super::uses::routing::RouteMemory;
use super::uses::secret_screen::SecretLedger;
use super::uses::session_context::ToolPreload;
use super::uses::task_view::TaskViewMemory;
use crate::sync::{read, write};

/// Shadow decisions in flight at once; more are skipped, never queued, so
/// shadow mode cannot build a backlog behind a slow model.
const SHADOW_SLOTS: usize = 4;

#[derive(Debug)]
pub(crate) struct DecisionHub {
    service: RwLock<Arc<DecisionService>>,
    trace: Arc<DecisionTrace>,
    shadow_slots: Arc<Semaphore>,
    memo: UseMemo,
    routes: RouteMemory,
    loops: LoopWatch,
    compaction: CompactionMemory,
    task_view: TaskViewMemory,
    preload: ToolPreload,
    secrets: SecretLedger,
}

impl DecisionHub {
    pub(crate) fn new(service: DecisionService) -> Self {
        Self {
            service: RwLock::new(Arc::new(service)),
            trace: Arc::default(),
            shadow_slots: Arc::new(Semaphore::new(SHADOW_SLOTS)),
            memo: UseMemo::default(),
            routes: RouteMemory::default(),
            loops: LoopWatch::default(),
            compaction: CompactionMemory::default(),
            task_view: TaskViewMemory::default(),
            preload: ToolPreload::default(),
            secrets: SecretLedger::default(),
        }
    }

    /// Verdicts and model-made clears of `decisions_compaction`.
    pub(crate) fn compaction(&self) -> &CompactionMemory {
        &self.compaction
    }

    /// The planned view and saved exchanges of `decisions_task_view`.
    pub(crate) fn task_view(&self) -> &TaskViewMemory {
        &self.task_view
    }

    /// MCP tools `decisions_session_context` picked for the first request.
    pub(crate) fn preload(&self) -> &ToolPreload {
        &self.preload
    }

    /// Screened results and settled values of `decisions_secret_screen`.
    pub(crate) fn secrets(&self) -> &SecretLedger {
        &self.secrets
    }

    /// What uses already offered or acted on in this session.
    pub(crate) fn memo(&self) -> &UseMemo {
        &self.memo
    }

    /// The current task and its route (`decisions_routing`).
    pub(crate) fn routes(&self) -> &RouteMemory {
        &self.routes
    }

    /// Each agent's recent steps (`decisions_loop_guard`).
    pub(crate) fn loops(&self) -> &LoopWatch {
        &self.loops
    }

    pub(crate) fn service(&self) -> Arc<DecisionService> {
        Arc::clone(&read(&self.service))
    }

    pub(crate) fn replace(&self, service: DecisionService) {
        *write(&self.service) = Arc::new(service);
    }

    pub(crate) fn trace(&self) -> &Arc<DecisionTrace> {
        &self.trace
    }

    /// `None` when every shadow slot is busy: skip this shadow decision.
    pub(crate) fn shadow_slot(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.shadow_slots).try_acquire_owned().ok()
    }
}
