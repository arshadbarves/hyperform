//! `deno test`: the result line (`ok | 5 passed | 0 failed | 1 ignored
//! (20ms)`); step counts in parentheses are not tests.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, number, regex};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static RESULT: OnceLock<Option<Regex>> = OnceLock::new();
    let result = regex(
        &RESULT,
        r"^(?:ok|FAILED) \| (\d+) passed(?: \(\d+ steps?\))? \| (\d+) failed(?: \(\d+ steps?\))?(?: \| (\d+) ignored)?",
    )?;
    let mut tally = Tally::default();
    for line in lines {
        let line = line.trim();
        if !(line.starts_with("ok |") || line.starts_with("FAILED |")) {
            continue;
        }
        if let Some(c) = result.captures(line) {
            let ignored = c.get(3).map_or(0, |m| number(m.as_str()));
            tally.add(number(&c[1]), number(&c[2]), ignored);
        }
    }
    tally.finish()
}

#[cfg(test)]
mod tests {
    use super::super::text::clean_lines;
    use super::*;

    #[test]
    fn reads_passed_failed_and_ignored() {
        let output = "\
running 3 tests from ./math_test.ts
adds ... ok (1ms)
divides ... FAILED (2ms)
later ... ignored (0ms)

 ERRORS

divides => ./math_test.ts:9:6
error: AssertionError: Values are not equal.

FAILED | 1 passed | 1 failed | 1 ignored (15ms)

error: Test failed
";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 1,
                failed: 1,
                skipped: 1
            })
        );
        let steps = "ok | 2 passed (3 steps) | 0 failed (8ms)\n";
        assert_eq!(
            parse(&clean_lines(steps)),
            Some(TestCounts {
                passed: 2,
                failed: 0,
                skipped: 0
            })
        );
    }
}
