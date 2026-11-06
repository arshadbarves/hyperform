//! Background shells: spawn, incremental and filtered reads, waiting,
//! kills, events, overflow, and cleanup on drop.
#![cfg(unix)]

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;
use support::{eventually_dead, wait_for_file};
use z_engine_host::{
    BackgroundShells, BackgroundSpec, EnvPolicy, HostError, JobEvent, JobEventSink, resolve_shell,
};
use z_engine_protocol::{JobId, JobStatus};

const WAIT: Duration = Duration::from_secs(10);

fn spec(command: &str, cwd: &Path) -> BackgroundSpec {
    BackgroundSpec {
        command: command.to_string(),
        cwd: cwd.to_path_buf(),
        shell: resolve_shell(None),
        env: EnvPolicy::default(),
        label: command.to_string(),
        owner: "main".to_string(),
    }
}

async fn until_finished(shells: &BackgroundShells, id: &JobId) {
    for _ in 0..200 {
        if shells.snapshot(id).is_some_and(|s| s.status.is_terminal()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("job {id} never finished");
}

#[tokio::test]
async fn spawn_wait_and_read_new() {
    let dir = tempfile::tempdir().unwrap();
    let shells = BackgroundShells::new(None);
    let id = shells
        .spawn(spec("echo hello; sleep 0.2; echo world", dir.path()))
        .await
        .unwrap();
    let read = shells.wait(&id, WAIT).await.unwrap();
    assert_eq!(read.status, JobStatus::Completed);
    assert_eq!(read.exit_code, Some(0));
    assert_eq!(read.output, "hello\nworld\n");
    assert_eq!(read.dropped_bytes, 0);
    assert_eq!(shells.read_new(&id, None).unwrap().output, "");

    let snapshot = shells.snapshot(&id).unwrap();
    assert_eq!(snapshot.tail, "hello\nworld\n");
    assert_eq!(snapshot.owner, "main");
    assert!(
        snapshot
            .finished_at
            .is_some_and(|end| end >= snapshot.started_at)
    );
}

#[tokio::test]
async fn reads_are_incremental_and_waits_time_out_without_error() {
    let dir = tempfile::tempdir().unwrap();
    let shells = BackgroundShells::new(None);
    let command = "echo first; while [ ! -f go ]; do sleep 0.05; done; echo second";
    let id = shells.spawn(spec(command, dir.path())).await.unwrap();

    // Short waits that time out are not errors; output accumulates.
    let mut early = String::new();
    for _ in 0..50 {
        let read = shells.wait(&id, Duration::from_millis(100)).await.unwrap();
        assert_eq!(read.status, JobStatus::Running);
        early.push_str(&read.output);
        if !early.is_empty() {
            break;
        }
    }
    assert_eq!(early, "first\n");

    std::fs::write(dir.path().join("go"), "").unwrap();
    let late = shells.wait(&id, WAIT).await.unwrap();
    assert_eq!(late.status, JobStatus::Completed);
    assert_eq!(late.output, "second\n");
}

#[tokio::test]
async fn filters_keep_matching_lines_and_consume_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let shells = BackgroundShells::new(None);
    let id = shells
        .spawn(spec(
            "printf 'error: a\\ninfo: b\\nerror: c\\n'",
            dir.path(),
        ))
        .await
        .unwrap();
    until_finished(&shells, &id).await;
    let errors = Regex::new("^error").unwrap();
    let read = shells.read_new(&id, Some(&errors)).unwrap();
    assert_eq!(read.output, "error: a\nerror: c\n");
    assert_eq!(shells.read_new(&id, None).unwrap().output, "");
}

#[tokio::test]
async fn kill_stops_the_whole_tree_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let shells = BackgroundShells::new(None);
    let command = "sh -c 'echo $$ > inner.pid; exec sleep 30' & sleep 30";
    let id = shells.spawn(spec(command, dir.path())).await.unwrap();
    let pid = wait_for_file(&dir.path().join("inner.pid")).await;

    let started = Instant::now();
    shells.kill(&id).await.unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    let snapshot = shells.snapshot(&id).unwrap();
    assert_eq!(snapshot.status, JobStatus::Killed);
    assert!(eventually_dead(&pid).await, "grandchild {pid} survived");
    shells.kill(&id).await.unwrap();
    assert_eq!(shells.snapshot(&id).unwrap().status, JobStatus::Killed);
}

#[tokio::test]
async fn failures_and_events_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let events = Arc::new(Mutex::new(Vec::<JobEvent>::new()));
    let sink_events = Arc::clone(&events);
    let sink: JobEventSink = Arc::new(move |event| sink_events.lock().unwrap().push(event));
    let shells = BackgroundShells::new(Some(sink));
    let id = shells
        .spawn(spec("echo boom; exit 7", dir.path()))
        .await
        .unwrap();
    let read = shells.wait(&id, WAIT).await.unwrap();
    assert_eq!((read.status, read.exit_code), (JobStatus::Failed, Some(7)));

    let events = events.lock().unwrap().clone();
    assert_eq!(
        events,
        vec![
            JobEvent::Output {
                id: id.clone(),
                text: "boom\n".to_string()
            },
            JobEvent::Exited {
                id,
                status: JobStatus::Failed,
                exit_code: Some(7)
            },
        ]
    );
}

#[tokio::test]
async fn overflow_beyond_one_mebibyte_is_counted() {
    let dir = tempfile::tempdir().unwrap();
    let shells = BackgroundShells::new(None);
    let command = "head -c 3000000 /dev/zero | tr '\\0' 'a'";
    let id = shells.spawn(spec(command, dir.path())).await.unwrap();
    let read = shells.wait(&id, WAIT).await.unwrap();
    assert_eq!(read.status, JobStatus::Completed);
    assert_eq!(read.output.len(), 1024 * 1024);
    assert_eq!(read.dropped_bytes, 3_000_000 - 1024 * 1024);
}

#[tokio::test]
async fn list_orders_jobs_and_unknown_ids_are_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let shells = BackgroundShells::new(None);
    let first = shells.spawn(spec("sleep 30", dir.path())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;
    let second = shells.spawn(spec("echo quick", dir.path())).await.unwrap();
    let ids: Vec<JobId> = shells.list().into_iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![first.clone(), second]);

    let unknown = JobId::from("job_missing");
    assert!(shells.snapshot(&unknown).is_none());
    assert!(matches!(
        shells.read_new(&unknown, None),
        Err(HostError::NotFound(_))
    ));
    assert!(shells.kill(&unknown).await.unwrap_err().is_not_found());

    shells.kill_all().await;
    assert_eq!(shells.snapshot(&first).unwrap().status, JobStatus::Killed);
}

#[tokio::test]
async fn dropping_the_last_handle_kills_running_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let shells = BackgroundShells::new(None);
    let command = "sh -c 'echo $$ > inner.pid; exec sleep 30' & sleep 30";
    shells.spawn(spec(command, dir.path())).await.unwrap();
    let pid = wait_for_file(&dir.path().join("inner.pid")).await;
    drop(shells);
    assert!(eventually_dead(&pid).await, "job {pid} outlived its owner");
}
