//! Jest: the `Tests:` summary line (`Tests: 1 failed, 5 passed, 6 total`);
//! skipped, todo and pending tests count as skipped.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, regex, terms};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static SUMMARY: OnceLock<Option<Regex>> = OnceLock::new();
    let summary = regex(&SUMMARY, r"^Tests:\s+(.*\d+ total)\s*$")?;
    let mut tally = Tally::default();
    for line in lines {
        let line = line.trim_start();
        if !line.starts_with("Tests:") {
            continue;
        }
        let Some(c) = summary.captures(line) else {
            continue;
        };
        let (mut passed, mut failed, mut skipped) = (0u32, 0u32, 0u32);
        for (count, word) in terms(&c[1]) {
            match word.as_str() {
                "passed" => passed = passed.saturating_add(count),
                "failed" => failed = failed.saturating_add(count),
                "skipped" | "todo" | "pending" => skipped = skipped.saturating_add(count),
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

    #[test]
    fn reads_failed_skipped_todo_and_passed() {
        let output = "\
 FAIL  src/math.test.js
  ● math › divides

    expect(received).toBe(expected) // Object.is equality

    Expected: 2
    Received: 3

 PASS  src/strings.test.js

Test Suites: 1 failed, 1 passed, 2 total
Tests:       1 failed, 1 skipped, 1 todo, 5 passed, 8 total
Snapshots:   0 total
Time:        1.234 s
Ran all test suites.
";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 5,
                failed: 1,
                skipped: 2
            })
        );
    }

    #[test]
    fn all_passing_and_absent_summaries() {
        let lines = clean_lines("Tests:       12 passed, 12 total\n");
        assert_eq!(
            parse(&lines),
            Some(TestCounts {
                passed: 12,
                failed: 0,
                skipped: 0
            })
        );
        assert_eq!(
            parse(&clean_lines("No tests found, exiting with code 1\n")),
            None
        );
    }
}
