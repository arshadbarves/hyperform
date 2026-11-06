//! Command-level parsing (lexer output plus `eval` detection) and the public
//! [`ShellAnalysis`] summary.

use super::lexer::lex;
use super::read_only::command_is_read_only;
use super::syntax::Segment;
use super::wrappers::executed_commands;

/// What static analysis can tell about a shell command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellAnalysis {
    /// Argument vector of each top-level simple command, with reserved words
    /// (`then`, `do`, `!`, ...) removed.
    pub segments: Vec<Vec<String>>,
    /// Quotes, substitutions, parentheses, and heredocs were all balanced.
    pub parse_ok: bool,
    /// Command or process substitution, backticks, arithmetic, or `eval`:
    /// part of the command is only known at run time.
    pub dynamic: bool,
    /// Files opened for writing by redirects; `/dev/null`, the standard
    /// streams, and descriptor duplication (`2>&1`) are not writes.
    pub writes: Vec<String>,
    /// Same as [`super::is_read_only`].
    pub read_only: bool,
}

/// Analyzes `command` without running anything.
pub fn analyze(command: &str) -> ShellAnalysis {
    let parsed = parse(command);
    ShellAnalysis {
        segments: parsed
            .segments
            .iter()
            .map(Segment::argv)
            .filter(|argv| !argv.is_empty())
            .collect(),
        parse_ok: parsed.ok,
        dynamic: parsed.dynamic,
        writes: parsed
            .all_segments()
            .flat_map(Segment::write_targets)
            .map(|word| word.text.clone())
            .collect(),
        read_only: command_is_read_only(&parsed),
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Parsed {
    pub segments: Vec<Segment>,
    /// Commands inside substitutions and expanding heredocs.
    pub nested: Vec<Segment>,
    pub ok: bool,
    pub dynamic: bool,
}

impl Parsed {
    pub fn all_segments(&self) -> impl Iterator<Item = &Segment> {
        self.segments.iter().chain(&self.nested)
    }
}

pub(crate) fn parse(command: &str) -> Parsed {
    let lexed = lex(command);
    let evaluates = lexed.segments.iter().chain(&lexed.nested).any(runs_eval);
    Parsed {
        segments: lexed.segments,
        nested: lexed.nested,
        ok: lexed.ok,
        dynamic: lexed.dynamic || evaluates,
    }
}

fn runs_eval(segment: &Segment) -> bool {
    executed_commands(segment).0.iter().any(|candidate| {
        candidate
            .words
            .first()
            .is_some_and(|word| word.text == "eval")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_reports_segments_writes_and_flags() {
        let analysis = analyze("cargo test 2>&1 | tee log.txt > out.txt; echo done >/dev/null");
        assert_eq!(
            analysis.segments,
            [
                vec!["cargo", "test"],
                vec!["tee", "log.txt"],
                vec!["echo", "done"]
            ]
        );
        assert!(analysis.parse_ok);
        assert!(!analysis.dynamic);
        assert_eq!(analysis.writes, ["out.txt"]);
        assert!(!analysis.read_only);
    }

    #[test]
    fn eval_is_dynamic_even_behind_wrappers() {
        assert!(analyze("eval \"$CMD\"").dynamic);
        assert!(analyze("command eval ls").dynamic);
        assert!(analyze("sudo -u root eval ls").dynamic);
        assert!(analyze("bash -c 'eval ls'").dynamic);
        assert!(!analyze("grep -rn eval src").dynamic);
        assert!(!analyze("echo eval").dynamic);
    }

    #[test]
    fn nested_writes_are_reported() {
        let analysis = analyze("echo $(date > stamp)");
        assert!(analysis.dynamic);
        assert_eq!(analysis.writes, ["stamp"]);
    }

    #[test]
    fn read_only_matches_the_classifier() {
        assert!(analyze("git status && ls -la").read_only);
        assert!(!analyze("git push").read_only);
        assert!(!analyze("").read_only);
    }
}
