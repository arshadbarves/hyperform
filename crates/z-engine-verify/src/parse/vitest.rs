//! Vitest: the `Tests` summary line (`Tests  1 failed | 4 passed (5)`);
//! skipped and todo tests count as skipped.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, regex, terms};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static SUMMARY: OnceLock<Option<Regex>> = OnceLock::new();
    let summary = regex(
        &SUMMARY,
        r"^Tests\s+(\d+ [a-z]+(?: \| \d+ [a-z]+)*)\s+\(\d+\)\s*$",
    )?;
    let mut tally = Tally::default();
    for line in lines {
        let line = line.trim_start();
        if !line.starts_with("Tests ") {
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
                "skipped" | "todo" => skipped = skipped.saturating_add(count),
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
    fn reads_the_tests_line_not_the_files_line() {
        let output = "\
 RUN  v2.1.8 /home/dev/web

 ✓ src/format.test.ts (3 tests) 4ms
 ❯ src/parse.test.ts (3 tests | 1 failed | 1 skipped) 9ms
   × parse > rejects bad input 5ms
     → expected 'ok' to be 'error'

⎯⎯⎯⎯⎯⎯⎯ Failed Tests 1 ⎯⎯⎯⎯⎯⎯⎯

 Test Files  1 failed | 1 passed (2)
      Tests  1 failed | 4 passed | 1 skipped (6)
   Start at  10:42:17
   Duration  612ms (transform 51ms, setup 0ms, collect 88ms, tests 13ms, environment 0ms, prepare 97ms)
";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 4,
                failed: 1,
                skipped: 1
            })
        );
    }

    #[test]
    fn colored_all_pass_and_no_tests() {
        let colored = "      \x1b[2mTests \x1b[22m \x1b[1m\x1b[32m5 passed\x1b[39m\x1b[22m\x1b[90m (5)\x1b[39m\n";
        assert_eq!(
            parse(&clean_lines(colored)),
            Some(TestCounts {
                passed: 5,
                failed: 0,
                skipped: 0
            })
        );
        assert_eq!(parse(&clean_lines("      Tests  no tests\n")), None);
    }
}
