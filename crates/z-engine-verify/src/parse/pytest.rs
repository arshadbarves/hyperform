//! pytest: the final summary (`=== 1 failed, 5 passed, 2 skipped in 0.12s
//! ===`, also without the rule in `-q` mode). Errors count as failures,
//! xpassed as passed, xfailed as skipped.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, regex, starts_with_digit, terms};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static SUMMARY: OnceLock<Option<Regex>> = OnceLock::new();
    let summary = regex(
        &SUMMARY,
        r"^(\d+ [a-z]+(?:, \d+ [a-z]+)*|no tests ran) in \d+(?:\.\d+)?s(?: \(\d+:\d{2}:\d{2}\))?$",
    )?;
    let mut tally = Tally::default();
    for line in lines {
        let body = line.trim().trim_matches('=').trim();
        if !(starts_with_digit(body) || body.starts_with("no tests ran")) {
            continue;
        }
        let Some(c) = summary.captures(body) else {
            continue;
        };
        let (mut passed, mut failed, mut skipped) = (0u32, 0u32, 0u32);
        for (count, word) in terms(&c[1]) {
            match word.as_str() {
                "passed" | "xpassed" => passed = passed.saturating_add(count),
                "failed" | "error" | "errors" => failed = failed.saturating_add(count),
                "skipped" | "xfailed" => skipped = skipped.saturating_add(count),
                _ => {}
            }
        }
        tally.add(passed, failed, skipped);
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
    fn full_summary_with_failures_errors_and_warnings() {
        let output = "\
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-8.3.2, pluggy-1.5.0
rootdir: /home/dev/api
configfile: pyproject.toml
collected 9 items

tests/test_api.py ..F.s.xE.                                              [100%]

==================================== ERRORS ====================================
___________________________ ERROR at setup of test_db __________________________
=================================== FAILURES ===================================
________________________________ test_divide ___________________________________
=========================== short test summary info ============================
FAILED tests/test_api.py::test_divide - ZeroDivisionError: division by zero
ERROR tests/test_api.py::test_db - RuntimeError: no database
= 1 failed, 5 passed, 1 skipped, 1 xfailed, 2 warnings, 1 error in 0.12s =
";
        assert_eq!(parse(&clean_lines(output)), counts(5, 2, 2));
    }

    #[test]
    fn quiet_mode_long_runs_and_no_tests() {
        assert_eq!(
            parse(&clean_lines(
                "........ [100%]\n8 passed in 61.02s (0:01:01)\n"
            )),
            counts(8, 0, 0)
        );
        assert_eq!(
            parse(&clean_lines(
                "============ no tests ran in 0.01s ============\n"
            )),
            counts(0, 0, 0)
        );
        assert_eq!(parse(&clean_lines("collected 0 items\n")), None);
    }
}
