//! One scripted session for the decision-layer end-to-end tests. Task one
//! reads two large files (enough to clear old results), runs a read-only,
//! an allowed and a denied command, writes a file, ends with a claim of
//! passing tests, and gets a follow-up with long command output. After a
//! manual compaction, task two reads again and asks the user a question.
//! Every approval is allowed once and every question gets its first
//! option, so the same script runs whatever the decision features do.

use regex::Regex;
use serde_json::{Value, json};
use z_engine_protocol::{
    ApprovalDecision, ApprovalRequest, Command, Event, QuestionAnswer, SessionStatus, TurnRecord,
};
use z_engine_testkit::{FixtureRepo, Script};

use crate::support::Harness;

/// Settings for the session: a small window so old reads are cleared,
/// `touch` allowed and `rm` denied by rule, `Write` asking as by default.
pub const BASE: &str = "schema = 2\n\n[model]\nmain = \"test-model\"\ncontext_window = 10000\n\n\
                        [context]\nkeep_recent_tool_results = 1\ncompact_at_percent = 99\n\n\
                        [permissions]\nallow = [\"Bash(touch:*)\"]\ndeny = [\"Bash(rm:*)\"]\n";

pub struct Workout {
    pub h: Harness,
    pub turns: Vec<TurnRecord>,
    pub approvals: Vec<ApprovalRequest>,
}

fn large(tag: &str) -> String {
    (0..500)
        .map(|line| format!("{tag} line {line:04} with some filler text\n"))
        .collect()
}

pub async fn run(settings: &str, trusted: bool) -> Workout {
    let (notes, data) = (large("notes"), large("data"));
    let repo = FixtureRepo::git(&[("notes.txt", notes.as_str()), ("data.txt", data.as_str())]);
    let mut builder = Harness::builder(repo).settings(settings);
    if trusted {
        builder = builder.trusted();
    }
    let mut w = Workout {
        h: builder.start().await,
        turns: Vec::new(),
        approvals: Vec::new(),
    };
    let read =
        |w: &Workout, file: &str| Script::tool("Read", json!({ "file_path": w.h.path(file) }));
    let bash = |command: &str| Script::tool("Bash", json!({ "command": command }));
    let write = |w: &Workout, file: &str| {
        let input = json!({ "file_path": w.h.path(file), "content": "hello\n" });
        Script::tool("Write", input)
    };

    let first = [
        read(&w, "notes.txt"),
        read(&w, "data.txt"),
        bash("ls"),
        write(&w, "greeting.txt"),
        bash("touch made.txt"),
        bash("rm -rf build"),
        Script::text("Done. The greeting is fixed and all tests pass."),
    ];
    w.turn(&first, "Fix the greeting; always keep answers short.")
        .await;
    let follow_up = [
        bash("seq 1 400"),
        write(&w, "farewell.txt"),
        Script::text("Added a farewell line."),
    ];
    w.turn(&follow_up, "also add a farewell line").await;

    w.h.wait(|e| {
        matches!(
            e,
            Event::StatusChanged {
                status: SessionStatus::Idle
            }
        )
    })
    .await;
    w.h.send(Command::Compact { instructions: None });
    w.h.wait(|e| matches!(e, Event::Compacted { .. })).await;

    let question = json!({ "questions": [{
        "question": "Which format should the README summary use?",
        "header": "Format",
        "options": [
            { "label": "Bullets", "description": "a short list" },
            { "label": "Prose", "description": "one paragraph" }
        ],
        "multiSelect": false
    }]});
    let second = [
        read(&w, "notes.txt"),
        Script::tool("AskUserQuestion", question),
        Script::text("The notes are filler lines; summarized as bullets."),
    ];
    w.turn(
        &second,
        "switch: summarize notes.txt as bullets for the README",
    )
    .await;
    w
}

impl Workout {
    /// Queues `scripts`, submits `text`, and answers every approval and
    /// question until the turn finishes.
    async fn turn(&mut self, scripts: &[Script], text: &str) {
        for script in scripts {
            self.h.model.push(script.clone());
        }
        self.h.submit(text);
        loop {
            let event = self
                .h
                .wait(|e| {
                    matches!(
                        e,
                        Event::ApprovalRequested { .. }
                            | Event::QuestionAsked { .. }
                            | Event::TurnFinished { .. }
                    )
                })
                .await;
            match event {
                Event::ApprovalRequested { request } => {
                    let request_id = request.request_id.clone();
                    let decision = ApprovalDecision::AllowOnce;
                    self.h.send(Command::ResolveApproval {
                        request_id,
                        decision,
                    });
                    self.approvals.push(request);
                }
                Event::QuestionAsked {
                    request_id,
                    questions,
                    ..
                } => {
                    let answers = questions.iter().map(|q| QuestionAnswer {
                        question: q.question.clone(),
                        answers: vec![q.options[0].label.clone()],
                    });
                    let answers = Some(answers.collect());
                    self.h.send(Command::AnswerQuestion {
                        request_id,
                        answers,
                    });
                }
                Event::TurnFinished { turn } => {
                    self.turns.push(turn);
                    return;
                }
                _ => unreachable!(),
            }
        }
    }

    fn normalize(&self, text: &str) -> String {
        let root = self.h.repo.path().to_string_lossy().into_owned();
        let data = self.h.paths.data_dir.to_string_lossy().into_owned();
        let text = text.replace(&root, "<root>").replace(&data, "<data>");
        let ulid = Regex::new("(?i)01[0-9a-z]{24}").unwrap();
        ulid.replace_all(&text, "<id>").into_owned()
    }

    /// Each turn's verification outcome, with check record ids replaced.
    pub fn verification(&self) -> Vec<String> {
        let turns = self.turns.iter();
        let outcomes = turns.map(|turn| format!("{:?}", turn.verification));
        outcomes.map(|outcome| self.normalize(&outcome)).collect()
    }

    /// The conversation each main request carried (roles, text, calls and
    /// results), with run-specific paths and call ids replaced.
    pub fn requests(&self) -> Vec<String> {
        let requests = self.h.main_requests();
        let messages = requests.iter().map(|request| {
            let parts = request.messages.iter();
            let parts = parts.map(|m| json!({ "role": m.role, "content": m.content }));
            Value::Array(parts.collect()).to_string()
        });
        messages.map(|text| self.normalize(&text)).collect()
    }

    /// `(tool, title)` of every approval card, in order.
    pub fn asked(&self) -> Vec<(String, String)> {
        let asked = self.approvals.iter();
        let asked = asked.map(|a| (a.tool.clone(), self.normalize(&a.title)));
        asked.collect()
    }

    /// `(tool, status)` of every finished tool call, in order.
    pub fn statuses(&mut self) -> Vec<(String, String)> {
        let seen = self.h.events.drain().to_vec();
        let mut tools = std::collections::HashMap::new();
        let mut statuses = Vec::new();
        for event in seen {
            match event {
                Event::ToolStarted { call_id, tool, .. } => {
                    tools.insert(call_id, tool);
                }
                Event::ToolFinished {
                    call_id, status, ..
                } => {
                    let tool = tools.get(&call_id).cloned().unwrap_or_default();
                    statuses.push((tool, format!("{status:?}")));
                }
                _ => {}
            }
        }
        statuses
    }
}
