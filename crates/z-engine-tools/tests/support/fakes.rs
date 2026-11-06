//! Fake ports: canned answers, and every call recorded for assertions.

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use z_engine_protocol::{
    AgentId, CheckRecord, JobId, JobKind, JobStatus, PlanDecision, Question, QuestionAnswer,
    TodoItem, ToolResultPart,
};
use z_engine_tools::{
    AgentCard, AgentPort, CheckPort, CheckSummary, InteractionPort, JobOutput, JobPort, LspPort,
    LspRequest, McpPort, SideModelPort, SkillContent, SkillPort, SpawnOutcome, SpawnRequest,
    ToolCtx,
};

#[derive(Debug, Default)]
pub struct FakeAgents {
    pub catalog: Vec<AgentCard>,
    pub background_job: Option<JobId>,
    pub requests: Mutex<Vec<SpawnRequest>>,
    pub applied: Mutex<Vec<AgentId>>,
}

#[async_trait]
impl AgentPort for FakeAgents {
    fn catalog(&self) -> Vec<AgentCard> {
        self.catalog.clone()
    }

    async fn spawn(&self, _ctx: &ToolCtx, req: SpawnRequest) -> Result<SpawnOutcome, String> {
        let background = req.background;
        self.requests.lock().unwrap().push(req);
        Ok(SpawnOutcome {
            agent_id: AgentId::from("agt_child"),
            job_id: if background {
                self.background_job.clone()
            } else {
                None
            },
            text: if background {
                "Started in the background.".into()
            } else {
                "Found it in src/lib.rs:3.".into()
            },
            footer: "12 tool calls, 3.1k tokens".into(),
        })
    }

    async fn apply_changes(&self, _ctx: &ToolCtx, agent_id: &AgentId) -> Result<String, String> {
        self.applied.lock().unwrap().push(agent_id.clone());
        Ok(format!("Applied 2 files from {agent_id}"))
    }
}

#[derive(Debug)]
pub struct FakeInteraction {
    pub can_ask: bool,
    pub answers: Option<Vec<QuestionAnswer>>,
    pub answered_earlier: Option<String>,
    pub decision: PlanDecision,
    pub asked: Mutex<Vec<Vec<Question>>>,
    pub plans: Mutex<Vec<String>>,
    pub todos: Mutex<Vec<Vec<TodoItem>>>,
}

impl FakeInteraction {
    pub fn new(can_ask: bool) -> Self {
        Self {
            can_ask,
            answers: None,
            answered_earlier: None,
            decision: PlanDecision::Revise {
                feedback: "more detail".into(),
            },
            asked: Mutex::default(),
            plans: Mutex::default(),
            todos: Mutex::default(),
        }
    }
}

#[async_trait]
impl InteractionPort for FakeInteraction {
    fn can_ask(&self, _ctx: &ToolCtx) -> bool {
        self.can_ask
    }

    async fn already_answered(&self, _ctx: &ToolCtx, _questions: &[Question]) -> Option<String> {
        self.answered_earlier.clone()
    }

    async fn ask(
        &self,
        _ctx: &ToolCtx,
        questions: Vec<Question>,
    ) -> Result<Option<Vec<QuestionAnswer>>, String> {
        self.asked.lock().unwrap().push(questions);
        Ok(self.answers.clone())
    }

    async fn propose_plan(&self, _ctx: &ToolCtx, plan: String) -> Result<PlanDecision, String> {
        self.plans.lock().unwrap().push(plan);
        Ok(self.decision.clone())
    }

    fn update_todos(&self, _ctx: &ToolCtx, todos: Vec<TodoItem>) {
        self.todos.lock().unwrap().push(todos);
    }
}

type JobRead = (JobId, Option<String>, Option<Duration>);

#[derive(Debug)]
pub struct FakeJobs {
    pub output: JobOutput,
    pub spawned: Mutex<Vec<(String, Option<String>)>>,
    pub reads: Mutex<Vec<JobRead>>,
    pub killed: Mutex<Vec<JobId>>,
}

impl Default for FakeJobs {
    fn default() -> Self {
        Self {
            output: JobOutput {
                status: JobStatus::Completed,
                exit_code: Some(0),
                output: "server ready\n".into(),
                kind: JobKind::Shell,
            },
            spawned: Mutex::default(),
            reads: Mutex::default(),
            killed: Mutex::default(),
        }
    }
}

#[async_trait]
impl JobPort for FakeJobs {
    async fn spawn_shell(
        &self,
        _ctx: &ToolCtx,
        command: String,
        description: Option<String>,
    ) -> Result<JobId, String> {
        self.spawned.lock().unwrap().push((command, description));
        Ok(JobId::from("job_1"))
    }

    async fn output(
        &self,
        _ctx: &ToolCtx,
        job: &JobId,
        filter: Option<String>,
        wait: Option<Duration>,
    ) -> Result<JobOutput, String> {
        if job.as_str() != "job_1" {
            return Err(format!("no job {job}"));
        }
        self.reads.lock().unwrap().push((job.clone(), filter, wait));
        Ok(self.output.clone())
    }

    async fn kill(&self, _ctx: &ToolCtx, job: &JobId) -> Result<(), String> {
        self.killed.lock().unwrap().push(job.clone());
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct FakeSkills;

impl SkillPort for FakeSkills {
    fn list(&self) -> Vec<(String, String)> {
        vec![("pdf".into(), "Work with PDFs".into())]
    }

    fn load(&self, name: &str) -> Result<SkillContent, String> {
        match name {
            "pdf" => Ok(SkillContent {
                body: "Use scripts/extract.py.".into(),
                dir: "/skills/pdf".into(),
            }),
            other => Err(format!("no skill named {other}")),
        }
    }
}

#[derive(Debug)]
pub struct FakeChecks {
    pub checks: Vec<CheckSummary>,
    pub record: CheckRecord,
    pub runs: Mutex<Vec<String>>,
}

#[async_trait]
impl CheckPort for FakeChecks {
    fn list(&self, _ctx: &ToolCtx) -> Vec<CheckSummary> {
        self.checks.clone()
    }

    async fn run(&self, _ctx: &ToolCtx, check_id: &str) -> Result<CheckRecord, String> {
        self.runs.lock().unwrap().push(check_id.to_string());
        Ok(self.record.clone())
    }
}

#[derive(Debug, Default)]
pub struct FakeLsp {
    pub requests: Mutex<Vec<LspRequest>>,
}

#[async_trait]
impl LspPort for FakeLsp {
    async fn query(&self, _ctx: &ToolCtx, req: LspRequest) -> Result<String, String> {
        let answer = format!("{} -> src/lib.rs:10:5", req.operation);
        self.requests.lock().unwrap().push(req);
        Ok(answer)
    }
}

#[derive(Debug, Default)]
pub struct FakeMcp {
    pub is_error: bool,
    pub calls: Mutex<Vec<(String, String, Value)>>,
    pub listed: Mutex<Vec<Option<String>>>,
}

#[async_trait]
impl McpPort for FakeMcp {
    async fn call_tool(
        &self,
        _ctx: &ToolCtx,
        server: &str,
        tool: &str,
        input: Value,
    ) -> Result<(Vec<ToolResultPart>, bool), String> {
        self.calls
            .lock()
            .unwrap()
            .push((server.into(), tool.into(), input));
        Ok((
            vec![ToolResultPart::Text {
                text: "issue #7 created\nmore".into(),
            }],
            self.is_error,
        ))
    }

    async fn list_resources(&self, _ctx: &ToolCtx, server: Option<&str>) -> Result<String, String> {
        self.listed.lock().unwrap().push(server.map(str::to_string));
        Ok("docs: file:///readme (README)".into())
    }

    async fn read_resource(
        &self,
        _ctx: &ToolCtx,
        server: &str,
        uri: &str,
    ) -> Result<Vec<ToolResultPart>, String> {
        Ok(vec![ToolResultPart::Text {
            text: format!("{server}:{uri} body"),
        }])
    }
}

#[derive(Debug, Default)]
pub struct FakeSideModel {
    pub fail: bool,
    pub calls: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl SideModelPort for FakeSideModel {
    async fn extract(
        &self,
        _ctx: &ToolCtx,
        content: String,
        prompt: String,
    ) -> Result<String, String> {
        self.calls.lock().unwrap().push((content, prompt.clone()));
        if self.fail {
            Err("model overloaded".into())
        } else {
            Ok(format!("extracted answer to: {prompt}"))
        }
    }
}
