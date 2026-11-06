//! `PlanHub`: the engine's work plans. Every session that attaches a plan
//! shares one live `PlanHandle`, so each plan log has a single writer for
//! the whole process; a handle no session holds is dropped and read from
//! disk again when next opened.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use hyperform_host::project_key;
use hyperform_protocol::PlanId;
use hyperform_protocol::work::PlanSummary;
use hyperform_store::PlanStore;
use hyperform_work::{Facts, PlanDraft, Submission, create};

use super::plan::PlanHandle;
use crate::sync::lock;

#[derive(Debug, Clone)]
pub(crate) struct PlanHub {
    inner: Arc<HubInner>,
}

#[derive(Debug)]
struct HubInner {
    store: PlanStore,
    open: Mutex<HashMap<PlanId, Weak<PlanHandle>>>,
}

impl PlanHub {
    pub(crate) fn new(plans_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(HubInner {
                store: PlanStore::new(plans_dir),
                open: Mutex::default(),
            }),
        }
    }

    /// The project's plans, most recently updated first; an open plan's
    /// entry is its live state.
    pub(crate) fn list(&self, root: &Path) -> Result<Vec<PlanSummary>, String> {
        let key = project_key(root);
        let mut plans = self
            .inner
            .store
            .list(&key)
            .map_err(|error| error.to_string())?;
        for plan in &mut plans {
            if let Some(handle) = self.live(&plan.plan_id) {
                *plan = handle.summary();
            }
        }
        plans.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.plan_id.cmp(&a.plan_id))
        });
        Ok(plans)
    }

    /// The live handle of a plan of the project at `root`.
    pub(crate) fn open(&self, root: &Path, plan_id: &PlanId) -> Result<Arc<PlanHandle>, String> {
        let key = project_key(root);
        let mut open = lock(&self.inner.open);
        if let Some(handle) = open.get(plan_id).and_then(Weak::upgrade) {
            return if handle.key() == key {
                Ok(handle)
            } else {
                Err(format!("plan {plan_id} belongs to another project"))
            };
        }
        if !self.inner.store.exists(&key, plan_id) {
            return Err(format!("plan {plan_id} was not found in this project"));
        }
        let handle = Arc::new(PlanHandle::load(self.inner.store.clone(), key, plan_id)?);
        open.retain(|_, weak| weak.strong_count() > 0);
        open.insert(plan_id.clone(), Arc::downgrade(&handle));
        Ok(handle)
    }

    /// Starts a plan of the project at `root`.
    pub(crate) fn create(
        &self,
        root: &Path,
        draft: &PlanDraft,
        submission: &Submission,
        facts: &Facts,
    ) -> Result<Arc<PlanHandle>, String> {
        let records = create(draft, submission, facts)
            .map_err(|rejection| rejection.messages().join("\n"))?;
        let handle = Arc::new(PlanHandle::create(
            self.inner.store.clone(),
            project_key(root),
            records,
        )?);
        lock(&self.inner.open).insert(draft.plan_id.clone(), Arc::downgrade(&handle));
        Ok(handle)
    }

    fn live(&self, plan_id: &PlanId) -> Option<Arc<PlanHandle>> {
        lock(&self.inner.open).get(plan_id).and_then(Weak::upgrade)
    }
}

#[cfg(test)]
mod tests {
    use hyperform_protocol::work::{Actor, PlanDepth, PlanStatus, WorkCause};

    use super::*;

    fn draft(root: &Path) -> PlanDraft {
        PlanDraft {
            plan_id: PlanId::new(),
            project_root: root.to_string_lossy().into_owned(),
            goal: "Ship it".into(),
            document: String::new(),
            depth: PlanDepth::Light,
            status: PlanStatus::Draft,
        }
    }

    #[test]
    fn sessions_share_one_handle_per_plan() {
        let dir = tempfile::tempdir().unwrap();
        let hub = PlanHub::new(dir.path().join("plans"));
        let root = dir.path().join("project");
        let submission = Submission::new(Actor::User, WorkCause::Edit, Vec::new());
        let draft = draft(&root);
        let created = hub
            .create(&root, &draft, &submission, &Facts::at(1))
            .unwrap();
        let again = hub.open(&root, &draft.plan_id).unwrap();
        assert!(Arc::ptr_eq(&created, &again));
        assert_eq!(hub.list(&root).unwrap()[0].goal, "Ship it");
        let elsewhere = dir.path().join("other");
        assert!(hub.open(&elsewhere, &draft.plan_id).is_err());
        drop((created, again));
        let reloaded = hub.open(&root, &draft.plan_id).unwrap();
        assert_eq!(reloaded.read(|state| state.header.goal.clone()), "Ship it");
    }
}
