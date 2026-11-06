//! Go: one root per `go.mod` that declares a module.

use z_engine_protocol::CheckKind;

use crate::discovery::collector::Collector;
use crate::discovery::rel;
use crate::discovery::walk::Found;

pub(crate) fn is_manifest(name: &str) -> bool {
    name == "go.mod"
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    for f in found {
        let manifest = rel::join(&f.dir, &f.name);
        let Some(text) = c.read(&manifest) else {
            continue;
        };
        if !declares_module(&text) {
            c.note(format!("{manifest}: no `module` directive"));
            continue;
        }
        let dir = f.dir.as_str();
        c.add_root(dir, "go", &manifest);
        c.check(dir, &manifest, "go:test", CheckKind::Test, "go test ./...");
        c.check(
            dir,
            &manifest,
            "go:build",
            CheckKind::Build,
            "go build ./...",
        );
        c.check(dir, &manifest, "go:vet", CheckKind::Lint, "go vet ./...");
    }
}

fn declares_module(text: &str) -> bool {
    text.lines()
        .map(|line| line.split("//").next().unwrap_or("").trim())
        .any(|line| {
            line.strip_prefix("module").is_some_and(|rest| {
                rest.starts_with(char::is_whitespace) && !rest.trim().is_empty()
            })
        })
}
