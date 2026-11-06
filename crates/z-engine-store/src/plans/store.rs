//! `PlanStore`: one directory per plan under the project's key,
//! `<plans dir>/<project key>/<plan id>/` holding `events.jsonl` (one
//! `WorkRecord` per line) and `meta.json`. The store reads and appends
//! records; folding them into a plan is the engine's job.

use std::fs;
use std::path::{Path, PathBuf};

use z_engine_protocol::PlanId;
use z_engine_protocol::work::{PlanSummary, WorkRecord};

use super::meta;
use crate::append::Appender;
use crate::durable::sync_dir;
use crate::error::StoreError;
use crate::layout::{check_id, is_valid_id};
use crate::read::read_jsonl;

const EVENTS_FILE: &str = "events.jsonl";
const META_FILE: &str = "meta.json";

#[derive(Debug, Clone)]
pub struct PlanStore {
    root: PathBuf,
}

/// The records of one plan log plus what had to be skipped to read them.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPlan {
    pub records: Vec<WorkRecord>,
    /// The final line was a torn fragment and was ignored.
    pub torn_tail: bool,
    /// Lines that were not JSON or not a known record.
    pub corrupt_lines: usize,
}

impl PlanStore {
    pub fn new(plans_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: plans_dir.into(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.root
    }

    /// `<plans dir>/<project key>`; the key must be a valid path segment.
    pub fn project_dir(&self, project_key: &str) -> Result<PathBuf, StoreError> {
        check_id("project key", project_key)?;
        Ok(self.root.join(project_key))
    }

    pub fn plan_dir(&self, project_key: &str, plan_id: &PlanId) -> Result<PathBuf, StoreError> {
        check_id("plan id", plan_id.as_str())?;
        Ok(self.project_dir(project_key)?.join(plan_id.as_str()))
    }

    /// Create the plan directory with `records` (synced) and `meta.json`.
    /// The plan id comes from `summary`; an existing plan is `Invalid`.
    pub fn create(
        &self,
        project_key: &str,
        records: &[WorkRecord],
        summary: &PlanSummary,
    ) -> Result<PlanLog, StoreError> {
        let project = self.project_dir(project_key)?;
        let dir = self.plan_dir(project_key, &summary.plan_id)?;
        fs::create_dir_all(&project).map_err(|error| StoreError::io(&project, error))?;
        fs::create_dir(&dir).map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => {
                StoreError::Invalid(format!("plan {} already exists", summary.plan_id))
            }
            _ => StoreError::io(&dir, error),
        })?;
        let initialize = || {
            let mut log = PlanLog::new(Appender::create_new(&dir.join(EVENTS_FILE))?);
            log.append_all(records)?;
            log.sync()?;
            meta::write(&dir.join(META_FILE), summary)?;
            sync_dir(&project)?;
            Ok(log)
        };
        initialize().inspect_err(|_| remove_partial(&dir))
    }

    /// Append handle for an existing plan; reopening repairs a torn tail.
    pub fn open_append(&self, project_key: &str, plan_id: &PlanId) -> Result<PlanLog, StoreError> {
        let path = self.plan_dir(project_key, plan_id)?.join(EVENTS_FILE);
        Ok(PlanLog::new(Appender::open(&path, false)?))
    }

    /// Every readable record, in log order.
    pub fn load(&self, project_key: &str, plan_id: &PlanId) -> Result<LoadedPlan, StoreError> {
        let path = self.plan_dir(project_key, plan_id)?.join(EVENTS_FILE);
        let lines = read_jsonl::<WorkRecord>(&path)?;
        Ok(LoadedPlan {
            records: lines.items,
            torn_tail: lines.torn_tail,
            corrupt_lines: lines.corrupt_lines,
        })
    }

    /// Atomically rewrite `meta.json` of an existing plan.
    pub fn write_meta(&self, project_key: &str, summary: &PlanSummary) -> Result<(), StoreError> {
        let dir = self.plan_dir(project_key, &summary.plan_id)?;
        if !dir.is_dir() {
            return Err(StoreError::NotFound(format!("plan {}", summary.plan_id)));
        }
        meta::write(&dir.join(META_FILE), summary)
    }

    pub fn read_meta(
        &self,
        project_key: &str,
        plan_id: &PlanId,
    ) -> Result<PlanSummary, StoreError> {
        meta::read(&self.plan_dir(project_key, plan_id)?.join(META_FILE))
    }

    pub fn exists(&self, project_key: &str, plan_id: &PlanId) -> bool {
        self.plan_dir(project_key, plan_id)
            .is_ok_and(|dir| dir.join(EVENTS_FILE).is_file())
    }

    /// Ids of the project's plans (directories holding a plan log), sorted.
    pub fn plan_ids(&self, project_key: &str) -> Result<Vec<PlanId>, StoreError> {
        let project = self.project_dir(project_key)?;
        let entries = match fs::read_dir(&project) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(StoreError::io(&project, error)),
        };
        let mut ids: Vec<PlanId> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().join(EVENTS_FILE).is_file())
            .filter_map(|entry| entry.file_name().to_str().map(String::from))
            .filter(|name| is_valid_id(name))
            .map(PlanId)
            .collect();
        ids.sort();
        Ok(ids)
    }

    /// Summaries of the project's plans, most recently updated first. A
    /// plan whose `meta.json` is missing or unreadable is left out (the
    /// engine rebuilds it from the log; `plan_ids` still lists it).
    pub fn list(&self, project_key: &str) -> Result<Vec<PlanSummary>, StoreError> {
        let mut plans: Vec<PlanSummary> = self
            .plan_ids(project_key)?
            .iter()
            .filter_map(|id| match self.read_meta(project_key, id) {
                Ok(summary) => Some(summary),
                Err(error) => {
                    tracing::warn!(plan = %id, %error, "skipping plan without readable meta.json");
                    None
                }
            })
            .collect();
        plans.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.plan_id.cmp(&a.plan_id))
        });
        Ok(plans)
    }
}

/// Append handle for `<plan>/events.jsonl`. Same failure behavior as
/// `SessionLog`: after a failed write every handle refuses records until
/// `PlanStore::open_append` reopens the log.
#[derive(Debug)]
pub struct PlanLog {
    inner: Appender,
}

impl PlanLog {
    fn new(inner: Appender) -> Self {
        Self { inner }
    }

    pub fn append(&mut self, record: &WorkRecord) -> Result<(), StoreError> {
        self.inner.append(record)
    }

    /// The records of one revision, in order.
    pub fn append_all(&mut self, records: &[WorkRecord]) -> Result<(), StoreError> {
        records
            .iter()
            .try_for_each(|record| self.inner.append(record))
    }

    pub fn sync(&mut self) -> Result<(), StoreError> {
        self.inner.sync()
    }

    pub fn path(&self) -> &Path {
        self.inner.path()
    }
}

fn remove_partial(dir: &Path) {
    if let Err(error) = fs::remove_dir_all(dir) {
        tracing::warn!(path = %dir.display(), %error, "could not remove partially created plan");
    }
}
