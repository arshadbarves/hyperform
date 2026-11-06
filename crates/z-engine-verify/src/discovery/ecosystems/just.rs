//! just: the `test`, `check`, `build` and `lint` recipes of a justfile.

use std::collections::BTreeSet;

use crate::discovery::collector::Collector;
use crate::discovery::ecosystems::make::TARGETS;
use crate::discovery::rel;
use crate::discovery::walk::Found;

pub(crate) fn is_manifest(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "justfile" | ".justfile")
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    let mut dirs: Vec<&str> = found.iter().map(|f| f.dir.as_str()).collect();
    dirs.dedup();
    for dir in dirs {
        let Some(file) = found.iter().find(|f| f.dir == dir) else {
            continue;
        };
        let manifest = rel::join(dir, &file.name);
        let Some(text) = c.read(&manifest) else {
            continue;
        };
        let recipes = recipes(&text);
        let wanted: Vec<_> = TARGETS
            .iter()
            .filter(|(r, _)| recipes.contains(r))
            .collect();
        if wanted.is_empty() {
            if dir.is_empty() {
                c.note(format!("{manifest}: no test, check, build or lint recipe"));
            }
            continue;
        }
        c.add_root(dir, "just", &manifest);
        for (recipe, kind) in wanted {
            c.check(
                dir,
                &manifest,
                &format!("just:{recipe}"),
                *kind,
                format!("just {recipe}"),
            );
        }
    }
}

/// Recipe names: unindented `name [params]:` lines that are not
/// assignments (`name := value`), settings, aliases, imports or modules.
fn recipes(text: &str) -> BTreeSet<&str> {
    let mut recipes = BTreeSet::new();
    for line in text.lines() {
        if line.starts_with([' ', '\t', '#', '[']) {
            continue;
        }
        let line = line.strip_prefix('@').unwrap_or(line);
        let end = line
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(line.len());
        let (name, rest) = line.split_at(end);
        if name.is_empty() || matches!(name, "alias" | "set" | "export" | "import" | "mod") {
            continue;
        }
        let Some(colon) = rest.find(':') else {
            continue;
        };
        if !rest[colon + 1..].starts_with('=') {
            recipes.insert(name);
        }
    }
    recipes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipes_not_assignments() {
        let text = "\
set shell := [\"bash\", \"-c\"]
version := \"1.0\"
alias t := test
export RUST_LOG := \"info\"

# run the tests
[group('dev')]
test filter='': build
    cargo test {{filter}}

@lint:
    cargo clippy

build:
    cargo build
";
        let recipes = recipes(text);
        assert_eq!(
            recipes.into_iter().collect::<Vec<_>>(),
            ["build", "lint", "test"]
        );
    }
}
