//! `bun test`: the `N pass` / `N fail` / `N skip` / `N todo` lines, read
//! only when the closing `Ran N tests across` line confirms the format.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, number, regex, starts_with_digit};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static COUNT: OnceLock<Option<Regex>> = OnceLock::new();
    let count = regex(&COUNT, r"^(\d+) (pass|fail|skip|todo)$")?;
    if !lines
        .iter()
        .any(|l| l.trim_start().starts_with("Ran ") && l.contains(" across "))
    {
        return None;
    }
    let mut tally = Tally::default();
    for line in lines {
        let line = line.trim();
        if !starts_with_digit(line) {
            continue;
        }
        let Some(c) = count.captures(line) else {
            continue;
        };
        let n = number(&c[1]);
        match &c[2] {
            "pass" => tally.add(n, 0, 0),
            "fail" => tally.add(0, n, 0),
            _ => tally.add(0, 0, n),
        }
    }
    tally.finish()
}

#[cfg(test)]
mod tests {
    use super::super::text::clean_lines;
    use super::*;

    #[test]
    fn reads_the_summary_block() {
        let output = "\
bun test v1.1.34 (5e5e7c60)

math.test.ts:
✓ adds [0.12ms]
✗ divides [0.30ms]
» later

 1 pass
 1 skip
 1 fail
 2 expect() calls
Ran 3 tests across 1 files. [18.00ms]
";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 1,
                failed: 1,
                skipped: 1
            })
        );
        assert_eq!(parse(&clean_lines(" 3 pass\n")), None);
    }
}
