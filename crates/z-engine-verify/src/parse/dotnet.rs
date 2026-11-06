//! `dotnet test`: one VSTest result line per test assembly
//! (`Passed!  - Failed: 0, Passed: 10, Skipped: 0, ...`), or the
//! Microsoft.Testing.Platform `Test summary:` line.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use z_engine_protocol::TestCounts;

use super::text::{Tally, number, regex};

pub(super) fn parse(lines: &[Cow<'_, str>]) -> Option<TestCounts> {
    static VSTEST: OnceLock<Option<Regex>> = OnceLock::new();
    static SUMMARY: OnceLock<Option<Regex>> = OnceLock::new();
    let vstest = regex(
        &VSTEST,
        r"(?:Passed|Failed)!\s+-\s+Failed:\s+(\d+),\s+Passed:\s+(\d+),\s+Skipped:\s+(\d+)",
    )?;
    let summary = regex(
        &SUMMARY,
        r"Test summary: total: \d+, failed: (\d+), succeeded: (\d+), skipped: (\d+)",
    )?;
    let mut tally = Tally::default();
    for line in lines {
        let found = if line.contains("Failed:") {
            vstest.captures(line)
        } else if line.contains("Test summary:") {
            summary.captures(line)
        } else {
            None
        };
        if let Some(c) = found {
            tally.add(number(&c[2]), number(&c[1]), number(&c[3]));
        }
    }
    tally.finish()
}

#[cfg(test)]
mod tests {
    use super::super::text::clean_lines;
    use super::*;

    #[test]
    fn sums_every_test_assembly() {
        let output = "\
  Determining projects to restore...
  App -> /src/App/bin/Debug/net8.0/App.dll
  App.Tests -> /src/App.Tests/bin/Debug/net8.0/App.Tests.dll
Test run for /src/App.Tests/bin/Debug/net8.0/App.Tests.dll (.NETCoreApp,Version=v8.0)
A total of 1 test files matched the specified pattern.
  Failed App.Tests.MathTests.Divides [4 ms]
  Error Message:
   Assert.Equal() Failure: Values differ

Failed!  - Failed:     1, Passed:     9, Skipped:     1, Total:    11, Duration: 31 ms - App.Tests.dll (net8.0)
Passed!  - Failed:     0, Passed:     4, Skipped:     0, Total:     4, Duration: 12 ms - Api.Tests.dll (net8.0)
";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 13,
                failed: 1,
                skipped: 1
            })
        );
    }

    #[test]
    fn testing_platform_summary() {
        let output =
            "Test summary: total: 7, failed: 2, succeeded: 5, skipped: 0, duration: 1.3s\n";
        assert_eq!(
            parse(&clean_lines(output)),
            Some(TestCounts {
                passed: 5,
                failed: 2,
                skipped: 0
            })
        );
        assert_eq!(parse(&clean_lines("Build succeeded.\n")), None);
    }
}
