//! Mocha: the `N passing`, `N failing` and `N pending` summary lines.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, number, regex, starts_with_digit};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static LINE: OnceLock<Option<Regex>> = OnceLock::new();
    let summary = regex(&LINE, r"^(\d+) (passing|failing|pending)(?: \([^)]*\))?$")?;
    let mut tally = Tally::default();
    for line in lines {
        let line = line.trim();
        if !starts_with_digit(line) {
            continue;
        }
        let Some(c) = summary.captures(line) else {
            continue;
        };
        let count = number(&c[1]);
        match &c[2] {
            "passing" => tally.add(count, 0, 0),
            "failing" => tally.add(0, count, 0),
            _ => tally.add(0, 0, count),
        }
    }
    tally.finish()
}

#[cfg(test)]
mod tests {
    use super::super::text::clean_lines;
    use super::*;

    #[test]
    fn reads_passing_failing_and_pending() {
        let output = "\
  Array
    #indexOf()
      ✔ returns -1 when the value is absent
      1) finds the value
      - handles sparse arrays


  6 passing (23ms)
  1 pending
  1 failing

  1) Array
       #indexOf()
         finds the value:
     AssertionError [ERR_ASSERTION]: -1 == 0
";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 6,
                failed: 1,
                skipped: 1
            })
        );
        assert_eq!(parse(&clean_lines("Error: No test files found\n")), None);
    }
}
