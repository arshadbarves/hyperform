//! Node: every `package.json`, monorepo packages included, with its
//! test, build, typecheck, lint and format scripts. The package manager is
//! chosen by the nearest lockfile (the package's directory, then its
//! ancestors), then the `packageManager` field, else npm.

use serde_json::{Map, Value};

use crate::discovery::collector::{Collector, brief};
use crate::discovery::rel;
use crate::discovery::scripts::{CANDIDATES, unusable};
use crate::discovery::walk::Found;

pub(crate) fn is_manifest(name: &str) -> bool {
    name == "package.json"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Manager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

/// Lockfiles by precedence when several share a directory.
const LOCKFILES: &[(&str, Manager)] = &[
    ("pnpm-lock.yaml", Manager::Pnpm),
    ("yarn.lock", Manager::Yarn),
    ("bun.lockb", Manager::Bun),
    ("bun.lock", Manager::Bun),
    ("package-lock.json", Manager::Npm),
    ("npm-shrinkwrap.json", Manager::Npm),
];

impl Manager {
    fn name(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
        }
    }

    /// The command for `script`. A bare `vitest` script would watch in a
    /// terminal, so it runs as `vitest run` through the manager instead.
    fn command(self, script: &str, body: &str) -> String {
        if body.trim() == "vitest" {
            let exec = match self {
                Self::Npm => "npx",
                Self::Pnpm => "pnpm exec",
                Self::Yarn => "yarn",
                Self::Bun => "bunx",
            };
            return format!("{exec} vitest run");
        }
        match (self, script) {
            // `bun test` is Bun's own runner, not the `test` script.
            (Self::Bun, _) => format!("bun run {script}"),
            (_, "test") => format!("{} test", self.name()),
            _ => format!("{} run {script}", self.name()),
        }
    }
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    for f in found {
        let path = rel::join(&f.dir, &f.name);
        let Some(text) = c.read(&path) else {
            continue;
        };
        let manifest = match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(manifest)) => manifest,
            Ok(_) => {
                c.note(format!("{path}: not a JSON object"));
                continue;
            }
            Err(error) => {
                c.note(format!("{path}: invalid JSON ({})", brief(&error)));
                continue;
            }
        };
        let manager = manager(c, &f.dir, &manifest);
        c.add_root(&f.dir, "node", &path);
        let scripts = manifest.get("scripts").and_then(Value::as_object);
        let mut added = 0usize;
        for (name, kind) in CANDIDATES {
            let Some(body) = scripts.and_then(|s| s.get(*name)).and_then(Value::as_str) else {
                continue;
            };
            if let Some(reason) = unusable(name, *kind, body) {
                c.note(format!("{path}: script `{name}` is not a check: {reason}"));
                continue;
            }
            let id = format!("{}:{name}", manager.name());
            c.check(&f.dir, &path, &id, *kind, manager.command(name, body));
            added += 1;
        }
        if added == 0 {
            c.note(format!(
                "{path}: no test, build, typecheck, lint or format script"
            ));
        }
    }
}

fn manager(c: &mut Collector, dir: &str, manifest: &Map<String, Value>) -> Manager {
    for ancestor in rel::ancestors(dir) {
        let present: Vec<&(&str, Manager)> = LOCKFILES
            .iter()
            .filter(|(lock, _)| c.is_file(&rel::join(ancestor, lock)))
            .collect();
        let Some((lock, chosen)) = present.first() else {
            continue;
        };
        if present.iter().any(|(_, other)| other != chosen) {
            c.note(format!(
                "{}: several lockfiles; using {} ({lock})",
                rel::display(ancestor),
                chosen.name()
            ));
        }
        return *chosen;
    }
    let declared = manifest
        .get("packageManager")
        .and_then(Value::as_str)
        .and_then(|spec| spec.split('@').next());
    match declared {
        Some("pnpm") => Manager::Pnpm,
        Some("yarn") => Manager::Yarn,
        Some("bun") => Manager::Bun,
        _ => Manager::Npm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_per_manager() {
        assert_eq!(Manager::Npm.command("test", "jest"), "npm test");
        assert_eq!(Manager::Npm.command("lint", "eslint ."), "npm run lint");
        assert_eq!(
            Manager::Pnpm.command("typecheck", "tsc"),
            "pnpm run typecheck"
        );
        assert_eq!(Manager::Yarn.command("test", "jest"), "yarn test");
        assert_eq!(Manager::Bun.command("test", "bun test"), "bun run test");
        assert_eq!(Manager::Npm.command("test", " vitest "), "npx vitest run");
        assert_eq!(
            Manager::Pnpm.command("test", "vitest"),
            "pnpm exec vitest run"
        );
        assert_eq!(Manager::Yarn.command("test", "vitest"), "yarn vitest run");
        assert_eq!(Manager::Bun.command("test", "vitest"), "bunx vitest run");
        assert_eq!(Manager::Npm.command("test", "vitest --run"), "npm test");
    }
}
