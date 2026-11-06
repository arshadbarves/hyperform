//! Work-plan helpers: settings with `work_plans` (and `work_parallel`) on,
//! the work tools' inputs, dispatching task helpers, the plan log as
//! stored, and the results the model got back.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};
use z_engine_host::project_key;
use z_engine_llm::ModelRequest;
use z_engine_protocol::work::{ParallelPolicy, Task, TaskStatus, WorkEdit, WorkEvent, WorkRecord};
use z_engine_protocol::{Command, CompactionMarker, Event, PlanId, SessionStatus, TaskId};
use z_engine_store::{LoadedPlan, PlanStore};
use z_engine_testkit::Script;
use z_engine_work::PlanState;

use super::{BASE_SETTINGS, Harness, agent_call, results};

/// `BASE_SETTINGS` with `work_plans` on, followed by `extra`.
pub fn work_settings(extra: &str) -> String {
    format!("{BASE_SETTINGS}\n[experimental]\nwork_plans = \"on\"\n\n{extra}")
}

/// `BASE_SETTINGS` with `work_plans` and `work_parallel` on, then `extra`.
pub fn parallel_settings(extra: &str) -> String {
    format!(
        "{BASE_SETTINGS}\n[experimental]\nwork_plans = \"on\"\nwork_parallel = \"on\"\n\n{extra}"
    )
}

/// `Agent` input dispatching a general helper for `task_id`; `marker` in
/// its instructions routes its rounds (`route_task`).
pub fn dispatch(task_id: &str, marker: &str) -> Value {
    let prompt = format!("{marker} work on task {task_id}");
    let mut call = agent_call("general", &format!("task {task_id}"), &prompt);
    call["task_id"] = json!(task_id);
    call
}

/// A helper's `TaskUpdate` sending `report` for `task_id`.
pub fn report(task_id: &str, report: Value) -> Value {
    update_task(task_id, json!({ "report": report }))
}

/// `script` answered after `delay`, so helpers started together overlap.
pub fn slow(script: Script, delay: Duration) -> Script {
    match script {
        Script::Events(events) => Script::Delayed(delay, events),
        other => other,
    }
}

/// `TaskCreate` input for `subject`, with `extra` fields merged in.
pub fn new_task(subject: &str, extra: Value) -> Value {
    with(
        json!({ "subject": subject, "description": format!("{subject}.") }),
        extra,
    )
}

/// `TaskUpdate` input for `task_id`, with `extra` fields merged in.
pub fn update_task(task_id: &str, extra: Value) -> Value {
    with(json!({ "taskId": task_id }), extra)
}

fn with(mut base: Value, extra: Value) -> Value {
    if let (Some(base), Value::Object(extra)) = (base.as_object_mut(), extra) {
        base.extend(extra);
    }
    base
}

/// The first tool result of `request`'s last message: `(is_error, text)`.
pub fn tool_reply(request: &ModelRequest) -> (bool, String) {
    let (_, is_error, text) = results(request.messages.last().unwrap()).remove(0);
    (is_error, text)
}

pub fn task(id: &str) -> TaskId {
    TaskId::from(id)
}

/// `Rejected` audit records in `records`.
pub fn rejections(records: &[WorkRecord]) -> Vec<Vec<String>> {
    records
        .iter()
        .filter_map(|record| match &record.event {
            WorkEvent::Rejected { reasons } => Some(reasons.clone()),
            _ => None,
        })
        .collect()
}

impl Harness {
    /// Runs `prompt` as a turn and waits until the session is idle again,
    /// so the actor takes the next command (`Compact` waits for it).
    pub async fn settled_turn(&mut self, prompt: &str) {
        self.run_turn(prompt).await;
        self.wait(|e| {
            matches!(
                e,
                Event::StatusChanged {
                    status: SessionStatus::Idle
                }
            )
        })
        .await;
    }

    /// Compacts the main history now and waits for it.
    pub async fn compact_now(&mut self) -> CompactionMarker {
        self.send(Command::Compact { instructions: None });
        match self.wait(|e| matches!(e, Event::Compacted { .. })).await {
            Event::Compacted { marker } => marker,
            other => panic!("{other:?}"),
        }
    }

    fn plan_store(&self) -> (PlanStore, String) {
        let root = std::fs::canonicalize(self.repo.path()).unwrap();
        (
            PlanStore::new(self.paths.plans_dir.clone()),
            project_key(&root),
        )
    }

    /// The project's only plan.
    pub fn plan_id(&self) -> PlanId {
        let plans = self.engine.work_plans(self.repo.path()).unwrap();
        assert_eq!(plans.len(), 1, "one plan: {plans:?}");
        plans[0].plan_id.clone()
    }

    /// The project's only plan log, as stored.
    pub fn plan_log(&self) -> LoadedPlan {
        let (store, key) = self.plan_store();
        store.load(&key, &self.plan_id()).unwrap()
    }

    /// The project's only plan, folded from its log on disk.
    pub fn plan(&self) -> PlanState {
        PlanState::fold(&self.plan_log().records).unwrap()
    }

    /// `events.jsonl` of the project's only plan.
    pub fn plan_events_file(&self) -> PathBuf {
        let (store, key) = self.plan_store();
        store
            .plan_dir(&key, &self.plan_id())
            .unwrap()
            .join("events.jsonl")
    }

    /// Sends `edit` to the project's only plan, as the user.
    pub fn edit_plan(&self, edit: WorkEdit) {
        self.send(Command::EditWork {
            plan_id: self.plan_id(),
            edit,
        });
    }

    /// The plan on disk, once it satisfies `done`.
    pub async fn plan_until(&mut self, done: impl Fn(&PlanState) -> bool) -> PlanState {
        loop {
            let plan = self.plan();
            if done(&plan) {
                return plan;
            }
            self.wait(|e| matches!(e, Event::WorkUpdated { .. })).await;
        }
    }

    /// Task `id` of the plan on disk, once it satisfies `done`.
    pub async fn task_until(&mut self, id: &str, done: impl Fn(&Task) -> bool) -> Task {
        let id = task(id);
        let plan = self
            .plan_until(|plan| plan.task(&id).is_some_and(&done))
            .await;
        plan.task(&id).unwrap().clone()
    }

    /// Task `id` once it is Done or Failed.
    pub async fn finished(&mut self, id: &str) -> Task {
        self.task_until(id, |t| {
            matches!(t.status, TaskStatus::Done | TaskStatus::Failed { .. })
        })
        .await
    }

    /// Approves the plan's parallel work, as the user.
    pub async fn approve_parallel(&mut self) {
        self.edit_plan(WorkEdit::ApproveParallel);
        self.plan_until(|plan| plan.header.parallel == ParallelPolicy::Approved)
            .await;
    }

    /// One turn whose first message dispatches each `(task, marker)` pair;
    /// the `(is_error, text)` of each `Agent` result.
    pub async fn dispatch_turn(&mut self, pairs: &[(&str, &str)]) -> Vec<(bool, String)> {
        let calls: Vec<(&str, Value)> = pairs
            .iter()
            .map(|(id, marker)| ("Agent", dispatch(id, marker)))
            .collect();
        self.model.push(Script::tools(&calls));
        self.model.push(Script::text("Dispatched."));
        self.settled_turn("run the tasks").await;
        let last = self.main_requests().pop().unwrap();
        results(last.messages.last().unwrap())
            .into_iter()
            .map(|(_, error, text)| (error, text))
            .collect()
    }
}
