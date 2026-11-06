mod support;

use support::{Fixture, check, command, has_note, ids, roots};
use z_engine_protocol::CheckKind;

#[test]
fn go_modules() {
    let fixture = Fixture::new(&[
        ("go.mod", "// service\nmodule example.com/shop\n\ngo 1.23\n"),
        ("tools/go.mod", "go 1.23\n"),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "go")]);
    assert_eq!(ids(&profile), ["go:test", "go:build", "go:vet"]);
    assert_eq!(command(&profile, "go:test"), "go test ./...");
    assert_eq!(command(&profile, "go:vet"), "go vet ./...");
    assert_eq!(check(&profile, "go:vet").kind, CheckKind::Lint);
    assert!(has_note(&profile, "tools/go.mod: no `module` directive"));
}

#[test]
fn cmake_needs_a_configured_build_directory() {
    let unconfigured = Fixture::new(&[
        ("CMakeLists.txt", "project(native)\nadd_subdirectory(lib)\n"),
        ("lib/CMakeLists.txt", "add_library(lib lib.c)\n"),
    ]);
    let profile = unconfigured.discover();
    assert_eq!(roots(&profile), [(".", "cmake")]);
    assert!(profile.checks.is_empty());
    assert!(has_note(&profile, "CMakeLists.txt: no build directory"));

    let configured = Fixture::new(&[(
        "CMakeLists.txt",
        "project(native)\nenable_testing()\nadd_test(NAME unit COMMAND unit)\n",
    )]);
    configured.mkdir("build");
    let profile = configured.discover();
    assert_eq!(ids(&profile), ["cmake:build", "cmake:test"]);
    assert_eq!(command(&profile, "cmake:build"), "cmake --build build");
    assert_eq!(command(&profile, "cmake:test"), "ctest --test-dir build");

    let untested = Fixture::new(&[("CMakeLists.txt", "project(native)\n")]);
    untested.mkdir("build");
    assert_eq!(ids(&untested.discover()), ["cmake:build"]);
}

#[test]
fn make_targets_and_just_recipes() {
    let fixture = Fixture::new(&[
        (
            "Makefile",
            ".PHONY: test lint\ntest:\n\tgo test ./...\nlint:\n\tgolangci-lint run\ninstall:\n\tcp x y\n",
        ),
        (
            "justfile",
            "set shell := [\"bash\", \"-c\"]\nbuild:\n    cargo build\n\ncheck: build\n    cargo check\n",
        ),
        ("docs/Makefile", "html:\n\tsphinx-build . _build\n"),
        ("bsd/Makefile", ".if defined(X)\ntest:\n.endif\n"),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "make"), (".", "just")]);
    assert_eq!(
        ids(&profile),
        ["make:test", "make:lint", "just:check", "just:build"]
    );
    assert_eq!(command(&profile, "make:test"), "make test");
    assert_eq!(command(&profile, "just:build"), "just build");
    assert_eq!(check(&profile, "just:check").kind, CheckKind::Test);
    assert!(has_note(&profile, "bsd/Makefile: non-GNU make syntax"));
    assert!(!profile.notes.iter().any(|n| n.starts_with("docs/")));

    let bare = Fixture::new(&[("Makefile", "all:\n\tcc main.c\n")]).discover();
    assert!(bare.roots.is_empty());
    assert!(has_note(
        &bare,
        "Makefile: no test, check, build or lint target"
    ));
}

#[test]
fn deno_tasks_and_deno_test() {
    let fixture = Fixture::new(&[(
        "deno.jsonc",
        "{\n  // tasks\n  \"tasks\": {\n    \"check\": \"deno check main.ts\",\n    \"fmt\": \"deno fmt\",\n    \"lint\": { \"command\": \"deno lint\", \"description\": \"lint\" },\n  },\n}\n",
    )]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "deno")]);
    assert_eq!(ids(&profile), ["deno:check", "deno:lint", "deno:test"]);
    assert_eq!(command(&profile, "deno:check"), "deno task check");
    assert_eq!(command(&profile, "deno:lint"), "deno task lint");
    assert_eq!(command(&profile, "deno:test"), "deno test");
    assert!(has_note(&profile, "task `fmt` is not a check"));

    let with_test =
        Fixture::new(&[("deno.json", r#"{"tasks":{"test":"deno test -A"}}"#)]).discover();
    assert_eq!(ids(&with_test), ["deno:test"]);
    assert_eq!(command(&with_test, "deno:test"), "deno task test");
}
