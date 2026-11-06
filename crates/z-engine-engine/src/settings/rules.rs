//! Glob-scoped rules: a rule that is not `alwaysApply` joins an agent's
//! context the first time it reads or writes a file matching one of its
//! gitignore-style globs (relative to the agent's root).

use std::collections::HashSet;
use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use z_engine_config::RuleDef;
use z_engine_context::InstructionDoc;

/// Rules matching `files` that are not in `seen` yet; their keys are added
/// to `seen` so each rule is shown once per agent.
pub(crate) fn scoped_rules<'a>(
    root: &Path,
    files: impl IntoIterator<Item = &'a Path>,
    rules: &[RuleDef],
    seen: &mut HashSet<String>,
) -> Vec<InstructionDoc> {
    let scoped: Vec<(&RuleDef, GlobSet)> = rules
        .iter()
        .filter(|rule| !rule.always_apply && !rule.body.trim().is_empty())
        .filter(|rule| !seen.contains(&key(rule)))
        .filter_map(|rule| matcher(&rule.globs).map(|globs| (rule, globs)))
        .collect();
    if scoped.is_empty() {
        return Vec::new();
    }
    let relative: Vec<&Path> = files
        .into_iter()
        .filter_map(|file| file.strip_prefix(root).ok())
        .collect();
    let mut docs = Vec::new();
    for (rule, globs) in scoped {
        if relative.iter().any(|file| globs.is_match(file)) && seen.insert(key(rule)) {
            docs.push(InstructionDoc {
                label: format!("Rule ({})", rule.name),
                path: rule.source.path.clone(),
                content: rule.body.clone(),
            });
        }
    }
    docs
}

fn key(rule: &RuleDef) -> String {
    format!("rule:{}", rule.source.path)
}

/// A glob without `/` matches at any depth, like in `.gitignore`.
fn matcher(globs: &[String]) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut any = false;
    for glob in globs {
        let glob = glob.trim().trim_start_matches('/');
        if glob.is_empty() {
            continue;
        }
        let mut patterns = vec![glob.to_string()];
        if !glob.contains('/') {
            patterns.push(format!("**/{glob}"));
        }
        for pattern in patterns {
            match GlobBuilder::new(&pattern).literal_separator(true).build() {
                Ok(compiled) => {
                    builder.add(compiled);
                    any = true;
                }
                Err(error) => tracing::debug!(%error, pattern, "rule glob ignored"),
            }
        }
    }
    any.then(|| builder.build().ok()).flatten()
}

#[cfg(test)]
mod tests {
    use z_engine_config::{ExtensionScope, ExtensionSource};

    use super::*;

    fn rule(name: &str, globs: &[&str], always: bool) -> RuleDef {
        RuleDef {
            name: name.into(),
            description: String::new(),
            globs: globs.iter().map(|glob| glob.to_string()).collect(),
            always_apply: always,
            body: format!("{name} body"),
            source: ExtensionSource::new(
                ExtensionScope::Project,
                Path::new(&format!("/p/.z-engine/rules/{name}.md")),
            ),
        }
    }

    #[test]
    fn matching_rules_are_injected_once() {
        let rules = [
            rule("rust", &["*.rs"], false),
            rule("web", &["web/**/*.ts"], false),
            rule("always", &["*.rs"], true),
            rule("none", &[], false),
        ];
        let root = Path::new("/p");
        let mut seen = HashSet::new();
        let file = Path::new("/p/src/deep/lib.rs");
        let docs = scoped_rules(root, [file], &rules, &mut seen);
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].label, "Rule (rust)");
        assert!(scoped_rules(root, [file], &rules, &mut seen).is_empty());
        let outside = Path::new("/p/src/app.ts");
        assert!(scoped_rules(root, [outside], &rules, &mut seen).is_empty());
        let web = Path::new("/p/web/ui/app.ts");
        assert_eq!(scoped_rules(root, [web], &rules, &mut seen).len(), 1);
    }
}
