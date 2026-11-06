//! Routing never overrides the user: an explicit effort or another model
//! wins, switching models needs `allow_model_switch`, a large request is
//! never routed down, and off, shadow, down or unsure route nothing. A
//! follow-up keeps its task's route; a new task closes the old one with its
//! cost and prompt-cache share.

use z_engine_config::{FeatureId, FeatureMode};
use z_engine_decisions::DecisionRecord;
use z_engine_protocol::decisions::RouteInfo;
use z_engine_protocol::{
    AgentId, Effort, Event, MessageId, TurnId, TurnOutcome, TurnRecord, Usage, VerificationOutcome,
    now_ms,
};

use super::decide::{COMPLEXITY, NEW_TASK};
use super::start::{CACHE_LIFETIME_MS, TASK_COST};
use super::{route_new_task, route_subagent, routed_effort};
use crate::decisions::uses::scripted::{
    Events, Scripted, install, main_run, session, session_with, traced,
};
use crate::session::SessionHandle;

const FEATURE: FeatureId = FeatureId::DecisionsRouting;
const SWITCHING: &str = "[model]\nmain = \"main-model\"\nfast = \"fast-model\"\n\n\
                         [decisions.routing]\nallow_model_switch = true\n";
const SMALL: &str = "fix the typo in the README title";
const FOLLOW_UP: &str = "also handle the empty input in that function";

fn sized(tier: &str) -> Scripted {
    Scripted::default().choice(COMPLEXITY, tier)
}

fn routes(events: &Events) -> Vec<RouteInfo> {
    let events = events.lock().unwrap();
    let routes = events.iter().filter_map(|event| match event {
        Event::RouteChosen { route } => Some(route.clone()),
        _ => None,
    });
    routes.collect()
}

/// The chat's last turn finished `idle_ms` ago.
fn finished(handle: &SessionHandle, idle_ms: u64) {
    let finished_at = now_ms().saturating_sub(idle_ms);
    handle.core.with_state(|state| {
        state.turns.push(TurnRecord {
            turn_id: TurnId::new(),
            message_id: MessageId::new(),
            outcome: TurnOutcome::Completed,
            verification: VerificationOutcome::NotApplicable,
            usage: Usage::default(),
            cost_usd: 0.0,
            started_at: finished_at,
            finished_at,
        });
    });
}

fn stale(handle: &SessionHandle) {
    finished(handle, CACHE_LIFETIME_MS + 60_000);
}

fn traced_now(handle: &SessionHandle, wanted: impl Fn(&DecisionRecord) -> bool) -> bool {
    handle.core.decisions.trace().recent(100).iter().any(wanted)
}

#[tokio::test]
async fn a_confident_size_sets_the_effort_only_while_the_user_leaves_it_unset() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, sized("complex"));
    route_new_task(&ctx, "refactor the parser into modules").await;
    assert_eq!(routed_effort(&handle.core), Some(Effort::High));
    let chosen = routes(&events);
    assert_eq!(chosen.len(), 1);
    assert_eq!(chosen[0].agent_id, AgentId::main());
    assert_eq!(
        (chosen[0].effort, chosen[0].model.clone()),
        (Some(Effort::High), None)
    );
    assert!(traced_now(&handle, |record| record.outcome == "effort high"));

    handle
        .core
        .with_state(|state| state.effort = Some(Effort::Low));
    stale(&handle);
    route_new_task(&ctx, "write the release notes").await;
    assert_eq!(
        routed_effort(&handle.core),
        None,
        "nothing to route: never asked"
    );
    assert_eq!(
        handle.core.with_state(|state| state.effort),
        Some(Effort::Low)
    );
    assert_eq!(routes(&events).len(), 1);
    handle.close("test").await;
}

#[tokio::test]
async fn off_shadow_down_and_unsure_route_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::Off, sized("simple"));
    route_new_task(&ctx, SMALL).await;
    assert!(handle.core.decisions.trace().recent(10).is_empty());
    assert!(handle.core.decisions.routes().current().is_none());

    install(&handle, FEATURE, FeatureMode::Shadow, sized("simple"));
    route_new_task(&ctx, SMALL).await;
    let would = |record: &DecisionRecord| record.shadow && record.outcome == "effort low";
    assert!(
        traced(&handle, would).await,
        "shadow records the route it would pick"
    );
    for model in [Scripted::down(), Scripted::default()] {
        install(&handle, FEATURE, FeatureMode::On, model);
        stale(&handle);
        route_new_task(&ctx, SMALL).await;
    }
    assert_eq!(routed_effort(&handle.core), None);
    assert!(routes(&events).is_empty());
    handle.close("test").await;
}

#[tokio::test]
async fn switching_models_needs_the_setting_and_a_small_request() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session_with(dir.path(), Some(SWITCHING)).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, sized("simple"));
    route_new_task(&ctx, SMALL).await;
    assert_eq!(ctx.model(), "fast-model");
    assert_eq!(routed_effort(&handle.core), Some(Effort::Low));
    assert_eq!(routes(&events)[0].model.as_deref(), Some("fast-model"));

    stale(&handle);
    route_new_task(&ctx, "update src/a.rs, src/b.rs, lib/c.ts and docs/d.md").await;
    assert_eq!(
        ctx.model(),
        "main-model",
        "a large request is never routed down"
    );
    assert_eq!(routed_effort(&handle.core), Some(Effort::Medium));
    assert!(traced_now(&handle, |record| record
        .override_reason
        .is_some()));

    handle
        .core
        .with_state(|state| state.model = "picked-by-user".into());
    stale(&handle);
    route_new_task(&ctx, SMALL).await;
    assert_eq!(ctx.model(), "picked-by-user");
    handle.close("test").await;

    let dir = tempfile::tempdir().unwrap();
    let plain = "[model]\nmain = \"main-model\"\nfast = \"fast-model\"\n";
    let (handle, _) = session_with(dir.path(), Some(plain)).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, sized("simple"));
    route_new_task(&ctx, SMALL).await;
    assert_eq!(ctx.model(), "main-model", "switching is off by default");
    assert_eq!(routed_effort(&handle.core), Some(Effort::Low));
    handle.close("test").await;
}

#[tokio::test]
async fn follow_ups_keep_the_route_and_a_new_task_records_the_old_ones_cost() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    let same_task = sized("simple").yes(NEW_TASK, false);
    install(&handle, FEATURE, FeatureMode::On, same_task);
    route_new_task(&ctx, SMALL).await;
    finished(&handle, 1_000);
    route_new_task(&ctx, FOLLOW_UP).await;
    route_new_task(&ctx, "yes, go on").await;
    assert_eq!(routed_effort(&handle.core), Some(Effort::Low));
    assert_eq!(routes(&events).len(), 1);
    assert!(traced_now(&handle, |record| record.outcome == "same task"));

    handle.core.with_state(|state| {
        state.usage.input_tokens = 100;
        state.usage.cache_read_tokens = 300;
        state.cost_usd = 0.25;
    });
    install(
        &handle,
        FEATURE,
        FeatureMode::On,
        sized("complex").yes(NEW_TASK, true),
    );
    route_new_task(&ctx, "now design the plugin system").await;
    assert_eq!(routed_effort(&handle.core), Some(Effort::High));
    let cost = |record: &DecisionRecord| {
        record.question == TASK_COST
            && record.outcome
                == "task of 3 turns: $0.2500, 75% of input from the prompt cache (effort low)"
    };
    assert!(traced_now(&handle, cost));
    handle.close("test").await;
}

#[tokio::test]
async fn a_stale_cache_starts_a_new_task_without_asking() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, sized("complex"));
    route_new_task(&ctx, "refactor the parser into modules").await;
    stale(&handle);
    install(&handle, FEATURE, FeatureMode::On, sized("simple"));
    route_new_task(&ctx, FOLLOW_UP).await;
    assert_eq!(routed_effort(&handle.core), Some(Effort::Low));
    assert!(!traced_now(&handle, |record| record.question == NEW_TASK));
    handle.close("test").await;
}

#[tokio::test]
async fn inheriting_subagents_take_the_fast_model_only_for_simple_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session_with(dir.path(), Some(SWITCHING)).await;
    let ctx = main_run(&handle);
    let helper = AgentId::new();
    install(&handle, FEATURE, FeatureMode::On, sized("simple"));
    let routed = route_subagent(&ctx, "explore", &helper, "find the config loader").await;
    assert_eq!(routed.as_deref(), Some("fast-model"));
    assert_eq!(routes(&events)[0].agent_id, helper);
    install(&handle, FEATURE, FeatureMode::On, sized("complex"));
    assert_eq!(
        route_subagent(&ctx, "explore", &helper, "audit auth").await,
        None
    );
    install(&handle, FEATURE, FeatureMode::Shadow, sized("simple"));
    assert_eq!(
        route_subagent(&ctx, "explore", &helper, "find it").await,
        None
    );
    handle.close("test").await;

    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, sized("simple"));
    assert_eq!(
        route_subagent(&ctx, "explore", &helper, "find it").await,
        None
    );
    handle.close("test").await;
}
