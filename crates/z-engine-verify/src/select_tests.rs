use std::path::PathBuf;

use z_engine_protocol::CheckKind;

use super::select_checks;
use crate::{CheckSource, CheckSpec, DEFAULT_CHECK_TIMEOUT_SECS, ProjectProfile};

fn spec(id: &str, kind: CheckKind, cwd: &str) -> CheckSpec {
    CheckSpec {
        id: id.to_string(),
        label: id.to_string(),
        kind,
        command: id.to_string(),
        cwd: PathBuf::from(cwd),
        source: CheckSource::Configured,
        timeout_secs: DEFAULT_CHECK_TIMEOUT_SECS,
    }
}

fn profile() -> ProjectProfile {
    ProjectProfile {
        roots: Vec::new(),
        checks: vec![
            spec("cargo:test", CheckKind::Test, "."),
            spec("cargo:clippy", CheckKind::Lint, "."),
            spec("web/npm:test", CheckKind::Test, "web"),
            spec("web/npm:lint", CheckKind::Lint, "web"),
            spec("packages/a/pnpm:test", CheckKind::Test, "packages/a"),
            spec("packages/a/pnpm:build", CheckKind::Build, "packages/a"),
        ],
        notes: Vec::new(),
    }
}

fn ids<'a>(checks: &[&'a CheckSpec]) -> Vec<&'a str> {
    checks.iter().map(|c| c.id.as_str()).collect()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

fn paths(values: &[&str]) -> Vec<PathBuf> {
    values.iter().map(PathBuf::from).collect()
}

#[test]
fn kinds_ids_and_unrooted_ids_select_in_profile_order() {
    let profile = profile();
    assert_eq!(
        ids(&select_checks(&profile, &strings(&["tests"]), &[])),
        ["cargo:test", "web/npm:test", "packages/a/pnpm:test"]
    );
    assert_eq!(
        ids(&select_checks(
            &profile,
            &strings(&["web/npm:lint", "build"]),
            &[]
        )),
        ["web/npm:lint", "packages/a/pnpm:build"]
    );
    assert_eq!(
        ids(&select_checks(&profile, &strings(&["npm:test"]), &[])),
        ["web/npm:test"]
    );
    assert_eq!(select_checks(&profile, &[], &[]).len(), 6);
    assert!(select_checks(&profile, &strings(&["deploy", " "]), &[]).is_empty());
}

#[test]
fn changed_paths_pick_the_deepest_root() {
    let profile = profile();
    let test = strings(&["test"]);
    assert_eq!(
        ids(&select_checks(&profile, &test, &paths(&["web/src/app.ts"]))),
        ["web/npm:test"]
    );
    assert_eq!(
        ids(&select_checks(
            &profile,
            &test,
            &paths(&["./packages/a/src/x.ts", "src/lib.rs"])
        )),
        ["cargo:test", "packages/a/pnpm:test"]
    );
    assert_eq!(
        ids(&select_checks(&profile, &test, &paths(&["README.md"]))),
        ["cargo:test"]
    );
}

#[test]
fn falls_back_to_every_match_when_no_root_holds_a_change() {
    let profile = profile();
    let build = strings(&["build"]);
    // `packages/a` is the only root with a build, and the change is elsewhere.
    assert_eq!(
        ids(&select_checks(&profile, &build, &paths(&["web/x.ts"]))),
        ["packages/a/pnpm:build"]
    );
    let nested_only = ProjectProfile {
        checks: profile.checks[2..].to_vec(),
        ..profile.clone()
    };
    assert_eq!(
        ids(&select_checks(
            &nested_only,
            &strings(&["test"]),
            &paths(&["docs/guide.md", "/abs/x", "../up"])
        )),
        ["web/npm:test", "packages/a/pnpm:test"]
    );
}

#[test]
fn configured_checks_match_their_own_id() {
    let mut profile = profile();
    profile
        .checks
        .push(spec("custom:unit", CheckKind::Custom, "."));
    assert_eq!(
        ids(&select_checks(&profile, &strings(&["unit"]), &[])),
        ["custom:unit"]
    );
    assert_eq!(
        ids(&select_checks(&profile, &strings(&["custom:unit"]), &[])),
        ["custom:unit"]
    );
}

#[test]
fn duplicate_ids_are_selected_once() {
    let mut profile = profile();
    profile
        .checks
        .push(spec("cargo:test", CheckKind::Test, "."));
    assert_eq!(
        ids(&select_checks(
            &profile,
            &strings(&["test", "cargo:test"]),
            &[]
        )),
        ["cargo:test", "web/npm:test", "packages/a/pnpm:test"]
    );
}
