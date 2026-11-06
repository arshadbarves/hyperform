//! Check selection only skips what a confident "no" covers, never when a
//! test file changed, and never in `off`, `shadow` or with a model that is
//! down or unsure.

use std::path::PathBuf;

use z_engine_config::{FeatureId, FeatureMode};
use z_engine_decisions::DecisionRecord;
use z_engine_protocol::CheckKind;
use z_engine_verify::{CheckSource, CheckSpec, DEFAULT_CHECK_TIMEOUT_SECS};

use super::{QUESTION, skips};
use crate::decisions::seams::select_needed;
use crate::decisions::uses::scripted::{Scripted, install, main_run, session, traced};

const FEATURE: FeatureId = FeatureId::DecisionsCheckSelect;

fn spec(id: &str, kind: CheckKind) -> CheckSpec {
    CheckSpec {
        id: id.into(),
        label: id.into(),
        kind,
        command: id.into(),
        cwd: PathBuf::from("."),
        source: CheckSource::Configured,
        timeout_secs: DEFAULT_CHECK_TIMEOUT_SECS,
    }
}

fn checks() -> [CheckSpec; 3] {
    [
        spec("cargo:test", CheckKind::Test),
        spec("npm:lint", CheckKind::Lint),
        spec("custom:e2e", CheckKind::Custom),
    ]
}

fn ids(selected: &[&CheckSpec]) -> Vec<String> {
    selected.iter().map(|check| check.id.clone()).collect()
}

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

#[test]
fn one_unsure_answer_keeps_every_check() {
    assert_eq!(skips(&["a", "b"], &[Some(false), Some(true)]), ["a"]);
    assert!(skips(&["a", "b"], &[Some(false), None]).is_empty());
    assert!(skips(&["a"], &[]).is_empty());
}

#[tokio::test]
async fn a_confident_no_skips_only_checks_no_changed_file_belongs_to() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let ctx = main_run(&handle);
    let all = checks();
    let rust = paths(&["src/lib.rs"]);
    install(
        &handle,
        FEATURE,
        FeatureMode::On,
        Scripted::default().yes(QUESTION, false),
    );
    let kept = select_needed(&ctx, all.iter().collect(), &rust, false).await;
    assert_eq!(ids(&kept), ["cargo:test"]);
    let strict = select_needed(&ctx, all.iter().collect(), &paths(&["README.md"]), true).await;
    assert_eq!(ids(&strict), ["cargo:test"], "strict keeps its Test check");
    let tests = paths(&["src/lib.rs", "tests/parser.rs"]);
    let kept = select_needed(&ctx, all.iter().collect(), &tests, false).await;
    assert_eq!(kept.len(), 3, "a changed test file runs everything");
    let records = handle.core.decisions.trace().recent(20);
    assert!(records.iter().any(|record| record.outcome == "skipped"));
    handle.close("test").await;
}

#[tokio::test]
async fn off_down_unsure_shadow_and_yes_keep_every_check() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let ctx = main_run(&handle);
    let all = checks();
    let rust = paths(&["src/lib.rs"]);
    for (mode, model) in [
        (FeatureMode::Off, Scripted::default().yes(QUESTION, false)),
        (FeatureMode::On, Scripted::down()),
        (FeatureMode::On, Scripted::default()),
        (FeatureMode::On, Scripted::default().yes(QUESTION, true)),
        (
            FeatureMode::Shadow,
            Scripted::default().yes(QUESTION, false),
        ),
    ] {
        install(&handle, FEATURE, mode, model);
        let kept = select_needed(&ctx, all.iter().collect(), &rust, false).await;
        assert_eq!(kept.len(), 3, "{mode:?}");
    }
    let would_skip = |record: &DecisionRecord| record.shadow && record.outcome == "skipped";
    assert!(traced(&handle, would_skip).await);
    handle.close("test").await;
}
