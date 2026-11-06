use std::fs;

use z_engine_protocol::PlanId;
use z_engine_protocol::work::{
    Actor, PlanDepth, PlanStatus, PlanSummary, WorkCause, WorkEvent, WorkRecord,
};

use super::*;
use crate::error::StoreError;

const KEY: &str = "0123456789abcdef";

fn summary(id: &str, updated_at: u64) -> PlanSummary {
    PlanSummary {
        plan_id: PlanId::from(id),
        project_root: "/p".into(),
        goal: "Ship it".into(),
        status: PlanStatus::Draft,
        depth: PlanDepth::Standard,
        revision: 1,
        tasks_total: 0,
        tasks_done: 0,
        created_at: 1,
        updated_at,
        blocked: 0,
        needs_you: 0,
        running: 0,
        last_session_id: None,
        attached_to: None,
    }
}

fn record(seq: u64, event: WorkEvent) -> WorkRecord {
    WorkRecord {
        seq,
        revision: seq,
        at: seq * 10,
        actor: Actor::User,
        cause: WorkCause::Edit,
        event,
    }
}

fn created(id: &str) -> WorkRecord {
    record(
        1,
        WorkEvent::PlanCreated {
            plan_id: PlanId::from(id),
            project_root: "/p".into(),
            goal: "Ship it".into(),
            document: String::new(),
            depth: PlanDepth::Standard,
            status: PlanStatus::Draft,
        },
    )
}

fn approved(seq: u64) -> WorkRecord {
    record(
        seq,
        WorkEvent::PlanStatusChanged {
            from: PlanStatus::Draft,
            to: PlanStatus::Active,
        },
    )
}

#[test]
fn records_round_trip_through_create_append_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlanStore::new(dir.path());
    let id = PlanId::from("pln_a");
    let mut log = store
        .create(KEY, &[created("pln_a")], &summary("pln_a", 1))
        .unwrap();
    assert_eq!(
        log.path(),
        dir.path().join(KEY).join("pln_a").join("events.jsonl")
    );
    log.append(&approved(2)).unwrap();
    log.sync().unwrap();
    drop(log);
    store
        .open_append(KEY, &id)
        .unwrap()
        .append(&approved(3))
        .unwrap();
    let loaded = store.load(KEY, &id).unwrap();
    assert_eq!(
        loaded.records,
        vec![created("pln_a"), approved(2), approved(3)]
    );
    assert!(!loaded.torn_tail);
    assert_eq!(loaded.corrupt_lines, 0);
    assert!(store.exists(KEY, &id));
    assert!(!store.exists(KEY, &PlanId::from("pln_b")));
}

#[test]
fn creating_a_plan_twice_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlanStore::new(dir.path());
    store
        .create(KEY, &[created("pln_a")], &summary("pln_a", 1))
        .unwrap();
    assert!(matches!(
        store.create(KEY, &[created("pln_a")], &summary("pln_a", 1)),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn a_torn_tail_is_ignored_on_load_and_cut_on_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlanStore::new(dir.path());
    let id = PlanId::from("pln_a");
    let log = store
        .create(KEY, &[created("pln_a")], &summary("pln_a", 1))
        .unwrap();
    let path = log.path().to_path_buf();
    drop(log);
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("{\"seq\":2,\"revision\":2,\"at\":");
    fs::write(&path, text).unwrap();
    let loaded = store.load(KEY, &id).unwrap();
    assert_eq!((loaded.records.len(), loaded.torn_tail), (1, true));
    store
        .open_append(KEY, &id)
        .unwrap()
        .append(&approved(2))
        .unwrap();
    let loaded = store.load(KEY, &id).unwrap();
    assert_eq!(loaded.records, vec![created("pln_a"), approved(2)]);
    assert!(!loaded.torn_tail);
}

#[test]
fn unknown_record_kinds_cost_one_line() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlanStore::new(dir.path());
    let id = PlanId::from("pln_a");
    let log = store
        .create(KEY, &[created("pln_a")], &summary("pln_a", 1))
        .unwrap();
    let path = log.path().to_path_buf();
    drop(log);
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str(
        "{\"seq\":2,\"revision\":2,\"at\":20,\"actor\":{\"type\":\"user\"},\"cause\":{\"type\":\"edit\"},\"event\":{\"type\":\"fromTheFuture\"}}\n",
    );
    text.push_str("not json\n");
    fs::write(&path, text).unwrap();
    store
        .open_append(KEY, &id)
        .unwrap()
        .append(&approved(4))
        .unwrap();
    let loaded = store.load(KEY, &id).unwrap();
    assert_eq!(loaded.records, vec![created("pln_a"), approved(4)]);
    assert_eq!(loaded.corrupt_lines, 2);
}

#[test]
fn meta_round_trips_and_lists_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlanStore::new(dir.path());
    assert!(store.list(KEY).unwrap().is_empty());
    store
        .create(KEY, &[created("pln_a")], &summary("pln_a", 5))
        .unwrap();
    store
        .create(KEY, &[created("pln_b")], &summary("pln_b", 9))
        .unwrap();
    store
        .create(KEY, &[created("pln_c")], &summary("pln_c", 7))
        .unwrap();
    let mut updated = summary("pln_a", 20);
    updated.status = PlanStatus::Active;
    store.write_meta(KEY, &updated).unwrap();
    assert_eq!(
        store.read_meta(KEY, &PlanId::from("pln_a")).unwrap(),
        updated
    );
    fs::write(dir.path().join(KEY).join("pln_c").join("meta.json"), "{").unwrap();
    fs::create_dir(dir.path().join(KEY).join("not-a-plan")).unwrap();
    let ids: Vec<String> = store
        .list(KEY)
        .unwrap()
        .into_iter()
        .map(|s| s.plan_id.0)
        .collect();
    assert_eq!(ids, ["pln_a", "pln_b"]);
    let all: Vec<String> = store
        .plan_ids(KEY)
        .unwrap()
        .into_iter()
        .map(|id| id.0)
        .collect();
    assert_eq!(all, ["pln_a", "pln_b", "pln_c"]);
    assert!(matches!(
        store.write_meta(KEY, &summary("pln_z", 1)),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn meta_from_before_the_plan_counts_reads_as_zero() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlanStore::new(dir.path());
    store
        .create(KEY, &[created("pln_a")], &summary("pln_a", 1))
        .unwrap();
    let path = dir.path().join(KEY).join("pln_a").join("meta.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let fields = value.as_object_mut().unwrap();
    for key in [
        "blocked",
        "needsYou",
        "running",
        "lastSessionId",
        "attachedTo",
    ] {
        assert!(fields.remove(key).is_some(), "{key} was not written");
    }
    fs::write(&path, value.to_string()).unwrap();
    let mut counted = summary("pln_a", 1);
    assert_eq!(
        store.read_meta(KEY, &PlanId::from("pln_a")).unwrap(),
        counted
    );
    counted.needs_you = 2;
    counted.last_session_id = Some("ses_1".into());
    store.write_meta(KEY, &counted).unwrap();
    assert_eq!(
        store.read_meta(KEY, &PlanId::from("pln_a")).unwrap(),
        counted
    );
}

#[test]
fn meta_from_a_newer_schema_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlanStore::new(dir.path());
    store
        .create(KEY, &[created("pln_a")], &summary("pln_a", 1))
        .unwrap();
    let path = dir.path().join(KEY).join("pln_a").join("meta.json");
    let text = fs::read_to_string(&path).unwrap().replace(
        &format!("\"schema\": {PLAN_SCHEMA}"),
        &format!("\"schema\": {}", PLAN_SCHEMA + 1),
    );
    fs::write(&path, text).unwrap();
    assert!(matches!(
        store.read_meta(KEY, &PlanId::from("pln_a")),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn keys_and_ids_must_be_path_segments() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlanStore::new(dir.path());
    for bad in ["", "..", "a/b", "../up"] {
        assert!(store.project_dir(bad).is_err(), "{bad:?} accepted");
        assert!(
            store.plan_dir(KEY, &PlanId::from(bad)).is_err(),
            "{bad:?} accepted"
        );
    }
    assert!(store.create("../x", &[], &summary("pln_a", 1)).is_err());
    assert!(matches!(
        store.load(KEY, &PlanId::from("pln_none")),
        Err(StoreError::NotFound(_))
    ));
}
