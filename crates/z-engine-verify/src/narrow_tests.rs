use std::path::{Path, PathBuf};

use z_engine_protocol::CheckKind;

use super::{is_test_path, owns_change, skip_checks, touches_tests};
use crate::{CheckSource, CheckSpec, DEFAULT_CHECK_TIMEOUT_SECS};

fn spec(id: &str, kind: CheckKind) -> CheckSpec {
    CheckSpec {
        id: id.to_string(),
        label: id.to_string(),
        kind,
        command: id.to_string(),
        cwd: PathBuf::from("."),
        source: CheckSource::Configured,
        timeout_secs: DEFAULT_CHECK_TIMEOUT_SECS,
    }
}

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

fn ids(checks: &[&CheckSpec]) -> Vec<String> {
    checks.iter().map(|check| check.id.clone()).collect()
}

#[test]
fn test_files_are_recognized_across_ecosystems() {
    for test in [
        "tests/flow.rs",
        "crates/a/src/select_tests.rs",
        "pkg/handler_test.go",
        "app/test_views.py",
        "conftest.py",
        "web/src/app.test.ts",
        "web/src/app.spec.tsx",
        "src/__tests__/a.js",
        "src/main/java/FooTest.java",
        "Foo.Tests/BarTests.cs",
    ] {
        assert!(is_test_path(Path::new(test)), "{test}");
    }
    for source in [
        "src/lib.rs",
        "src/latest.rs",
        "docs/testing.md",
        "README.md",
    ] {
        assert!(!is_test_path(Path::new(source)), "{source}");
    }
    assert!(touches_tests(&paths(&["src/lib.rs", "tests/a.rs"])));
    assert!(!touches_tests(&paths(&["src/lib.rs"])));
}

#[test]
fn a_check_owns_the_files_of_its_ecosystem() {
    let cargo = spec("cargo:test", CheckKind::Test);
    let npm = spec("web/npm:lint", CheckKind::Lint);
    let custom = spec("custom:e2e", CheckKind::Custom);
    assert!(owns_change(&cargo, &paths(&["src/lib.rs"])));
    assert!(owns_change(&cargo, &paths(&["Cargo.toml"])));
    assert!(!owns_change(&cargo, &paths(&["README.md", "web/app.ts"])));
    assert!(owns_change(&npm, &paths(&["web/src/App.svelte"])));
    assert!(!owns_change(&npm, &paths(&["src/lib.rs"])));
    assert!(!owns_change(&custom, &paths(&["src/lib.rs"])));
    let go = spec("go:test", CheckKind::Test);
    assert!(owns_change(&go, &paths(&["go.sum"])));
}

#[test]
fn skips_keep_order_and_strict_keeps_its_evidence() {
    let checks = [
        spec("cargo:test", CheckKind::Test),
        spec("cargo:clippy", CheckKind::Lint),
        spec("npm:build", CheckKind::Build),
    ];
    let selected = || checks.iter().collect::<Vec<_>>();
    assert_eq!(ids(&skip_checks(selected(), &[], true)), ids(&selected()));
    let lint = ["cargo:clippy".to_string()];
    assert_eq!(
        ids(&skip_checks(selected(), &lint, false)),
        ["cargo:test", "npm:build"]
    );
    let all: Vec<String> = ids(&selected());
    assert!(skip_checks(selected(), &all, false).is_empty());
    assert_eq!(
        ids(&skip_checks(selected(), &all, true)),
        ["cargo:test", "npm:build"]
    );
    let test_only = ["cargo:test".to_string()];
    assert_eq!(
        ids(&skip_checks(selected(), &test_only, true)),
        ["cargo:clippy", "npm:build"],
        "a remaining build still proves the change"
    );
}
