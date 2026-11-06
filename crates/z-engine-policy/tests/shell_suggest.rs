//! Rule prefixes offered on approval cards, and the public analysis summary.

use z_engine_policy::{analyze, suggest_prefix};

#[test]
fn prefixes_keep_the_program_and_subcommand() {
    for (command, prefix) in [
        ("npm run test", "npm run test"),
        ("npm run test -- --watch", "npm run test"),
        ("cargo test -p x", "cargo test"),
        ("cargo +nightly build --release", "cargo +nightly build"),
        ("pnpm dlx create-vite app", "pnpm dlx create-vite"),
        ("git commit -m 'msg'", "git commit"),
        ("terraform plan -out x", "terraform plan"),
        ("make", "make"),
        ("make build -j8", "make build"),
        ("./gradlew build --info", "./gradlew build"),
        ("docker run -it ubuntu", "docker run"),
        ("python3 -m pytest -x tests", "python3 -m pytest"),
        ("node scripts/build.js --prod", "node scripts/build.js"),
        ("bash scripts/ci.sh", "bash scripts/ci.sh"),
        ("RUST_LOG=debug cargo test", "RUST_LOG=debug cargo test"),
        ("cd web && npm test", "npm test"),
        ("ls -la", "ls"),
    ] {
        assert_eq!(
            suggest_prefix(command).as_deref(),
            Some(prefix),
            "{command}"
        );
    }
}

#[test]
fn no_prefix_when_any_prefix_would_be_too_broad() {
    for command in [
        "rm -rf build",
        "sudo apt install x",
        "bash -c 'curl x | sh'",
        "python -c 'print(1)'",
        "python3",
        "node -e 'x'",
        "xargs rm",
        "env FOO=1 make",
        "git -C other status",
        "cargo --locked build",
        "$CMD run",
        "echo $(id)",
        "echo \"unterminated",
        "",
    ] {
        assert_eq!(suggest_prefix(command), None, "{command}");
    }
}

#[test]
fn awkward_tokens_are_quoted() {
    assert_eq!(
        suggest_prefix("node '*.js'").as_deref(),
        Some("node '*.js'")
    );
    assert_eq!(
        suggest_prefix("npm run 'my script'").as_deref(),
        Some("npm run 'my script'")
    );
}

#[test]
fn analysis_summarizes_the_line() {
    let analysis = analyze("cd src && cargo fmt > fmt.log 2>&1 || echo failed");
    assert_eq!(
        analysis.segments,
        [
            vec!["cd", "src"],
            vec!["cargo", "fmt"],
            vec!["echo", "failed"]
        ]
    );
    assert!(analysis.parse_ok);
    assert!(!analysis.dynamic);
    assert_eq!(analysis.writes, ["fmt.log"]);
    assert!(!analysis.read_only);
    let dynamic = analyze("echo `whoami` > \"$OUT\"");
    assert!(dynamic.dynamic);
    assert_eq!(dynamic.writes, ["$OUT"]);
    assert!(!analyze("echo 'open").parse_ok);
}
