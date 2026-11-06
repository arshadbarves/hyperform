//! Gradle: `5 tests completed, 1 failed, 1 skipped`. Gradle prints counts
//! only when a test task fails, so a passing run has none.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, number, regex};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static SUMMARY: OnceLock<Option<Regex>> = OnceLock::new();
    let summary = regex(
        &SUMMARY,
        r"^(\d+) tests? completed, (\d+) failed(?:, (\d+) skipped)?",
    )?;
    let mut tally = Tally::default();
    for line in lines {
        if !line.contains(" completed, ") {
            continue;
        }
        let Some(c) = summary.captures(line.trim()) else {
            continue;
        };
        let (completed, failed) = (number(&c[1]), number(&c[2]));
        let skipped = c.get(3).map_or(0, |m| number(m.as_str()));
        tally.add(
            completed.saturating_sub(failed).saturating_sub(skipped),
            failed,
            skipped,
        );
    }
    tally.finish()
}

#[cfg(test)]
mod tests {
    use super::super::text::clean_lines;
    use super::*;

    #[test]
    fn reads_failure_summaries() {
        let output = "\
> Task :app:test FAILED

AppTest > divides() FAILED
    org.opentest4j.AssertionFailedError at AppTest.java:21

6 tests completed, 1 failed, 2 skipped

FAILURE: Build failed with an exception.
";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 3,
                failed: 1,
                skipped: 2
            })
        );
        assert_eq!(parse(&clean_lines("BUILD SUCCESSFUL in 4s\n")), None);
    }
}
