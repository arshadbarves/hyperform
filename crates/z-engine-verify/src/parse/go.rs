//! `go test`: verbose runs count top-level `--- PASS/FAIL/SKIP` lines
//! (subtests are indented and not counted); plain runs count packages from
//! the `ok` and `FAIL` package lines.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, regex};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    let verbose = lines.iter().any(|l| {
        l.starts_with("=== RUN") || l.starts_with("--- PASS: ") || l.starts_with("--- SKIP: ")
    });
    if verbose {
        tests(lines)
    } else {
        packages(lines)
    }
}

fn tests(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    let mut tally = Tally::default();
    for line in lines {
        if line.starts_with("--- PASS: ") {
            tally.add(1, 0, 0);
        } else if line.starts_with("--- FAIL: ") {
            tally.add(0, 1, 0);
        } else if line.starts_with("--- SKIP: ") {
            tally.add(0, 0, 1);
        }
    }
    tally.finish()
}

fn packages(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static OK: OnceLock<Option<Regex>> = OnceLock::new();
    static FAIL: OnceLock<Option<Regex>> = OnceLock::new();
    let ok = regex(&OK, r"^ok\s+\S+\s+(?:\d+(?:\.\d+)?s|\(cached\))")?;
    let fail = regex(
        &FAIL,
        r"^FAIL\s+\S+(?:\s+\d+(?:\.\d+)?s|\s+\[(?:build|setup) failed\])",
    )?;
    let mut tally = Tally::default();
    for line in lines {
        if line.starts_with("ok") && ok.is_match(line) {
            tally.add(1, 0, 0);
        } else if line.starts_with("FAIL") && fail.is_match(line) {
            tally.add(0, 1, 0);
        }
    }
    tally.finish()
}

#[cfg(test)]
mod tests {
    use super::super::text::clean_lines;
    use super::*;

    fn counts(passed: u32, failed: u32, skipped: u32) -> Option<TestCounts> {
        Some(TestCounts {
            passed,
            failed,
            skipped,
        })
    }

    #[test]
    fn verbose_runs_count_top_level_tests() {
        let output = "\
=== RUN   TestAdd
--- PASS: TestAdd (0.00s)
=== RUN   TestTable
=== RUN   TestTable/zero
=== RUN   TestTable/negative
    table_test.go:18: got -1, want 1
--- FAIL: TestTable (0.00s)
    --- PASS: TestTable/zero (0.00s)
    --- FAIL: TestTable/negative (0.00s)
=== RUN   TestNetwork
    net_test.go:9: skipping in short mode
--- SKIP: TestNetwork (0.00s)
FAIL
FAIL\texample.com/calc\t0.012s
FAIL
";
        assert_eq!(parse(&clean_lines(output)), counts(1, 1, 1));
    }

    #[test]
    fn plain_runs_count_packages() {
        let output = "\
ok  \texample.com/calc\t0.011s
ok  \texample.com/calc/internal\t(cached)
?   \texample.com/calc/cmd\t[no test files]
--- FAIL: TestParse (0.00s)
    parse_test.go:12: unexpected token
FAIL
FAIL\texample.com/calc/parse\t0.009s
# example.com/calc/io [example.com/calc/io.test]
io/io_test.go:5:2: undefined: Reader
FAIL\texample.com/calc/io [build failed]
FAIL
";
        assert_eq!(parse(&clean_lines(output)), counts(2, 2, 0));
        assert_eq!(parse(&clean_lines("go: no packages to test\n")), None);
    }
}
