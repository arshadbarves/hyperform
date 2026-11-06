//! Work plans for the Plan tab: a project's plans, the packet the model
//! gets for a task of a session's attached plan, why that task looks the
//! way it does, and which tasks may run in parallel helpers.

use std::path::Path;

use hyperform_context::estimate_text;
use hyperform_protocol::work::{ParallelSet, PlanStatus, PlanSummary, TaskPacket, WhyStep};
use hyperform_protocol::{SessionId, TaskId};
use hyperform_work::{parallel, select, why};

use crate::engine::Engine;
use crate::error::EngineError;
use crate::session::SessionCore;
use crate::work::{PlanHandle, current, instant};

impl Engine {
    /// The project's plans, most recently updated first.
    pub fn work_plans(&self, project_root: &Path) -> Result<Vec<PlanSummary>, EngineError> {
        let root = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.into());
        self.shared()
            .plans
            .list(&root)
            .map_err(EngineError::Invalid)
    }

    /// The packet of `task_id` as the main agent would get it now.
    pub async fn work_packet(
        &self,
        session_id: &SessionId,
        task_id: &str,
    ) -> Result<TaskPacket, EngineError> {
        let handle = self
            .handle(session_id)
            .ok_or_else(|| EngineError::NotOpen(session_id.clone()))?;
        let core = &handle.core;
        let plan = attached_plan(core)?;
        let facts = current(core, &core.root).await;
        let budget = u64::from(core.settings().settings.work.packet_tokens);
        let task_id = TaskId(task_id.trim().to_string());
        plan.read(|state| select(state, &task_id, budget, &facts, &estimate_text))
            .map_err(|error| EngineError::NotFound(error.to_string()))
    }

    /// The records that made `task_id` what it is, oldest first.
    pub fn work_why(
        &self,
        session_id: &SessionId,
        task_id: &str,
    ) -> Result<Vec<WhyStep>, EngineError> {
        let handle = self
            .handle(session_id)
            .ok_or_else(|| EngineError::NotOpen(session_id.clone()))?;
        let plan = attached_plan(&handle.core)?;
        let task_id = TaskId(task_id.trim().to_string());
        plan.read_log(|records, state| {
            state
                .task(&task_id)
                .map(|_| why(records, state, &task_id))
                .ok_or_else(|| EngineError::NotFound(format!("task {task_id}")))
        })
    }

    /// The tasks that may start in parallel helpers now (an Active plan),
    /// or that would once parallel work is approved (a Draft's preview).
    pub fn work_parallel(&self, session_id: &SessionId) -> Result<ParallelSet, EngineError> {
        let handle = self
            .handle(session_id)
            .ok_or_else(|| EngineError::NotOpen(session_id.clone()))?;
        let core = &handle.core;
        let plan = attached_plan(core)?;
        let facts = instant(core);
        Ok(plan.read(|state| match state.header.status {
            PlanStatus::Draft => parallel::preview(state, &facts),
            _ => parallel::eligible(state, &facts),
        }))
    }
}

fn attached_plan(core: &SessionCore) -> Result<std::sync::Arc<PlanHandle>, EngineError> {
    core.work
        .plan()
        .ok_or_else(|| EngineError::NotFound("no work plan is attached to this chat".into()))
}

#[cfg(test)]
mod tests {
    use hyperform_protocol::AgentId;
    use hyperform_protocol::work::{ParallelBlock, TaskScope, WorkCause, WorkEdit};
    use hyperform_tools::NewTask;
    use hyperform_work::Submission;

    use super::super::testing::engine;
    use crate::work::{Ids, agent, ensure, instant, proposals};

    #[tokio::test]
    async fn plans_packets_and_why_come_from_the_attached_plan() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let settings = "schema = 2\n[experimental]\nwork_plans = \"on\"\n";
        std::fs::write(&engine.paths().user_settings_file, settings).unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let id = engine.open_session(&project, None).await.unwrap();
        assert!(engine.work_plans(&project).unwrap().is_empty());
        assert!(engine.work_packet(&id, "t1").await.is_err());

        let core = &engine.handle(&id).unwrap().core;
        let actor = agent(core, &AgentId::main());
        let cause = WorkCause::Edit;
        let (plan, fresh) =
            ensure(core, &actor, &cause, "Ship login", &TaskScope::default()).unwrap();
        assert!(fresh);
        let task = NewTask {
            subject: "Add the form".into(),
            description: "A login form".into(),
            ..NewTask::default()
        };
        plan.submit(&instant(core), |state| {
            let (_, proposal) = proposals::new_task(&mut Ids::new(state), &task)?;
            Ok(Submission::new(actor, cause, vec![proposal]))
        })
        .unwrap();

        let plans = engine.work_plans(&project).unwrap();
        assert_eq!((plans.len(), plans[0].goal.as_str()), (1, "Ship login"));
        assert_eq!(plans[0].tasks_total, 1);
        let packet = engine.work_packet(&id, "t1").await.unwrap();
        assert_eq!(
            (packet.task.title.as_str(), packet.goal.as_str()),
            ("Add the form", "Ship login")
        );
        assert!(!engine.work_why(&id, "t1").unwrap().is_empty());
        assert!(engine.work_why(&id, "t9").is_err());
        assert!(matches!(
            engine.work_why(&hyperform_protocol::SessionId::new(), "t1"),
            Err(crate::EngineError::NotOpen(_))
        ));
        engine.shutdown().await;
    }

    #[tokio::test]
    async fn a_draft_previews_parallel_work_and_an_active_plan_needs_approval() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let settings = "schema = 2\n[experimental]\nwork_plans = \"on\"\n[work]\nmax_parallel = 2\n[agents]\nmax_concurrent = 4\n";
        std::fs::write(&engine.paths().user_settings_file, settings).unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let id = engine.open_session(&project, None).await.unwrap();
        assert!(engine.work_parallel(&id).is_err(), "no plan attached");

        let core = &engine.handle(&id).unwrap().core;
        core.with_state(|state| state.planning = true);
        let actor = agent(core, &AgentId::main());
        let (plan, _) = ensure(
            core,
            &actor,
            &WorkCause::Edit,
            "Split",
            &TaskScope::default(),
        )
        .unwrap();
        let scoped = |subject: &str, file: &str| NewTask {
            subject: subject.into(),
            description: format!("{subject}."),
            scope: vec![file.into()],
            ..NewTask::default()
        };
        plan.submit(&instant(core), |state| {
            let mut ids = Ids::new(state);
            let (_, a) = proposals::new_task(&mut ids, &scoped("Lexer", "src/lex.rs"))?;
            let (_, b) = proposals::new_task(&mut ids, &scoped("Docs", "docs/lex.md"))?;
            Ok(Submission::new(actor, WorkCause::Edit, vec![a, b]))
        })
        .unwrap();
        let preview = engine.work_parallel(&id).unwrap();
        assert_eq!((preview.eligible.len(), preview.max), (2, 2), "{preview:?}");

        core.with_state(|state| state.planning = false);
        let approve = |edit: WorkEdit| {
            plan.submit(&instant(core), |_| Ok(Submission::edit(edit)))
                .unwrap()
        };
        approve(WorkEdit::Approve);
        let waiting = engine.work_parallel(&id).unwrap();
        assert!(waiting.eligible.is_empty(), "{waiting:?}");
        assert!(matches!(
            waiting.excluded[0].block,
            ParallelBlock::NotApproved { .. }
        ));
        approve(WorkEdit::ApproveParallel);
        assert_eq!(engine.work_parallel(&id).unwrap().eligible.len(), 2);
        engine.shutdown().await;
    }
}
