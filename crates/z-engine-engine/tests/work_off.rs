//! With `work_plans` Off (the default) nothing of work plans shows: the
//! model is offered no work tools and no plan reminders, a compaction sends
//! the summarizer the same request as before, nothing is written under the
//! plans directory, and the plan commands are refused with a notice.

mod support;

use serde_json::json;
use support::{
    BASE_SETTINGS, Harness, SUMMARY_NEEDLE, all_text, new_task, tool_names, tool_reply,
    work_settings,
};
use z_engine_llm::ModelRequest;
use z_engine_prompts::auxiliary::{COMPACT, COMPACT_WORK};
use z_engine_protocol::work::WorkEdit;
use z_engine_protocol::{Command, Event, PlanId};
use z_engine_testkit::{FixtureRepo, Script};
use z_engine_tools::names;

const PLAN_REMINDERS: [&str; 3] = [
    "This chat works on the plan",
    "The task packet for",
    "Checking the work plan against the workspace",
];

fn assert_untouched_by_work(request: &ModelRequest) {
    let tools = tool_names(request);
    for work in names::WORK {
        assert!(!tools.iter().any(|name| name == work), "{work} offered");
    }
    let exit = request
        .tools
        .iter()
        .find(|tool| tool.name == names::EXIT_PLAN_MODE)
        .expect("ExitPlanMode in plan mode");
    let fields = exit.input_schema["properties"].as_object().unwrap();
    assert!(
        fields.keys().all(|key| key == "plan"),
        "plain ExitPlanMode: {fields:?}"
    );
    let text: String = request.messages.iter().map(all_text).collect();
    for reminder in PLAN_REMINDERS {
        assert!(!text.contains(reminder), "{reminder} sent");
    }
}

fn plans_on_disk(h: &Harness) -> usize {
    match std::fs::read_dir(&h.paths.plans_dir) {
        Ok(entries) => entries.count(),
        Err(_) => 0,
    }
}

#[tokio::test]
async fn off_offers_no_work_tools_or_reminders_and_writes_no_plan() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nplanning = true\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model
        .push(Script::tool("TaskCreate", new_task("Ship it", json!({}))));
    h.model.push(Script::text("Planned."));
    h.run_turn("plan the release").await;
    h.model.push(Script::text("Still planning."));
    h.run_turn("go on").await;

    let requests = h.main_requests();
    assert_eq!(requests.len(), 3);
    for request in &requests {
        assert_untouched_by_work(request);
    }
    let (is_error, text) = tool_reply(&requests[1]);
    assert!(is_error, "an unknown tool: {text}");
    assert_eq!(plans_on_disk(&h), 0);
    assert!(h.engine.work_plans(h.repo.path()).unwrap().is_empty());
    h.events.drain();
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::WorkUpdated { .. } | Event::TaskPacketSent { .. })),
        0
    );
}

#[tokio::test]
async fn off_refuses_plan_commands_with_a_notice() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    let plan_id = PlanId::new();

    h.send(Command::AttachPlan {
        plan_id: plan_id.clone(),
    });
    let attach = h.notice("Work plans are off").await;
    assert!(attach.contains("Settings > Experimental"), "{attach}");
    h.send(Command::EditWork {
        plan_id,
        edit: WorkEdit::Approve,
    });
    let edit = h.notice("Work plans are off").await;
    assert_eq!(edit, attach);
    h.command("replan", "");
    h.notice("/replan is not a known command").await;
    assert_eq!(plans_on_disk(&h), 0);
}

/// Two turns, a manual compaction, and the summarizer's request.
async fn summary_request(settings: &str) -> (ModelRequest, Harness) {
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(settings)
        .start()
        .await;
    for (prompt, reply) in [("first", "one"), ("second", "two")] {
        h.model.push(Script::text(reply));
        h.settled_turn(prompt).await;
    }
    h.compact_now().await;
    let request = h
        .model
        .requests()
        .into_iter()
        .find(|request| request.system_text().contains(SUMMARY_NEEDLE))
        .expect("a summary request");
    (request, h)
}

#[tokio::test]
async fn compaction_without_a_plan_sends_the_summarizer_the_same_request() {
    let (off, off_h) = summary_request(BASE_SETTINGS).await;
    let (on, on_h) = summary_request(&work_settings("")).await;

    assert_eq!(off.system_text(), COMPACT);
    assert!(!off.system_text().contains(COMPACT_WORK.trim()));
    assert_eq!(on.system_text(), off.system_text());
    let content = |request: &ModelRequest| {
        request
            .messages
            .iter()
            .map(|message| (message.role, message.content.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(content(&on), content(&off));
    assert_eq!(on.tools.len(), off.tools.len());
    for h in [&off_h, &on_h] {
        let after = h.transcript();
        assert!(
            !after
                .iter()
                .map(all_text)
                .any(|text| text.contains("task packet")),
            "no packet without a plan"
        );
        assert_eq!(plans_on_disk(h), 0);
    }
}
