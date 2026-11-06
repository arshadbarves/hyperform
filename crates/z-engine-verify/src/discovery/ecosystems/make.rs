//! Make: the `test`, `check`, `build` and `lint` targets of the makefile
//! GNU make would read. Only literal, unconditional rule targets count;
//! variables, includes and conditionals are not evaluated.

use std::collections::BTreeSet;

use z_engine_protocol::CheckKind;

use crate::discovery::collector::Collector;
use crate::discovery::rel;
use crate::discovery::walk::Found;

/// GNU make's lookup order.
const MAKEFILES: &[&str] = &["GNUmakefile", "makefile", "Makefile"];
pub(crate) const TARGETS: &[(&str, CheckKind)] = &[
    ("test", CheckKind::Test),
    ("check", CheckKind::Test),
    ("build", CheckKind::Build),
    ("lint", CheckKind::Lint),
];

pub(crate) fn is_manifest(name: &str) -> bool {
    MAKEFILES.contains(&name)
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    let mut dirs: Vec<&str> = found.iter().map(|f| f.dir.as_str()).collect();
    dirs.dedup();
    for dir in dirs {
        let Some(name) = MAKEFILES
            .iter()
            .find(|m| found.iter().any(|f| f.dir == dir && f.name == **m))
        else {
            continue;
        };
        let manifest = rel::join(dir, name);
        let Some(text) = c.read(&manifest) else {
            continue;
        };
        let targets = match targets(&text) {
            Some(targets) => targets,
            None => {
                c.note(format!(
                    "{manifest}: non-GNU make syntax; targets were not read"
                ));
                continue;
            }
        };
        let wanted: Vec<&(&str, CheckKind)> = TARGETS
            .iter()
            .filter(|(t, _)| targets.contains(t))
            .collect();
        if wanted.is_empty() {
            if dir.is_empty() {
                c.note(format!("{manifest}: no test, check, build or lint target"));
            }
            continue;
        }
        c.add_root(dir, "make", &manifest);
        for (target, kind) in wanted {
            c.check(
                dir,
                &manifest,
                &format!("make:{target}"),
                *kind,
                format!("make {target}"),
            );
        }
    }
}

/// Literal rule targets, or `None` for dialects this reader cannot follow.
fn targets(text: &str) -> Option<BTreeSet<&str>> {
    let unknown_dialect = text.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with(".RECIPEPREFIX") || line.starts_with(".if")
    });
    if unknown_dialect {
        return None;
    }
    let mut targets = BTreeSet::new();
    let mut conditional = 0usize;
    let mut continued = false;
    for line in text.lines() {
        let skip = continued;
        continued = line.trim_end().ends_with('\\');
        if skip || line.starts_with('\t') || line.trim_start().starts_with('#') {
            continue;
        }
        let directive = line.split_whitespace().next().unwrap_or("");
        if matches!(directive, "ifdef" | "ifndef" | "ifeq" | "ifneq" | "define") {
            conditional += 1;
            continue;
        }
        if matches!(directive, "endif" | "endef") {
            conditional = conditional.saturating_sub(1);
            continue;
        }
        if conditional != 0 || line.starts_with(char::is_whitespace) || continued {
            continue;
        }
        let Some((left, right)) = line.split_once(':') else {
            continue;
        };
        if right.starts_with('=') || left.contains(['$', '%', '=', '\\', '#']) {
            continue;
        }
        targets.extend(left.split_whitespace());
    }
    Some(targets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_unconditional_targets_only() {
        let text = "\
.PHONY: test lint
CC := cc
test: build
\tcargo test
build:
\tcargo build
ifeq ($(CI),1)
lint:
\techo lint
endif
$(OUT)/check: x
docs: \\
  more
";
        let found = targets(text).unwrap();
        assert!(found.contains("test") && found.contains("build"));
        assert!(!found.contains("lint") && !found.contains("check"));
        assert!(!found.contains("CC"));
        assert_eq!(targets(".RECIPEPREFIX = >\ntest:\n> echo\n"), None);
    }
}
