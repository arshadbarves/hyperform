//! CTest: `80% tests passed, 1 tests failed out of 5`.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, number, regex};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static SUMMARY: OnceLock<Option<Regex>> = OnceLock::new();
    let summary = regex(
        &SUMMARY,
        r"^\d+% tests passed, (\d+) tests? failed out of (\d+)",
    )?;
    let mut tally = Tally::default();
    for line in lines {
        if !line.contains("% tests passed, ") {
            continue;
        }
        if let Some(c) = summary.captures(line.trim()) {
            let failed = number(&c[1]);
            tally.add(number(&c[2]).saturating_sub(failed), failed, 0);
        }
    }
    tally.finish()
}

#[cfg(test)]
mod tests {
    use super::super::text::clean_lines;
    use super::*;

    #[test]
    fn reads_the_summary() {
        let output = "\
Test project /home/dev/native/build
    Start 1: unit
1/5 Test #1: unit .............................   Passed    0.01 sec
    Start 2: parser
2/5 Test #2: parser ...........................***Failed    0.02 sec

80% tests passed, 1 tests failed out of 5

Total Test time (real) =   0.09 sec

The following tests FAILED:
\t  2 - parser (Failed)
Errors while running CTest
";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 4,
                failed: 1,
                skipped: 0
            })
        );
        assert_eq!(parse(&clean_lines("No tests were found!!!\n")), None);
    }
}
