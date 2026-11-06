//! Maven Surefire: the per-module `Tests run: N, Failures: F, Errors: E,
//! Skipped: S` totals. Per-class lines (with `Time elapsed`) are not
//! counted, so a module is counted once; errors count as failures.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, number, regex};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static TOTAL: OnceLock<Option<Regex>> = OnceLock::new();
    let total = regex(
        &TOTAL,
        r"^(?:\[[A-Z]+\]\s+)?Tests run: (\d+), Failures: (\d+), Errors: (\d+), Skipped: (\d+)\s*$",
    )?;
    let mut tally = Tally::default();
    for line in lines {
        if !line.contains("Tests run: ") {
            continue;
        }
        let Some(c) = total.captures(line.trim()) else {
            continue;
        };
        let (run, failures, errors, skipped) =
            (number(&c[1]), number(&c[2]), number(&c[3]), number(&c[4]));
        let failed = failures.saturating_add(errors);
        tally.add(
            run.saturating_sub(failed).saturating_sub(skipped),
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
    fn counts_module_totals_once() {
        let output = "\
[INFO] -------------------------------------------------------
[INFO]  T E S T S
[INFO] -------------------------------------------------------
[INFO] Running com.example.AppTest
[ERROR] Tests run: 4, Failures: 1, Errors: 0, Skipped: 1, Time elapsed: 0.05 s <<< FAILURE! -- in com.example.AppTest
[INFO] Running com.example.ParserTest
[INFO] Tests run: 3, Failures: 0, Errors: 1, Skipped: 0, Time elapsed: 0.01 s -- in com.example.ParserTest
[INFO]
[INFO] Results:
[INFO]
[ERROR] Failures:
[ERROR]   AppTest.divides:21 expected: <2> but was: <3>
[INFO]
[ERROR] Tests run: 7, Failures: 1, Errors: 1, Skipped: 1
[INFO]
[INFO] BUILD FAILURE
";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 4,
                failed: 2,
                skipped: 1
            })
        );
        assert_eq!(parse(&clean_lines("[INFO] BUILD SUCCESS\n")), None);
    }
}
