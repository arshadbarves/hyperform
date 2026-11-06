mod support;

use support::{Fixture, has_note, roots};
use z_engine_verify::{DiscoveryOptions, discover};

#[test]
fn depth_limit_is_visible() {
    let fixture = Fixture::new(&[
        ("a/b/c/d/package.json", "{}"),
        ("a/b/c/d/e/go.mod", "module example.com/deep\n"),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [("a/b/c/d", "node")]);
    assert!(has_note(
        &profile,
        "more than 4 levels below the root were not scanned"
    ));

    let shallow = fixture.discover_with(DiscoveryOptions {
        max_depth: 0,
        ..DiscoveryOptions::default()
    });
    assert!(shallow.roots.is_empty());
}

#[test]
fn entry_limit_stops_the_walk() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = \"first\"\n"),
        ("zz/package.json", "{}"),
    ]);
    for i in 0..50 {
        fixture.write(&format!("files/f{i:02}.txt"), "");
    }
    let profile = fixture.discover_with(DiscoveryOptions {
        max_entries: 10,
        ..DiscoveryOptions::default()
    });
    assert_eq!(roots(&profile), [(".", "cargo")]);
    assert!(has_note(
        &profile,
        "discovery stopped after 10 directory entries"
    ));
    assert_eq!(fixture.discover().roots.len(), 2);
}

#[test]
fn oversized_manifests_are_skipped() {
    let fixture = Fixture::new(&[(
        "package.json",
        &format!(
            r#"{{"scripts":{{"test":"jest"}},"description":"{}"}}"#,
            "x".repeat(200)
        ),
    )]);
    let profile = fixture.discover_with(DiscoveryOptions {
        max_manifest_bytes: 100,
        ..DiscoveryOptions::default()
    });
    assert!(profile.roots.is_empty());
    assert!(has_note(
        &profile,
        "package.json: skipped: larger than 100 bytes"
    ));
    assert_eq!(fixture.discover().checks.len(), 1);
}

#[test]
fn gitignore_rules_are_honored() {
    let fixture = Fixture::new(&[
        (".gitignore", "ignored/\n/generated\nsub/*\n!sub/keep/\n"),
        (".git/info/exclude", "private/\n"),
        ("ignored/package.json", "{}"),
        ("generated/Cargo.toml", "[package]\nname = \"gen\"\n"),
        ("sub/drop/package.json", "{}"),
        ("sub/keep/package.json", "{}"),
        ("private/go.mod", "module example.com/private\n"),
        ("web/.gitignore", "legacy/\n"),
        ("web/package.json", "{}"),
        ("web/legacy/package.json", "{}"),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [("sub/keep", "node"), ("web", "node")]);
}

#[test]
fn dependency_build_and_state_directories_are_never_entered() {
    let fixture = Fixture::new(&[
        ("node_modules/left-pad/package.json", "{}"),
        (
            "target/package/demo-0.1.0/Cargo.toml",
            "[package]\nname = \"demo\"\n",
        ),
        (
            ".z-engine/worktrees/agt_1/Cargo.toml",
            "[package]\nname = \"copy\"\n",
        ),
        ("build/Makefile", "test:\n\tctest\n"),
        (".venv/lib/pyproject.toml", "[tool.pytest]\n"),
        ("cmake-build-debug/Makefile", "test:\n\tctest\n"),
    ]);
    let profile = fixture.discover();
    assert!(profile.roots.is_empty(), "{:?}", profile.roots);
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_followed() {
    let fixture = Fixture::new(&[("real/package.json", "{}"), ("outside/package.json", "{}")]);
    std::os::unix::fs::symlink(fixture.path().join("real"), fixture.path().join("link")).unwrap();
    fixture.mkdir("pkg");
    std::os::unix::fs::symlink(
        fixture.path().join("outside/package.json"),
        fixture.path().join("pkg/package.json"),
    )
    .unwrap();
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [("outside", "node"), ("real", "node")]);
}

#[test]
fn discovery_never_runs_scripts() {
    let fixture = Fixture::new(&[(
        "package.json",
        r#"{"scripts":{"test":"touch DISCOVERY_RAN","build":"touch DISCOVERY_RAN"}}"#,
    )]);
    let profile = fixture.discover();
    assert_eq!(profile.checks.len(), 2);
    assert!(!fixture.path().join("DISCOVERY_RAN").exists());
}

#[test]
fn a_missing_root_is_a_note() {
    let fixture = Fixture::new(&[]);
    let profile = discover(
        &fixture.path().join("missing"),
        &DiscoveryOptions::default(),
    );
    assert!(profile.roots.is_empty() && profile.checks.is_empty());
    assert!(has_note(&profile, ".: cannot list directory"));
}
