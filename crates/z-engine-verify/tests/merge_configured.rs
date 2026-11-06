mod support;

use std::path::PathBuf;

use support::{Fixture, check, has_note, ids};
use z_engine_protocol::CheckKind;
use z_engine_verify::{CheckSource, ConfiguredCheck, DEFAULT_CHECK_TIMEOUT_SECS, merge_configured};

fn configured(id: &str, command: &str, kind: CheckKind) -> ConfiguredCheck {
    ConfiguredCheck {
        id: id.to_string(),
        label: String::new(),
        command: command.to_string(),
        kind,
        cwd: None,
        timeout_secs: 0,
    }
}

#[test]
fn configured_checks_override_by_id_and_are_added_otherwise() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = \"app\"\n"),
        ("web/package.json", r#"{"scripts":{"test":"vitest run"}}"#),
    ]);
    fixture.mkdir("e2e");
    let mut profile = fixture.discover();
    let docs = fixture.path().join("docs");
    merge_configured(
        &mut profile,
        &[
            configured("cargo:test", "cargo nextest run", CheckKind::Test),
            ConfiguredCheck {
                label: "End-to-end".into(),
                cwd: Some("./e2e/".into()),
                timeout_secs: 900,
                ..configured("e2e", "npm run e2e", CheckKind::Test)
            },
            ConfiguredCheck {
                cwd: Some(docs.display().to_string()),
                timeout_secs: 30,
                ..configured("docs:links", "lychee docs", CheckKind::Custom)
            },
            configured("custom:e2e", "npx playwright test", CheckKind::Test),
        ],
        fixture.path(),
    );
    assert_eq!(
        ids(&profile),
        [
            "cargo:test",
            "cargo:build",
            "cargo:clippy",
            "cargo:fmt",
            "web/npm:test",
            "custom:e2e",
            "docs:links",
        ]
    );
    let test = check(&profile, "cargo:test");
    assert_eq!(test.command, "cargo nextest run");
    assert_eq!(test.label, "cargo nextest run");
    assert_eq!(test.source, CheckSource::Configured);
    assert_eq!(test.cwd, PathBuf::from("."));
    assert_eq!(test.timeout_secs, DEFAULT_CHECK_TIMEOUT_SECS);

    // The later `custom:e2e` replaced the earlier bare `e2e`.
    let e2e = check(&profile, "custom:e2e");
    assert_eq!(e2e.command, "npx playwright test");
    assert_eq!(e2e.cwd, PathBuf::from("."));

    let links = check(&profile, "docs:links");
    assert_eq!(links.cwd, PathBuf::from("docs"));
    assert_eq!(links.kind, CheckKind::Custom);
    assert_eq!(links.timeout_secs, 30);
    assert!(has_note(
        &profile,
        "configured check `docs:links`: directory `docs` does not exist"
    ));
}

#[test]
fn configured_directories_and_labels() {
    let fixture = Fixture::new(&[]);
    fixture.mkdir("e2e");
    let mut profile = fixture.discover();
    merge_configured(
        &mut profile,
        &[ConfiguredCheck {
            label: "End-to-end".into(),
            cwd: Some("./e2e/".into()),
            timeout_secs: 900,
            ..configured("e2e", "npm run e2e", CheckKind::Test)
        }],
        fixture.path(),
    );
    let e2e = check(&profile, "custom:e2e");
    assert_eq!(e2e.label, "End-to-end");
    assert_eq!(e2e.cwd, PathBuf::from("e2e"));
    assert_eq!(e2e.timeout_secs, 900);
    assert!(profile.notes.is_empty(), "{:?}", profile.notes);
}

#[test]
fn checks_without_an_id_or_command_are_ignored_with_a_note() {
    let fixture = Fixture::new(&[]);
    let mut profile = fixture.discover();
    merge_configured(
        &mut profile,
        &[
            configured("  ", "make smoke", CheckKind::Test),
            configured("smoke", "   ", CheckKind::Test),
        ],
        fixture.path(),
    );
    assert!(profile.checks.is_empty());
    assert!(has_note(&profile, "configured check `` was ignored"));
    assert!(has_note(&profile, "configured check `smoke` was ignored"));
}
