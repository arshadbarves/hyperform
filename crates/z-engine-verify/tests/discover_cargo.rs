mod support;

use std::path::PathBuf;

use support::{Fixture, check, command, has_note, ids, roots};
use z_engine_protocol::CheckKind;
use z_engine_verify::CheckSource;

#[test]
fn a_single_package_gets_test_build_lint_and_format() {
    let fixture = Fixture::new(&[(
        "Cargo.toml",
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "cargo")]);
    assert_eq!(profile.roots[0].manifest, "Cargo.toml");
    assert_eq!(
        ids(&profile),
        ["cargo:test", "cargo:build", "cargo:clippy", "cargo:fmt"]
    );
    assert_eq!(command(&profile, "cargo:test"), "cargo test");
    assert_eq!(command(&profile, "cargo:build"), "cargo build");
    assert_eq!(
        command(&profile, "cargo:clippy"),
        "cargo clippy --all-targets"
    );
    assert_eq!(command(&profile, "cargo:fmt"), "cargo fmt --check");
    let test = check(&profile, "cargo:test");
    assert_eq!(test.kind, CheckKind::Test);
    assert_eq!(test.label, "cargo test");
    assert_eq!(test.cwd, PathBuf::from("."));
    assert_eq!(
        test.source,
        CheckSource::Discovered {
            manifest: "Cargo.toml".into()
        }
    );
    assert_eq!(check(&profile, "cargo:clippy").kind, CheckKind::Lint);
    assert_eq!(check(&profile, "cargo:fmt").kind, CheckKind::Format);
}

#[test]
fn workspace_members_are_covered_by_the_workspace_root() {
    let fixture = Fixture::new(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"examples/standalone\"]\n",
        ),
        ("crates/core/Cargo.toml", "[package]\nname = \"core\"\n"),
        ("crates/cli/Cargo.toml", "[package]\nname = \"cli\"\n"),
        (
            "examples/standalone/Cargo.toml",
            "[package]\nname = \"standalone\"\n",
        ),
    ]);
    let profile = fixture.discover();
    assert_eq!(
        roots(&profile),
        [(".", "cargo"), ("examples/standalone", "cargo")]
    );
    assert_eq!(command(&profile, "cargo:test"), "cargo test --workspace");
    assert_eq!(
        command(&profile, "cargo:clippy"),
        "cargo clippy --workspace --all-targets"
    );
    assert_eq!(command(&profile, "cargo:fmt"), "cargo fmt --check");
    let standalone = check(&profile, "examples/standalone/cargo:test");
    assert_eq!(standalone.command, "cargo test");
    assert_eq!(standalone.cwd, PathBuf::from("examples/standalone"));
    assert_eq!(standalone.label, "cargo test (examples/standalone)");
    assert!(!ids(&profile).iter().any(|id| id.starts_with("crates/")));
}

#[test]
fn a_root_package_with_a_workspace_tests_every_member() {
    let fixture = Fixture::new(&[
        (
            "Cargo.toml",
            "[package]\nname = \"app\"\n\n[workspace]\nmembers = [\"plugin\"]\n",
        ),
        ("plugin/Cargo.toml", "[package]\nname = \"plugin\"\n"),
        ("tools/nested/Cargo.toml", "[workspace]\n"),
        (
            "tools/nested/inner/Cargo.toml",
            "[package]\nname = \"inner\"\n",
        ),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "cargo"), ("tools/nested", "cargo")]);
    assert_eq!(command(&profile, "cargo:build"), "cargo build --workspace");
    assert_eq!(
        command(&profile, "tools/nested/cargo:test"),
        "cargo test --workspace"
    );
}

#[test]
fn invalid_manifests_become_notes() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", "[package\nname = \"broken\""),
        ("other/Cargo.toml", "name = \"no tables\"\n"),
        ("ok/Cargo.toml", "[package]\nname = \"ok\"\n"),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [("ok", "cargo")]);
    assert!(has_note(
        &profile,
        "Cargo.toml: invalid TOML (TOML parse error"
    ));
    assert!(has_note(
        &profile,
        "other/Cargo.toml: has neither a [package] nor a [workspace] table"
    ));
}
