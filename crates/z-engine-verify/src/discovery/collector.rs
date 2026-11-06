//! What the walk and the ecosystem parsers write into: roots, checks with
//! consistent ids and labels, and notes; plus bounded reads and existence
//! probes relative to the discovery root.

use std::path::{Path, PathBuf};

use z_engine_protocol::CheckKind;

use super::read::read_bounded;
use super::rel;
use super::types::{DiscoveryOptions, ProjectProfile, ProjectRoot};
use crate::{CheckSource, CheckSpec, DEFAULT_CHECK_TIMEOUT_SECS};

/// Notes kept before the rest are only counted.
const MAX_NOTES: usize = 50;

#[derive(Debug)]
pub(crate) struct Collector<'a> {
    root: &'a Path,
    max_bytes: u64,
    roots: Vec<(String, ProjectRoot)>,
    checks: Vec<(String, CheckSpec)>,
    notes: Vec<String>,
    omitted_notes: usize,
}

impl<'a> Collector<'a> {
    pub(crate) fn new(root: &'a Path, options: &DiscoveryOptions) -> Self {
        Self {
            root,
            max_bytes: options.max_manifest_bytes,
            roots: Vec::new(),
            checks: Vec::new(),
            notes: Vec::new(),
            omitted_notes: 0,
        }
    }

    /// Absolute path of a root-relative path.
    pub(crate) fn path(&self, rel: &str) -> PathBuf {
        if rel.is_empty() {
            self.root.to_path_buf()
        } else {
            self.root.join(rel)
        }
    }

    /// The text of a root-relative file, or `None` with a note saying why.
    pub(crate) fn read(&mut self, rel: &str) -> Option<String> {
        match read_bounded(&self.path(rel), self.max_bytes) {
            Ok(text) => Some(text),
            Err(problem) => {
                self.note(format!("{rel}: {problem}"));
                None
            }
        }
    }

    pub(crate) fn is_file(&self, rel: &str) -> bool {
        self.path(rel).is_file()
    }

    pub(crate) fn is_dir(&self, rel: &str) -> bool {
        self.path(rel).is_dir()
    }

    pub(crate) fn add_root(&mut self, dir: &str, ecosystem: &str, manifest: &str) {
        let exists = self
            .roots
            .iter()
            .any(|(d, root)| d == dir && root.ecosystem == ecosystem);
        if !exists {
            self.roots.push((
                dir.to_string(),
                ProjectRoot {
                    path: rel::display(dir).to_string(),
                    ecosystem: ecosystem.to_string(),
                    manifest: manifest.to_string(),
                },
            ));
        }
    }

    /// Adds `<dir>/<local_id>` (just `local_id` at the root), labelled with
    /// the command and, for nested roots, the directory.
    pub(crate) fn check(
        &mut self,
        dir: &str,
        manifest: &str,
        local_id: &str,
        kind: CheckKind,
        command: impl Into<String>,
    ) {
        let command = command.into();
        let (id, label) = if dir.is_empty() {
            (local_id.to_string(), command.clone())
        } else {
            (format!("{dir}/{local_id}"), format!("{command} ({dir})"))
        };
        if self.checks.iter().any(|(_, check)| check.id == id) {
            return;
        }
        let spec = CheckSpec {
            id,
            label,
            kind,
            command,
            cwd: PathBuf::from(rel::display(dir)),
            source: CheckSource::Discovered {
                manifest: manifest.to_string(),
            },
            timeout_secs: DEFAULT_CHECK_TIMEOUT_SECS,
        };
        self.checks.push((dir.to_string(), spec));
    }

    pub(crate) fn note(&mut self, text: impl Into<String>) {
        if self.notes.len() < MAX_NOTES {
            self.notes.push(text.into());
        } else {
            self.omitted_notes += 1;
        }
    }

    pub(crate) fn finish(mut self) -> ProjectProfile {
        self.roots.sort_by(|a, b| rel::tree_order(&a.0, &b.0));
        self.checks.sort_by(|a, b| rel::tree_order(&a.0, &b.0));
        if self.omitted_notes > 0 {
            self.notes
                .push(format!("{} more notes were omitted", self.omitted_notes));
        }
        ProjectProfile {
            roots: self.roots.into_iter().map(|(_, root)| root).collect(),
            checks: self.checks.into_iter().map(|(_, check)| check).collect(),
            notes: self.notes,
        }
    }
}

/// A parser error on one line: TOML and JSON errors carry source excerpts
/// that do not belong in a note.
pub(crate) fn brief(error: &dyn std::fmt::Display) -> String {
    let text = error.to_string();
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    match (lines.first(), lines.last()) {
        (Some(first), Some(last)) if first != last => format!("{first}: {last}"),
        (Some(first), _) => (*first).to_string(),
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_checks_are_prefixed_and_sorted_after_the_root() {
        let root = Path::new("/project");
        let mut collector = Collector::new(root, &DiscoveryOptions::default());
        collector.add_root("web", "node", "web/package.json");
        collector.check(
            "web",
            "web/package.json",
            "npm:test",
            CheckKind::Test,
            "npm test",
        );
        collector.add_root("", "cargo", "Cargo.toml");
        collector.add_root("", "cargo", "Cargo.toml");
        collector.check(
            "",
            "Cargo.toml",
            "cargo:test",
            CheckKind::Test,
            "cargo test",
        );
        collector.check(
            "",
            "Cargo.toml",
            "cargo:test",
            CheckKind::Test,
            "cargo test --twice",
        );
        let profile = collector.finish();
        assert_eq!(profile.roots.len(), 2);
        assert_eq!(profile.roots[0].path, ".");
        let ids: Vec<_> = profile.checks.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["cargo:test", "web/npm:test"]);
        assert_eq!(profile.checks[0].label, "cargo test");
        assert_eq!(profile.checks[0].cwd, PathBuf::from("."));
        assert_eq!(profile.checks[1].label, "npm test (web)");
        assert_eq!(profile.checks[1].cwd, PathBuf::from("web"));
    }

    #[test]
    fn notes_are_capped() {
        let mut collector = Collector::new(Path::new("/p"), &DiscoveryOptions::default());
        for i in 0..MAX_NOTES + 3 {
            collector.note(format!("note {i}"));
        }
        let notes = collector.finish().notes;
        assert_eq!(notes.len(), MAX_NOTES + 1);
        assert_eq!(notes[MAX_NOTES], "3 more notes were omitted");
    }

    #[test]
    fn brief_keeps_the_location_and_the_message() {
        let error =
            "TOML parse error at line 1, column 5\n  |\n1 | [bad\n  |     ^\ninvalid table header";
        assert_eq!(
            brief(&error),
            "TOML parse error at line 1, column 5: invalid table header"
        );
        assert_eq!(
            brief(&"expected value at line 1 column 1"),
            "expected value at line 1 column 1"
        );
    }
}
