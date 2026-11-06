//! Work-plan security boundaries: a task's helper changes only its own
//! task; an untrusted project can neither turn on automatic merges nor
//! drop the default serial paths; `Agent { task_id }` with work plans off
//! binds nothing; and the read-only `audit` agent is offered only with
//! work plans on and cannot write, even in Bypass mode.

mod support;

use hyperform_protocol::Event;
use hyperform_protocol::work::{Integration, ParallelBlock, TaskStatus};
use hyperform_testkit::{FixtureRepo, Script};
use serde_json::{Value, json};
use support::{
    BASE_SETTINGS, Harness, agent_call, first_user_text, new_task, parallel_settings, report,
    route_task, task, task_requests, tool_names, tool_reply, update_task, work_settings,
};

const BYPASS: &str = "[permissions]\nmode = \"bypass\"\n";

/// A session over a fresh repo with `tasks` planned in its first turn.
async fn planned(builder: support::Builder, tasks: &[(&str, Value)]) -> Harness {
    let mut h = builder.start().await;
    for (subject, fields) in tasks {
        let input = new_task(subject, fields.clone());
        h.model.push(Script::tool("TaskCreate", input));
    }
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan it").await;
    h
}

fn repo() -> FixtureRepo {
    FixtureRepo::git(&[("README.md", "fixture\n")])
}

#[tokio::test]
async fn a_helper_cannot_change_another_task() {
    let builder = Harness::builder(repo()).settings(&work_settings(BYPASS));
    let tasks = [
        ("Own", json!({ "scope": ["src"] })),
        ("Other", json!({ "scope": ["docs"] })),
    ];
    let mut h = planned(builder, &tasks).await;
    let rounds = vec![
        Script::tool(
            "TaskUpdate",
            update_task("t2", json!({ "status": "in_progress" })),
        ),
        Script::tool("TaskUpdate", report("t2", json!({ "summary": "I did t2" }))),
        Script::tool("TaskUpdate", report("t1", json!({ "summary": "Did t1" }))),
        Script::text("Reported."),
    ];
    route_task(&h.model, "OWN-ONLY", rounds);
    let replies = h.dispatch_turn(&[("t1", "OWN-ONLY")]).await;
    assert!(!replies[0].0, "{replies:?}");

    let helper = task_requests(&h.model, "OWN-ONLY");
    for round in [1, 2] {
        let (error, text) = tool_reply(&helper[round]);
        assert!(error && text.contains("task t2 is not its own"), "{text}");
    }
    let other = h.plan().task(&task("t2")).unwrap().clone();
    assert_eq!(other.status, TaskStatus::Pending);
    assert!(other.run.is_empty(), "{:?}", other.run);
}

#[tokio::test]
async fn an_untrusted_project_cannot_turn_on_automatic_merges() {
    let builder = Harness::builder(repo())
        .settings(&parallel_settings(BYPASS))
        .project_settings("schema = 2\n\n[work]\nmerge = \"auto\"\n");
    let mut h = planned(builder, &[("Add src", json!({ "scope": ["src"] }))]).await;
    h.approve_parallel().await;
    let rounds = vec![
        Script::tools(&[
            (
                "Write",
                json!({ "file_path": "src/new.rs", "content": "new\n" }),
            ),
            (
                "TaskUpdate",
                report("t1", json!({ "summary": "Added new" })),
            ),
        ]),
        Script::text("Reported."),
    ];
    route_task(&h.model, "UNTRUSTED-MERGE", rounds);
    let replies = h.dispatch_turn(&[("t1", "UNTRUSTED-MERGE")]).await;
    assert!(!replies[0].0, "{replies:?}");

    let waiting = h.task_until("t1", |t| t.run.integration.is_some()).await;
    assert!(
        matches!(
            waiting.run.integration,
            Some(Integration::PendingApply { .. })
        ),
        "{:?}",
        waiting.run
    );
    assert!(!h.repo.exists("src/new.rs"), "nothing was merged");
    let warned = h.events.seen().iter().any(|event| {
        matches!(event, Event::Notice { text, .. }
            if text.contains("not trusted") && text.contains("automatic merges"))
    });
    assert!(warned, "the user is told the setting is ignored");
}

#[tokio::test]
async fn an_untrusted_project_cannot_drop_the_default_serial_paths() {
    for trusted in [false, true] {
        let mut builder = Harness::builder(repo())
            .settings(&parallel_settings(""))
            .project_settings("schema = 2\n\n[work]\nserial_paths = []\n");
        if trusted {
            builder = builder.trusted();
        }
        let tasks = [
            ("Add src", json!({ "scope": ["src"] })),
            ("Bump the manifest", json!({ "scope": ["Cargo.toml"] })),
        ];
        let mut h = planned(builder, &tasks).await;
        h.approve_parallel().await;
        let set = h.engine.work_parallel(&h.session).unwrap();
        let serial = set.excluded.iter().any(|excluded| {
            matches!(&excluded.block, ParallelBlock::SerialPaths { task_id, .. } if *task_id == task("t2"))
        });
        assert_eq!(serial, !trusted, "trusted {trusted}: {set:?}");
        assert_eq!(set.eligible.len(), if trusted { 2 } else { 1 }, "{set:?}");
    }
}

#[tokio::test]
async fn agent_with_a_task_id_binds_nothing_once_work_plans_are_off() {
    let builder = Harness::builder(repo()).settings(&work_settings(BYPASS));
    let mut h = planned(builder, &[("Add src", json!({ "scope": ["src"] }))]).await;
    let before = h.plan_log().records.len();
    let off = format!("{BASE_SETTINGS}\n{BYPASS}");
    std::fs::write(&h.paths.user_settings_file, off).unwrap();
    h.events.drain();
    h.engine.reload_settings(None);
    h.wait(|e| matches!(e, Event::Snapshot { .. })).await;
    let reloaded = h.events.seen().len();

    route_task(&h.model, "OFF-HELPER", vec![Script::text("Looked around.")]);
    h.model
        .push(Script::tool("Agent", support::dispatch("t1", "OFF-HELPER")));
    h.model.push(Script::text("Done."));
    h.settled_turn("run t1").await;

    let (error, text) = tool_reply(h.main_requests().last().unwrap());
    assert!(!error, "{text}");
    let started = h.agent_started().await;
    assert_eq!(started.task_id, None);
    let packets = h.events.seen()[reloaded..]
        .iter()
        .filter(|e| matches!(e, Event::TaskPacketSent { .. }));
    assert_eq!(packets.count(), 0, "no packet once work plans are off");
    let helper = task_requests(&h.model, "OFF-HELPER");
    let brief = first_user_text(&helper[0]);
    assert!(!brief.contains("You were dispatched for task"), "{brief}");
    assert_eq!(h.plan_log().records.len(), before, "the plan is untouched");
}

#[tokio::test]
async fn the_audit_agent_is_offered_only_with_work_plans_and_never_writes() {
    let off = format!("{BASE_SETTINGS}\n{BYPASS}");
    for (settings, offered) in [(off, false), (work_settings(BYPASS), true)] {
        let mut h = Harness::builder(repo()).settings(&settings).start().await;
        let write = json!({ "file_path": "audit.txt", "content": "x\n" });
        let rounds = vec![Script::tool("Write", write), Script::text("Audited.")];
        route_task(&h.model, "AUDIT-HELPER", rounds);
        let call = agent_call("audit", "audit", "AUDIT-HELPER review the repo");
        h.model.push(Script::tool("Agent", call));
        h.model.push(Script::text("Done."));
        h.settled_turn("audit").await;

        let main = h.main_requests();
        let agent = main[0].tools.iter().find(|t| t.name == "Agent").unwrap();
        assert_eq!(agent.description.contains("- audit"), offered);
        let (error, text) = tool_reply(main.last().unwrap());
        assert_eq!(error, !offered, "{text}");
        if offered {
            let helper = task_requests(&h.model, "AUDIT-HELPER");
            let mut tools = tool_names(&helper[0]);
            tools.sort();
            assert_eq!(tools, ["Glob", "Grep", "Read"]);
            let (refused, why) = tool_reply(&helper[1]);
            assert!(refused, "{why}");
        } else {
            assert!(text.contains("unknown subagent_type"), "{text}");
        }
        assert!(!h.repo.exists("audit.txt"));
    }
}
