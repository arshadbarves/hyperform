//! Diagnostics as `path:line:col: severity: message [code]`, stating when
//! the server said nothing about the current content.

use std::path::Path;

use z_engine_host::relative_display;

use super::super::types::FileDiagnostics;
use super::locations::position;
use super::{MAX_ITEMS, more};

pub fn format_diagnostics(root: &Path, report: &FileDiagnostics) -> String {
    let file = relative_display(root, &report.path);
    if report.diagnostics.is_empty() {
        return if report.fresh {
            format!("No diagnostics in {file}.")
        } else {
            format!(
                "No diagnostics received for {file}: the server published nothing for its current \
                 content in time (it may still be analyzing). This does not show the file is clean."
            )
        };
    }
    let mut lines: Vec<String> = report
        .diagnostics
        .iter()
        .take(MAX_ITEMS)
        .map(|diagnostic| {
            let at = position(root, &diagnostic.path, diagnostic.line, diagnostic.column);
            let code = diagnostic
                .code
                .as_deref()
                .map(|code| format!(" [{code}]"))
                .unwrap_or_default();
            let message = diagnostic.message.trim().replace('\n', "\n    ");
            format!("{at}: {}: {message}{code}", diagnostic.severity.label())
        })
        .collect();
    lines.extend(more(report.diagnostics.len()));
    if !report.fresh {
        lines.push(
            "(These may be stale: nothing was published for the current content in time.)".into(),
        );
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::lsp::types::{Diagnostic, Severity};

    fn diagnostic(severity: Severity, message: &str, code: Option<&str>) -> Diagnostic {
        Diagnostic {
            path: PathBuf::from("/proj/src/lib.rs"),
            line: 3,
            column: 9,
            end_line: 3,
            end_column: 12,
            severity,
            message: message.into(),
            code: code.map(str::to_string),
            source: Some("rustc".into()),
        }
    }

    #[test]
    fn renders_severity_message_and_code() {
        let report = FileDiagnostics {
            path: PathBuf::from("/proj/src/lib.rs"),
            diagnostics: vec![
                diagnostic(
                    Severity::Error,
                    "mismatched types\nexpected i32",
                    Some("E0308"),
                ),
                diagnostic(Severity::Warning, "unused variable", None),
            ],
            fresh: true,
        };
        assert_eq!(
            format_diagnostics(Path::new("/proj"), &report),
            "src/lib.rs:3:9: error: mismatched types\n    expected i32 [E0308]\nsrc/lib.rs:3:9: warning: unused variable"
        );
    }

    #[test]
    fn empty_results_say_whether_they_are_fresh() {
        let mut report = FileDiagnostics {
            path: PathBuf::from("/proj/src/lib.rs"),
            diagnostics: Vec::new(),
            fresh: true,
        };
        assert_eq!(
            format_diagnostics(Path::new("/proj"), &report),
            "No diagnostics in src/lib.rs."
        );
        report.fresh = false;
        let stale = format_diagnostics(Path::new("/proj"), &report);
        assert!(stale.contains("does not show the file is clean"), "{stale}");
        report
            .diagnostics
            .push(diagnostic(Severity::Hint, "h", None));
        assert!(format_diagnostics(Path::new("/proj"), &report).ends_with("in time.)"));
    }
}
