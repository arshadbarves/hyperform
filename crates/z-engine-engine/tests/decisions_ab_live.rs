//! Live A/B of the decision layer (ignored): small fixture tasks run
//! against a real model twice, once with every decision feature off and
//! once with the chosen features on, and the run prints tokens, cost and
//! how many tasks each arm solved (results go in docs/status.md).
//!
//! ```text
//! ZENGINE_MODEL=<model> ZENGINE_API_KEY=<key> \
//! ZENGINE_DECISIONS_URL=http://127.0.0.1:8000 \
//! cargo test -p z-engine-engine --test decisions_ab_live -- --ignored --nocapture
//! ```
//!
//! Optional: `ZENGINE_PROVIDER` and `ZENGINE_BASE_URL` for the model;
//! `ZENGINE_DECISIONS_KEY` (bearer key), `ZENGINE_DECISIONS_ALLOW_REMOTE=1`
//! and `ZENGINE_AB_FEATURES` (comma-separated ids, default every available
//! feature). Without a model or a decision model it says so and passes.

use std::sync::Arc;
use std::time::Duration;

use z_engine_config::{EnvOverrides, FEATURES, Paths};
use z_engine_engine::{Engine, EngineOptions};
use z_engine_protocol::{ApprovalDecision, Command, Event, QuestionAnswer, TurnOutcome};
use z_engine_testkit::{EventRecorder, FixtureRepo};

const TURN_LIMIT: Duration = Duration::from_secs(600);

struct Task {
    name: &'static str,
    files: &'static [(&'static str, &'static str)],
    prompts: &'static [&'static str],
    solved: fn(&FixtureRepo) -> bool,
}

const TASKS: &[Task] = &[
    Task {
        name: "fix a bug",
        files: &[(
            "src/math.js",
            "export function add(a, b) {\n  return a - b;\n}\n",
        )],
        prompts: &["add() in src/math.js returns the wrong result. Fix it to return the sum."],
        solved: |repo| repo.read("src/math.js").contains("a + b"),
    },
    Task {
        name: "three tasks, one chat",
        files: &[
            (
                "app.py",
                "def greet(name):\n    return 'Hi ' + name\n\n\nprint(greet('Ada'))\n",
            ),
            ("NOTES.md", "# Notes\n"),
        ],
        prompts: &[
            "Rename the function greet to welcome in app.py, including its call.",
            "Unrelated: add the line 'Owner: Ada' to NOTES.md.",
            "Another task: create a file VERSION that contains just 1.0.0.",
        ],
        solved: |repo| {
            let app = repo.read("app.py");
            let version = repo.exists("VERSION") && repo.read("VERSION").trim() == "1.0.0";
            app.contains("def welcome(")
                && !app.contains("greet(")
                && version
                && repo.read("NOTES.md").contains("Owner: Ada")
        },
    },
];

#[derive(Debug, Default)]
struct Tally {
    solved: usize,
    failed_turns: usize,
    input: u64,
    cache_read: u64,
    cache_write: u64,
    output: u64,
    cost_usd: f64,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn on_settings(endpoint: &str) -> String {
    let chosen = env("ZENGINE_AB_FEATURES");
    let mut toml = "schema = 2\n\n[experimental]\n".to_string();
    let features = FEATURES.iter().filter(|spec| spec.available);
    for spec in features.map(|spec| spec.id.to_string()) {
        if chosen
            .as_ref()
            .is_none_or(|list| list.split(',').any(|id| id.trim() == spec))
        {
            toml.push_str(&format!("{spec} = \"on\"\n"));
        }
    }
    toml.push_str(&format!(
        "\n[decisions]\nendpoint = \"{endpoint}\"\ntimeout_ms = 1000\n"
    ));
    if env("ZENGINE_DECISIONS_ALLOW_REMOTE").is_some() {
        toml.push_str("allow_remote = true\n");
    }
    if env("ZENGINE_DECISIONS_KEY").is_some() {
        toml.push_str("api_key_env = \"ZENGINE_DECISIONS_KEY\"\n");
    }
    toml
}

/// Runs every task in a fresh engine with `settings`, allowing every
/// approval once and answering questions with their first option.
async fn arm(name: &str, settings: &str) -> Tally {
    let mut tally = Tally::default();
    for task in TASKS {
        let dirs = tempfile::tempdir().unwrap();
        let paths = Paths::with_roots(dirs.path().join("config"), dirs.path().join("data"));
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(&paths.user_settings_file, settings).unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let engine = Engine::new(EngineOptions {
            paths,
            event_sink: Arc::new(move |envelope| {
                tx.send(envelope).ok();
            }),
            client_factory: None,
            env: EnvOverrides::from_process(),
        })
        .unwrap();
        let mut events = EventRecorder::new(rx);
        let repo = FixtureRepo::git(task.files);
        let session = engine.open_session(repo.path(), None).await.unwrap();
        for prompt in task.prompts {
            let submit = Command::Submit {
                text: prompt.to_string(),
                attachments: Vec::new(),
            };
            engine.send(&session, submit).unwrap();
            loop {
                let wanted = |e: &Event| {
                    matches!(
                        e,
                        Event::ApprovalRequested { .. }
                            | Event::QuestionAsked { .. }
                            | Event::TurnFinished { .. }
                    )
                };
                let reply = match events.wait_for_within(TURN_LIMIT, wanted).await {
                    Event::ApprovalRequested { request } => Command::ResolveApproval {
                        request_id: request.request_id,
                        decision: ApprovalDecision::AllowOnce,
                    },
                    Event::QuestionAsked {
                        request_id,
                        questions,
                        ..
                    } => {
                        let answers = questions.iter().map(|q| QuestionAnswer {
                            question: q.question.clone(),
                            answers: q.options.iter().take(1).map(|o| o.label.clone()).collect(),
                        });
                        let answers = Some(answers.collect());
                        Command::AnswerQuestion {
                            request_id,
                            answers,
                        }
                    }
                    Event::TurnFinished { turn } => {
                        tally.failed_turns += usize::from(turn.outcome != TurnOutcome::Completed);
                        tally.input += turn.usage.input_tokens;
                        tally.cache_read += turn.usage.cache_read_tokens;
                        tally.cache_write += turn.usage.cache_write_tokens;
                        tally.output += turn.usage.output_tokens;
                        tally.cost_usd += turn.cost_usd;
                        break;
                    }
                    _ => unreachable!(),
                };
                engine.send(&session, reply).unwrap();
            }
        }
        let solved = (task.solved)(&repo);
        tally.solved += usize::from(solved);
        println!(
            "{name}: {} -> {}",
            task.name,
            if solved { "solved" } else { "not solved" }
        );
        engine.close_session(&session).await.unwrap();
    }
    tally
}

#[tokio::test]
#[ignore = "needs a real model (ZENGINE_MODEL) and a decision model (ZENGINE_DECISIONS_URL)"]
async fn decisions_off_vs_on_with_a_real_model() {
    let (Some(_), Some(endpoint)) = (env("ZENGINE_MODEL"), env("ZENGINE_DECISIONS_URL")) else {
        eprintln!("ZENGINE_MODEL or ZENGINE_DECISIONS_URL is not set; skipping");
        return;
    };
    let off = arm("off", "schema = 2\n").await;
    let on = arm("on", &on_settings(&endpoint)).await;
    println!("arm | solved | failed turns | input | cache read | cache write | output | cost");
    for (name, t) in [("off", &off), ("on", &on)] {
        println!(
            "{name} | {}/{} | {} | {} | {} | {} | {} | ${:.4}",
            t.solved,
            TASKS.len(),
            t.failed_turns,
            t.input,
            t.cache_read,
            t.cache_write,
            t.output,
            t.cost_usd
        );
    }
}
