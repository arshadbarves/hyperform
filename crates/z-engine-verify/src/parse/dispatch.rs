//! Chooses a summary parser: the runner named by the command first (so a
//! `cargo test` whose tests print other formats is read as cargo), then
//! every format by output shape. Each parser only answers when its own
//! distinctive summary line is present.

use std::borrow::Cow;

use z_engine_protocol::TestCounts;

use super::text::clean_lines;
use super::{bun, cargo, ctest, deno, dotnet, go, gradle, jest, maven, mocha, pytest, vitest};

type Parser = fn(&[Cow<'_, str>]) -> Option<TestCounts>;

/// Every known format; the order only matters for output mixing formats.
const PARSERS: &[(Runner, Parser)] = &[
    (Runner::Cargo, cargo::parse),
    (Runner::Vitest, vitest::parse),
    (Runner::Jest, jest::parse),
    (Runner::Pytest, pytest::parse),
    (Runner::Go, go::parse),
    (Runner::Dotnet, dotnet::parse),
    (Runner::Mocha, mocha::parse),
    (Runner::Deno, deno::parse),
    (Runner::Bun, bun::parse),
    (Runner::Ctest, ctest::parse),
    (Runner::Maven, maven::parse),
    (Runner::Gradle, gradle::parse),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Runner {
    Cargo,
    Vitest,
    Jest,
    Pytest,
    Go,
    Dotnet,
    Mocha,
    Deno,
    Bun,
    Ctest,
    Maven,
    Gradle,
}

/// Test counts reported in `output` of `command`, or `None` when no known
/// test summary is present (a build, a lint, an unknown runner, or a run
/// that failed before reporting). CPU-bound in the size of `output`: run
/// it off the async executor for large outputs, as `run_check` does.
pub fn parse_counts(command: &str, output: &str) -> Option<TestCounts> {
    let lines = clean_lines(output);
    let preferred = runner_of(command);
    let first = PARSERS
        .iter()
        .filter(|(runner, _)| Some(*runner) == preferred);
    let rest = PARSERS
        .iter()
        .filter(|(runner, _)| Some(*runner) != preferred);
    first.chain(rest).find_map(|(_, parse)| parse(&lines))
}

/// The test runner a command names, looking through wrappers such as
/// `uv run`, `npx` or `.venv/bin/python -m`.
fn runner_of(command: &str) -> Option<Runner> {
    let words: Vec<&str> = command
        .split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')'))
        .filter(|word| !word.is_empty())
        .map(program_name)
        .collect();
    words.iter().enumerate().find_map(|(i, word)| match *word {
        "cargo" | "cargo-nextest" => Some(Runner::Cargo),
        "vitest" => Some(Runner::Vitest),
        "jest" => Some(Runner::Jest),
        "pytest" | "py.test" | "tox" | "nox" => Some(Runner::Pytest),
        "go" => Some(Runner::Go),
        "dotnet" => Some(Runner::Dotnet),
        "mocha" => Some(Runner::Mocha),
        "deno" => Some(Runner::Deno),
        "bun" if words.get(i + 1) == Some(&"test") => Some(Runner::Bun),
        "ctest" => Some(Runner::Ctest),
        "mvn" | "mvnw" => Some(Runner::Maven),
        "gradle" | "gradlew" => Some(Runner::Gradle),
        _ => None,
    })
}

/// `./gradlew` -> `gradlew`, `C:\tools\mvn.cmd` -> `mvn`.
fn program_name(word: &str) -> &str {
    let name = word.rsplit(['/', '\\']).next().unwrap_or(word);
    [".exe", ".cmd", ".bat"]
        .iter()
        .find_map(|ext| name.strip_suffix(ext))
        .unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARGO: &str = "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
    const PYTEST: &str = "==== 2 passed, 1 failed in 0.05s ====\n";

    #[test]
    fn runners_are_found_through_wrappers() {
        assert_eq!(runner_of("cargo test --workspace"), Some(Runner::Cargo));
        assert_eq!(runner_of("uv run pytest -q"), Some(Runner::Pytest));
        assert_eq!(
            runner_of(".venv/bin/python -m pytest"),
            Some(Runner::Pytest)
        );
        assert_eq!(runner_of("pnpm exec vitest run"), Some(Runner::Vitest));
        assert_eq!(runner_of("./gradlew test"), Some(Runner::Gradle));
        assert_eq!(runner_of(r".\mvnw.cmd -q test"), Some(Runner::Maven));
        assert_eq!(runner_of("bun test"), Some(Runner::Bun));
        assert_eq!(runner_of("bun run test"), None);
        assert_eq!(runner_of("npm test"), None);
    }

    #[test]
    fn the_named_runner_wins_over_other_formats() {
        let mixed = format!("{PYTEST}{CARGO}");
        let cargo = parse_counts("cargo test", &mixed).unwrap();
        assert_eq!((cargo.passed, cargo.failed), (3, 0));
        let pytest = parse_counts("python3 -m pytest", &mixed).unwrap();
        assert_eq!((pytest.passed, pytest.failed), (2, 1));
    }

    #[test]
    fn unknown_commands_fall_back_to_output_shape() {
        let counts = parse_counts("make test", PYTEST).unwrap();
        assert_eq!((counts.passed, counts.failed), (2, 1));
        let counts =
            parse_counts("npm test", "Tests:       1 failed, 3 passed, 4 total\n").unwrap();
        assert_eq!((counts.passed, counts.failed), (3, 1));
        assert_eq!(
            parse_counts("cargo build", "   Compiling demo v0.1.0\n"),
            None
        );
        assert_eq!(
            parse_counts("npm run lint", "✖ 3 problems (3 errors, 0 warnings)\n"),
            None
        );
    }
}
