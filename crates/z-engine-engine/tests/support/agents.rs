//! Subagent helpers: recognizing subagent requests, routing scripts to one
//! subagent by a marker in its task, and waiting for agent events.

use serde_json::{Value, json};
use z_engine_llm::ModelRequest;
use z_engine_protocol::{AgentId, AgentInfo, Event, Role};
use z_engine_testkit::{FixtureRepo, Script, ScriptedModel};

use super::{Harness, all_text};

/// A heading of the subagent preamble, absent from the main prompt.
pub const SUBAGENT_NEEDLE: &str = "## Working as a subagent";

pub fn is_subagent(request: &ModelRequest) -> bool {
    request.system_text().contains(SUBAGENT_NEEDLE)
}

/// Text of a request's first user message (a subagent's task).
pub fn first_user_text(request: &ModelRequest) -> String {
    request
        .messages
        .iter()
        .find(|message| message.role == Role::User)
        .map(all_text)
        .unwrap_or_default()
}

/// Answers the rounds of the subagent whose task contains `marker`.
pub fn route_task(model: &ScriptedModel, marker: &'static str, scripts: Vec<Script>) {
    model.route_queue(
        move |request| is_subagent(request) && first_user_text(request).contains(marker),
        scripts,
    );
}

/// Requests of the subagent whose task contains `marker`.
pub fn task_requests(model: &ScriptedModel, marker: &str) -> Vec<ModelRequest> {
    model
        .requests()
        .into_iter()
        .filter(|request| is_subagent(request) && first_user_text(request).contains(marker))
        .collect()
}

/// `Agent` tool input.
pub fn agent_call(kind: &str, description: &str, prompt: &str) -> Value {
    json!({ "subagent_type": kind, "description": description, "prompt": prompt })
}

pub fn tool_names(request: &ModelRequest) -> Vec<String> {
    request.tools.iter().map(|tool| tool.name.clone()).collect()
}

/// Writes `.z-engine/agents/<name>.md` (before the session opens).
pub fn write_agent(repo: &FixtureRepo, name: &str, frontmatter: &str, body: &str) {
    let text =
        format!("---\nname: {name}\ndescription: Test agent {name}.\n{frontmatter}---\n{body}\n");
    repo.write(&format!(".z-engine/agents/{name}.md"), &text);
}

impl Harness {
    /// The first `n` agents started in this session, in start order.
    pub async fn agents_started(&mut self, n: usize) -> Vec<AgentInfo> {
        loop {
            self.events.drain();
            let started: Vec<AgentInfo> = self
                .events
                .seen()
                .iter()
                .filter_map(|event| match event {
                    Event::AgentStarted { info } => Some(info.clone()),
                    _ => None,
                })
                .collect();
            if started.len() >= n {
                return started;
            }
            self.wait(|event| matches!(event, Event::AgentStarted { .. }))
                .await;
        }
    }

    /// The first agent started in this session.
    pub async fn agent_started(&mut self) -> AgentInfo {
        self.agents_started(1).await.remove(0)
    }

    /// The first update showing `agent` in a terminal state.
    pub async fn agent_finished(&mut self, agent: &AgentId) -> AgentInfo {
        let agent = agent.clone();
        match self
            .expect(move |event| {
                matches!(event, Event::AgentUpdated { info }
                    if info.agent_id == agent && info.status.is_terminal())
            })
            .await
        {
            Event::AgentUpdated { info } => info,
            other => panic!("expected AgentUpdated, got {other:?}"),
        }
    }

    /// The last known `AgentInfo` of `agent` from the events seen so far.
    pub fn agent_info(&mut self, agent: &AgentId) -> AgentInfo {
        self.events.drain();
        self.events
            .seen()
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::AgentStarted { info } | Event::AgentUpdated { info }
                    if info.agent_id == *agent =>
                {
                    Some(info.clone())
                }
                _ => None,
            })
            .expect("an event for the agent")
    }
}
