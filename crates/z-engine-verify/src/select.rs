//! Which checks to run: those matching the requested kinds or ids, scoped
//! to the project roots that contain the changed paths.

use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};

use z_engine_protocol::CheckKind;

use crate::{CheckSpec, ProjectProfile};

/// Checks matching any selector — a kind (`test`, `build`, `lint`, ...), a
/// check id (`web/npm:test`), an id without its root (`npm:test`), or a
/// configured check's own id (`unit` for `custom:unit`); no selectors means
/// every check. When the matches span several roots, each
/// changed path (root-relative) is attributed to the deepest root holding
/// it and only those roots' checks are kept; if no match lies in such a
/// root, all matches are kept. Profile order, each id once.
pub fn select_checks<'a>(
    profile: &'a ProjectProfile,
    selectors: &[String],
    changed: &[PathBuf],
) -> Vec<&'a CheckSpec> {
    let mut seen = HashSet::new();
    let matching: Vec<&CheckSpec> = profile
        .checks
        .iter()
        .filter(|check| selectors.is_empty() || selectors.iter().any(|s| matches(check, s)))
        .filter(|check| seen.insert(check.id.as_str()))
        .collect();
    scope(matching, changed)
}

fn matches(check: &CheckSpec, selector: &str) -> bool {
    let selector = selector.trim();
    !selector.is_empty()
        && (check.id == selector
            || check
                .id
                .rsplit_once('/')
                .is_some_and(|(_, local)| local == selector)
            || check.id.strip_prefix("custom:") == Some(selector)
            || CheckKind::parse(selector) == Some(check.kind))
}

fn scope<'a>(checks: Vec<&'a CheckSpec>, changed: &[PathBuf]) -> Vec<&'a CheckSpec> {
    let dirs: Vec<Option<Vec<String>>> = checks.iter().map(|c| relative_parts(&c.cwd)).collect();
    let roots: BTreeSet<&Vec<String>> = dirs.iter().flatten().collect();
    if changed.is_empty() || roots.len() < 2 {
        return checks;
    }
    let preferred: BTreeSet<&Vec<String>> = changed
        .iter()
        .filter_map(|path| relative_parts(path))
        .filter_map(|path| {
            roots
                .iter()
                .copied()
                .filter(|root| path.starts_with(root))
                .max_by_key(|root| root.len())
        })
        .collect();
    let scoped: Vec<&CheckSpec> = checks
        .iter()
        .zip(&dirs)
        .filter(|(_, dir)| dir.as_ref().is_some_and(|dir| preferred.contains(dir)))
        .map(|(check, _)| *check)
        .collect();
    if scoped.is_empty() { checks } else { scoped }
}

/// Normal components of a root-relative path; `None` for absolute paths
/// and paths that climb above the root.
fn relative_parts(path: &Path) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts)
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
