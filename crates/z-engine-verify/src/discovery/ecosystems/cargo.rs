//! Cargo: one root per workspace or standalone package. A crate inside a
//! workspace directory belongs to the nearest enclosing workspace (unless
//! that workspace excludes it) and is covered by the workspace's checks.

use z_engine_protocol::CheckKind;

use crate::discovery::collector::{Collector, brief};
use crate::discovery::rel;
use crate::discovery::walk::Found;

pub(crate) fn is_manifest(name: &str) -> bool {
    name == "Cargo.toml"
}

struct Manifest {
    dir: String,
    path: String,
    workspace: bool,
    exclude: Vec<String>,
}

pub(crate) fn discover(c: &mut Collector, found: &[Found]) {
    let manifests: Vec<Manifest> = found.iter().filter_map(|f| read(c, f)).collect();
    for manifest in &manifests {
        if !manifest.workspace && is_member(manifest, &manifests) {
            continue;
        }
        let (dir, path) = (manifest.dir.as_str(), manifest.path.as_str());
        let all = if manifest.workspace {
            " --workspace"
        } else {
            ""
        };
        c.add_root(dir, "cargo", path);
        c.check(
            dir,
            path,
            "cargo:test",
            CheckKind::Test,
            format!("cargo test{all}"),
        );
        c.check(
            dir,
            path,
            "cargo:build",
            CheckKind::Build,
            format!("cargo build{all}"),
        );
        c.check(
            dir,
            path,
            "cargo:clippy",
            CheckKind::Lint,
            format!("cargo clippy{all} --all-targets"),
        );
        // At a workspace root `cargo fmt` already covers every member.
        c.check(
            dir,
            path,
            "cargo:fmt",
            CheckKind::Format,
            "cargo fmt --check",
        );
    }
}

fn read(c: &mut Collector, found: &Found) -> Option<Manifest> {
    let path = rel::join(&found.dir, &found.name);
    let text = c.read(&path)?;
    let table = match text.parse::<toml::Table>() {
        Ok(table) => table,
        Err(error) => {
            c.note(format!("{path}: invalid TOML ({})", brief(&error)));
            return None;
        }
    };
    let workspace = table.get("workspace").and_then(toml::Value::as_table);
    let package = table.get("package").and_then(toml::Value::as_table);
    if workspace.is_none() && package.is_none() {
        c.note(format!(
            "{path}: has neither a [package] nor a [workspace] table"
        ));
        return None;
    }
    let exclude = workspace
        .and_then(|w| w.get("exclude"))
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(rel::clean)
                .collect()
        })
        .unwrap_or_default();
    Some(Manifest {
        dir: found.dir.clone(),
        path,
        workspace: workspace.is_some(),
        exclude,
    })
}

fn is_member(package: &Manifest, all: &[Manifest]) -> bool {
    let nearest = all
        .iter()
        .filter(|w| w.workspace && w.dir != package.dir && rel::within(&package.dir, &w.dir))
        .max_by_key(|w| rel::depth(&w.dir));
    nearest.is_some_and(|workspace| {
        let local = rel::strip(&package.dir, &workspace.dir);
        !workspace
            .exclude
            .iter()
            .any(|excluded| rel::within(local, excluded))
    })
}
