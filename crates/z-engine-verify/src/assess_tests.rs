use z_engine_protocol::{AgentId, CheckKind, CheckRecord, TestCounts, VerificationOutcome};

use super::assess;

fn record(check_id: &str, kind: CheckKind, passed: bool, started_at: u64) -> CheckRecord {
    CheckRecord {
        record_id: format!("chk_{check_id}_{started_at}"),
        check_id: check_id.to_string(),
        label: check_id.replace(':', " "),
        kind,
        command: check_id.replace(':', " "),
        cwd: ".".into(),
        agent_id: AgentId::main(),
        exit_code: Some(if passed { 0 } else { 1 }),
        passed,
        timed_out: false,
        started_at,
        duration_ms: 10,
        tests: None,
        artifact: None,
        fingerprint_before: "fp0".into(),
        fingerprint_after: "fp1".into(),
        output_tail: String::new(),
    }
}

fn unverified(reason: &str) -> VerificationOutcome {
    VerificationOutcome::Unverified {
        reason: reason.into(),
    }
}

fn verified(ids: &[&str]) -> VerificationOutcome {
    VerificationOutcome::Verified {
        checks: ids.iter().map(|id| id.to_string()).collect(),
    }
}

#[test]
fn rules_table() {
    let test = record("cargo:test", CheckKind::Test, true, 100);
    let build = record("cargo:build", CheckKind::Build, true, 110);
    let lint = record("cargo:clippy", CheckKind::Lint, true, 120);
    let failed_test = record("cargo:test", CheckKind::Test, false, 90);
    let stale_build = CheckRecord {
        fingerprint_after: "old".into(),
        ..record("cargo:build", CheckKind::Build, true, 130)
    };
    let no_checks = unverified("no checks have run since the last change");
    let stale = unverified("checks are stale: files changed after they ran");
    let lint_only = unverified("only lint/format checks ran; run tests or a build");
    type Case<'a> = (
        &'a str,
        Vec<CheckRecord>,
        bool,
        Option<u64>,
        Option<&'a str>,
        VerificationOutcome,
    );
    let cases: Vec<Case> = vec![
        (
            "nothing changed",
            vec![failed_test.clone()],
            false,
            None,
            None,
            VerificationOutcome::NotApplicable,
        ),
        (
            "changed without checks",
            vec![],
            true,
            None,
            None,
            no_checks.clone(),
        ),
        (
            "checks older than the change",
            vec![test.clone()],
            true,
            Some(101),
            None,
            no_checks,
        ),
        (
            "a passing test",
            vec![test.clone()],
            true,
            Some(100),
            Some("fp1"),
            verified(&["chk_cargo:test_100"]),
        ),
        (
            "fingerprint unknown",
            vec![test.clone(), lint.clone()],
            true,
            None,
            None,
            verified(&["chk_cargo:test_100", "chk_cargo:clippy_120"]),
        ),
        (
            "the latest run of a check wins",
            vec![failed_test.clone(), test.clone()],
            true,
            None,
            Some("fp1"),
            verified(&["chk_cargo:test_100"]),
        ),
        (
            "a later failure wins",
            vec![
                test.clone(),
                CheckRecord {
                    started_at: 200,
                    ..failed_test.clone()
                },
            ],
            true,
            None,
            None,
            VerificationOutcome::Failed {
                reason: "cargo test failed (exit 1)".into(),
            },
        ),
        (
            "all stale",
            vec![test.clone()],
            true,
            None,
            Some("fp9"),
            stale,
        ),
        (
            "only lint is fresh",
            vec![
                lint.clone(),
                CheckRecord {
                    fingerprint_after: "old".into(),
                    ..test.clone()
                },
            ],
            true,
            None,
            Some("fp1"),
            lint_only.clone(),
        ),
        (
            "only lint ran",
            vec![lint.clone()],
            true,
            None,
            None,
            lint_only,
        ),
        (
            "stale records are not evidence",
            vec![build.clone(), stale_build.clone(), test.clone()],
            true,
            None,
            Some("fp1"),
            verified(&["chk_cargo:test_100"]),
        ),
        (
            "fresh build with a stale lint",
            vec![
                build.clone(),
                CheckRecord {
                    fingerprint_after: "old".into(),
                    ..lint.clone()
                },
            ],
            true,
            None,
            Some("fp1"),
            verified(&["chk_cargo:build_110"]),
        ),
    ];
    for (name, records, mutated, since, now, expected) in cases {
        assert_eq!(assess(&records, mutated, since, now), expected, "{name}");
    }
}

#[test]
fn failure_reasons_name_exit_codes_tests_and_timeouts() {
    let mut test = record("web/npm:test", CheckKind::Test, false, 10);
    test.label = "npm test (web)".into();
    test.tests = Some(TestCounts {
        passed: 4,
        failed: 2,
        skipped: 0,
    });
    let mut build = record("cargo:build", CheckKind::Build, false, 11);
    build.exit_code = Some(101);
    let mut slow = record("pytest", CheckKind::Test, false, 12);
    slow.label = String::new();
    slow.exit_code = None;
    slow.timed_out = true;
    let mut single = record("go:test", CheckKind::Test, false, 13);
    single.exit_code = Some(0);
    single.tests = Some(TestCounts {
        passed: 3,
        failed: 1,
        skipped: 0,
    });
    let records = vec![
        test,
        build,
        slow,
        single,
        record("make:lint", CheckKind::Lint, false, 14),
    ];
    let VerificationOutcome::Failed { reason } = assess(&records, true, None, None) else {
        panic!("expected a failure");
    };
    assert_eq!(
        reason,
        "npm test (web) failed (exit 1, 2 tests failed); cargo build failed (exit 101); \
         pytest failed (timed out); 2 more failed"
    );
    let VerificationOutcome::Failed { reason } = assess(&records[3..4], true, None, None) else {
        panic!("expected a failure");
    };
    assert_eq!(reason, "go test failed (exit 0, 1 test failed)");
}
