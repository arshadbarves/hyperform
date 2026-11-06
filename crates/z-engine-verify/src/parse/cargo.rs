//! Rust test output: every libtest `test result:` line is summed (one per
//! test binary and doc-test run); without any, cargo-nextest's final
//! `Summary` line is used.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, number, regex, terms};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static RESULT: OnceLock<Option<Regex>> = OnceLock::new();
    let result = regex(
        &RESULT,
        r"^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored",
    )?;
    let mut tally = Tally::default();
    for line in lines {
        let line = line.trim_start();
        if !line.starts_with("test result: ") {
            continue;
        }
        if let Some(c) = result.captures(line) {
            tally.add(number(&c[1]), number(&c[2]), number(&c[3]));
        }
    }
    tally.finish().or_else(|| nextest(lines))
}

fn nextest(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static SUMMARY: OnceLock<Option<Regex>> = OnceLock::new();
    let summary = regex(&SUMMARY, r"^Summary \[[^\]]*\] \d+ tests? run: (.+)$")?;
    let run = lines
        .iter()
        .rev()
        .map(|line| line.trim())
        .filter(|line| line.starts_with("Summary ["))
        .find_map(|line| summary.captures(line))?;
    let mut counts = TestCounts::default();
    for (count, word) in terms(&run[1]) {
        match word.as_str() {
            "passed" => counts.passed = counts.passed.saturating_add(count),
            "failed" | "timed" => counts.failed = counts.failed.saturating_add(count),
            "skipped" => counts.skipped = counts.skipped.saturating_add(count),
            _ => {}
        }
    }
    Some(counts)
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
    fn sums_every_test_binary_and_doc_tests() {
        let output = "\
   Compiling demo v0.1.0 (/tmp/demo)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.52s
     Running unittests src/lib.rs (target/debug/deps/demo-1a2b3c)

running 3 tests
test tests::adds ... ok
test tests::slow ... ignored
test tests::breaks ... FAILED

failures:

---- tests::breaks stdout ----
thread 'tests::breaks' panicked at src/lib.rs:12:9:
assertion `left == right` failed

failures:
    tests::breaks

test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/api.rs (target/debug/deps/api-4d5e6f)

running 2 tests
test lists ... ok
test reads ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Doc-tests demo

running 1 test
test src/lib.rs - add (line 3) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
";
        assert_eq!(parse(&clean_lines(output)), counts(4, 1, 1));
    }

    #[test]
    fn a_compile_error_has_no_counts() {
        let output = "error[E0425]: cannot find value `x` in this scope\n\
                      error: could not compile `demo` (lib test) due to 1 previous error\n";
        assert_eq!(parse(&clean_lines(output)), None);
    }

    #[test]
    fn nextest_summary_is_read() {
        let output = "\
    Starting 3 tests across 1 binary
        PASS [   0.004s] demo tests::adds
        FAIL [   0.005s] demo tests::breaks
------------
     Summary [   0.006s] 3 tests run: 2 passed, 1 failed, 1 skipped
        FAIL [   0.005s] demo tests::breaks
error: test run failed
";
        assert_eq!(parse(&clean_lines(output)), counts(2, 1, 1));
    }
}
