//! A validated grep request: the compiled-pattern check, the resolved search
//! target, and the path forms each engine needs.

use std::path::{Path, PathBuf};

use ignore::overrides::{Override, OverrideBuilder};
use ignore::types::{Types, TypesBuilder};
use regex::{Regex, RegexBuilder};

use super::query::GrepQuery;
use crate::HostError;
use crate::fs::{normalize, resolve};

#[derive(Debug, Clone)]
pub(super) struct Plan {
    /// Paths are displayed relative to this root.
    pub(super) root: PathBuf,
    /// Absolute file or directory to search.
    pub(super) target: PathBuf,
    pub(super) query: GrepQuery,
}

impl Plan {
    pub(super) fn new(root: &Path, query: &GrepQuery) -> Result<Plan, HostError> {
        if query.pattern.is_empty() {
            return Err(HostError::Invalid("grep pattern is empty".to_string()));
        }
        if !query.multiline && query.pattern.contains('\n') {
            return Err(HostError::Invalid(
                "pattern contains a newline; enable multiline to match across lines".to_string(),
            ));
        }
        build_regex(query)?;
        let root = normalize(root);
        if let Some(glob) = &query.glob {
            glob_filter(&root, glob)?;
        }
        if let Some(name) = &query.file_type {
            type_filter(name)?;
        }
        let target = match &query.path {
            Some(path) => resolve(&root, path),
            None => root.clone(),
        };
        if !target.exists() {
            return Err(HostError::NotFound(format!(
                "search path {}",
                target.display()
            )));
        }
        Ok(Plan {
            root,
            target,
            query: query.clone(),
        })
    }

    /// The search path handed to ripgrep (which runs in `root`), chosen so
    /// its output paths come out relative to the root.
    pub(super) fn rg_target(&self) -> PathBuf {
        match self.target.strip_prefix(&self.root) {
            Ok(rel) if rel.as_os_str().is_empty() => PathBuf::from("."),
            Ok(rel) => rel.to_path_buf(),
            Err(_) => self.target.clone(),
        }
    }

    /// How a walked path is displayed: relative to the root when inside it.
    pub(super) fn display(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
            _ => path.display().to_string(),
        }
    }
}

/// The pattern as both engines interpret it: `^`/`$` at line boundaries,
/// `.` crossing newlines only in multiline mode.
pub(super) fn build_regex(query: &GrepQuery) -> Result<Regex, HostError> {
    RegexBuilder::new(&query.pattern)
        .case_insensitive(query.case_insensitive)
        .multi_line(true)
        .dot_matches_new_line(query.multiline)
        .build()
        .map_err(|e| HostError::Invalid(format!("bad pattern `{}`: {e}", query.pattern)))
}

/// ripgrep `--glob` semantics, rooted where ripgrep runs.
pub(super) fn glob_filter(root: &Path, glob: &str) -> Result<Override, HostError> {
    let invalid = |e: ignore::Error| HostError::Invalid(format!("bad glob `{glob}`: {e}"));
    let mut builder = OverrideBuilder::new(root);
    builder.add(glob).map_err(invalid)?;
    builder.build().map_err(invalid)
}

/// ripgrep `--type` with its default type definitions.
pub(super) fn type_filter(name: &str) -> Result<Types, HostError> {
    let mut builder = TypesBuilder::new();
    builder.add_defaults().select(name);
    builder
        .build()
        .map_err(|e| HostError::Invalid(format!("unknown file type `{name}`: {e}")))
}
