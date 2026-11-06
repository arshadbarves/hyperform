//! Path specifiers for `Read(..)`, `Edit(..)` and file tools, with
//! gitignore-style semantics.

use std::fmt;
use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::engine::PolicyContext;
use crate::paths::resolve;

/// Default macOS and Windows filesystems ignore case, so `.ENV` names the
/// same file as `.env` there.
const CASE_INSENSITIVE: bool = cfg!(any(target_os = "macos", target_os = "windows"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Project,
    Home,
    Root,
}

/// A gitignore-style pattern. `src/**` is relative to the project root, `~/x`
/// to the home directory, and `//abs/x` to the filesystem root. A single
/// leading `/` is tried both ways: absolute (the v2 spelling) and relative
/// to the project root (Claude Code's spelling). A pattern without an inner
/// `/` matches at any depth (`*.env`), and a pattern that matches a directory
/// also matches everything below it.
#[derive(Debug, Clone)]
pub(crate) struct PathPattern {
    source: String,
    anchors: &'static [Anchor],
    matcher: GlobSet,
}

#[derive(Debug)]
pub(crate) enum PatternError {
    Invalid(&'static str),
    Glob(globset::Error),
}

impl PathPattern {
    pub fn parse(source: &str) -> Result<Self, PatternError> {
        let source = source.trim();
        if source.starts_with('!') {
            return Err(PatternError::Invalid("negated patterns are not supported"));
        }
        let (anchors, rest): (&'static [Anchor], &str) =
            if let Some(rest) = source.strip_prefix("//") {
                (&[Anchor::Root], rest)
            } else if let Some(rest) = source.strip_prefix('~') {
                if !(rest.is_empty() || rest.starts_with('/')) {
                    return Err(PatternError::Invalid("only `~/` home paths are supported"));
                }
                (&[Anchor::Home], rest)
            } else if let Some(rest) = source.strip_prefix('/') {
                (&[Anchor::Root, Anchor::Project], rest)
            } else {
                (
                    &[Anchor::Project],
                    source.strip_prefix("./").unwrap_or(source),
                )
            };
        let body = match rest.trim_matches('/') {
            "." => "",
            body => body,
        };
        let anchored = *anchors != [Anchor::Project] || body.contains('/');
        let mut builder = GlobSetBuilder::new();
        for glob in globs(body, anchored) {
            let glob = GlobBuilder::new(&glob)
                .literal_separator(true)
                .case_insensitive(CASE_INSENSITIVE)
                .build()
                .map_err(PatternError::Glob)?;
            builder.add(glob);
        }
        let matcher = builder.build().map_err(PatternError::Glob)?;
        Ok(Self {
            source: source.to_string(),
            anchors,
            matcher,
        })
    }

    /// `path` should be absolute and normalized; a relative path is taken
    /// from the project root.
    pub fn matches(&self, path: &Path, ctx: &PolicyContext) -> bool {
        let path = resolve(&ctx.project_root, path);
        let under = |base: &Path| {
            path.strip_prefix(base)
                .is_ok_and(|relative| self.matcher.is_match(relative))
        };
        self.anchors.iter().any(|anchor| match anchor {
            Anchor::Project => under(&ctx.project_root),
            Anchor::Home => ctx.home.as_deref().is_some_and(under),
            Anchor::Root => self.matcher.is_match(root_relative(&path)),
        })
    }
}

/// The glob and its descendants; unanchored bodies match at any depth. A
/// `dir/**` pattern also matches `dir` itself, so listing or searching the
/// directory counts as reading what is inside it.
fn globs(body: &str, anchored: bool) -> Vec<String> {
    if body.is_empty() || body == "**" {
        return vec!["**".to_string()];
    }
    let base = if anchored {
        body.to_string()
    } else {
        format!("**/{body}")
    };
    match base.strip_suffix("/**") {
        Some(dir) => vec![dir.to_string(), base.clone()],
        None => vec![format!("{base}/**"), base],
    }
}

/// The absolute path without its leading separator; a drive prefix is kept
/// (`C:/Users/x`).
fn root_relative(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    match text.strip_prefix('/') {
        Some(rest) => rest.to_string(),
        None => text,
    }
}

impl PartialEq for PathPattern {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Eq for PathPattern {}

impl fmt::Display for PathPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

#[cfg(test)]
#[path = "path_pattern_tests.rs"]
mod tests;
