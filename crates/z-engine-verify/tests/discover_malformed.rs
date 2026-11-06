mod support;

use support::{Fixture, has_note, ids, roots};

#[test]
fn malformed_manifests_produce_notes_not_checks() {
    for (file, text) in [
        ("Cargo.toml", "[package"),
        ("Cargo.toml", "package = []"),
        ("package.json", "{bad"),
        ("package.json", "null"),
        ("pyproject.toml", "[tool.pytest.ini_options"),
        ("go.mod", "go 1.23"),
        ("pom.xml", "<wrong/>"),
        ("App.csproj", "<Wrong/>"),
        ("App.sln", "not a solution"),
        ("App.slnx", "<Project/>"),
        ("deno.json", "{bad"),
        ("deno.jsonc", "[]"),
        ("Makefile", ".RECIPEPREFIX = >\ntest:\n> echo\n"),
    ] {
        let profile = Fixture::new(&[(file, text)]).discover();
        assert!(
            profile.checks.is_empty(),
            "{file}: {text} -> {:?}",
            ids(&profile)
        );
        assert!(
            profile.notes.iter().any(|n| n.starts_with(file)),
            "{file}: {text} -> {:?}",
            profile.notes
        );
    }
}

#[test]
fn unreadable_text_is_a_note() {
    let fixture = Fixture::new(&[]);
    std::fs::write(fixture.path().join("package.json"), [0xff, 0xfe, 0x00]).unwrap();
    std::fs::write(fixture.path().join("go.mod"), [0x00, 0x9f, 0x92, 0x96]).unwrap();
    let profile = fixture.discover();
    assert!(profile.roots.is_empty());
    assert!(has_note(&profile, "package.json: skipped: not UTF-8 text"));
    assert!(has_note(&profile, "go.mod: skipped: not UTF-8 text"));
}

#[test]
fn a_bad_nested_manifest_does_not_hide_good_roots() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = \"ok\"\n"),
        ("web/package.json", "{\"scripts\": }"),
        ("web/api/go.mod", "module example.com/api\n"),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "cargo"), ("web/api", "go")]);
    assert!(has_note(&profile, "web/package.json: invalid JSON ("));
}
